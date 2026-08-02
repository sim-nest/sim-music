//! Canonical `Score` to bounded MusicXML partwise export.

use num_rational::Ratio;
use sim_lib_music_core::{Articulation, MelodyItem, Music, Score, Time};
use sim_lib_pitch_core::{Letter, SpelledPitch};

use crate::{
    model::{
        NotationError, NotationIdentity, NotationIdentityKind, NotationLoss, NotationLossKind,
        NotationReport, loss_diagnostic, musicxml_error,
    },
    musicxml_support::{
        MAX_DIVISIONS, checked_lcm, ensure_unique_identity_ids, escape_xml, fifths_from_key,
        identity_map, item_duration, retained_id, validate_xml_id,
    },
    spell::spell_pitch_in_key,
};

/// Exports canonical `Score` through the bounded MusicXML partwise profile.
///
/// Matching canonical paths in `identities` reproduce identifiers retained by
/// a prior import. Missing identifiers are allocated deterministically.
pub fn export_musicxml_partwise_report(
    score: &Score,
    identities: &[NotationIdentity],
) -> Result<NotationReport<String>, NotationError> {
    let parts = export_parts(score)?;
    let divisions = divisions_for(&parts)?;
    let measure_duration = Ratio::new(
        i64::from(score.time_signature.0),
        i64::from(score.time_signature.1),
    );
    if measure_duration <= Ratio::from_integer(0) {
        return Err(musicxml_error(
            "MusicXML export requires a positive time signature",
            None,
        ));
    }
    let identity_map = identity_map(identities)?;
    let mut retained = Vec::new();
    let mut losses = Vec::new();
    let mut output = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str("<score-partwise version=\"4.0\">\n  <part-list>\n");
    for (part_index, part) in parts.iter().enumerate() {
        let path = format!("part/{part_index}");
        let id =
            retained_id(&identity_map, &path).unwrap_or_else(|| format!("P{}", part_index + 1));
        validate_xml_id(&id)?;
        retained.push(NotationIdentity {
            kind: NotationIdentityKind::Part,
            canonical_path: path,
            xml_id: id.clone(),
        });
        output.push_str(&format!(
            "    <score-part id=\"{}\"><part-name>{}</part-name></score-part>\n",
            escape_xml(&id),
            escape_xml(&part.name),
        ));
    }
    output.push_str("  </part-list>\n");

    for (part_index, part) in parts.iter().enumerate() {
        let part_path = format!("part/{part_index}");
        let part_id = retained_id(&identity_map, &part_path)
            .unwrap_or_else(|| format!("P{}", part_index + 1));
        let measures = partition_measures(&part.items, measure_duration)?;
        output.push_str(&format!("  <part id=\"{}\">\n", escape_xml(&part_id)));
        let mut event_index = 0usize;
        for (measure_index, measure) in measures.iter().enumerate() {
            output.push_str(&format!("    <measure number=\"{}\">\n", measure_index + 1));
            if measure_index == 0 {
                render_attributes(&mut output, score, divisions)?;
                output.push_str(&format!(
                    "      <direction><sound tempo=\"{}\"/></direction>\n",
                    score.tempo_bpm
                ));
            }
            for item in *measure {
                let path = format!("part/{part_index}/event/{event_index}");
                let id = retained_id(&identity_map, &path)
                    .unwrap_or_else(|| format!("P{}-E{}", part_index + 1, event_index + 1));
                validate_xml_id(&id)?;
                retained.push(NotationIdentity {
                    kind: NotationIdentityKind::Event,
                    canonical_path: path.clone(),
                    xml_id: id.clone(),
                });
                render_item(
                    &mut output,
                    item,
                    &id,
                    divisions,
                    score.key.as_deref(),
                    &path,
                    &mut losses,
                )?;
                event_index += 1;
            }
            output.push_str("    </measure>\n");
        }
        output.push_str("  </part>\n");
    }
    output.push_str("</score-partwise>\n");
    ensure_unique_identity_ids(&retained)?;
    let diagnostics = losses.iter().map(loss_diagnostic).collect();
    Ok(NotationReport {
        value: output,
        diagnostics,
        identities: retained,
        losses,
    })
}

/// Exports with deterministic profile identifiers and discards sidecar evidence.
pub fn export_musicxml_partwise(score: &Score) -> Result<String, NotationError> {
    Ok(export_musicxml_partwise_report(score, &[])?.value)
}

#[derive(Clone)]
struct ExportPart {
    name: String,
    items: Vec<MelodyItem>,
}

fn export_parts(score: &Score) -> Result<Vec<ExportPart>, NotationError> {
    let parts = match &score.body {
        Music::Note(note) => vec![ExportPart {
            name: "Music".to_owned(),
            items: vec![MelodyItem::Note(note.clone())],
        }],
        Music::Rest(rest) => vec![ExportPart {
            name: "Music".to_owned(),
            items: vec![MelodyItem::Rest(rest.clone())],
        }],
        Music::Melody(melody) => vec![ExportPart {
            name: "Music".to_owned(),
            items: melody.items.clone(),
        }],
        Music::Counterpoint(counterpoint) => counterpoint
            .voices
            .iter()
            .zip(counterpoint.normalized_voice_names())
            .map(|(melody, name)| ExportPart {
                name,
                items: melody.items.clone(),
            })
            .collect(),
        other => {
            return Err(NotationError::UnsupportedMusicObject(match other {
                Music::Chord(_) => "Chord",
                Music::Progression(_) => "Progression",
                Music::Par(_) => "Par",
                Music::Seq(_) => "Seq",
                Music::PianoRoll(_) => "PianoRoll",
                Music::Arranger(_) => "Arranger",
                Music::MidiTrack(_) => "MidiTrack",
                Music::MidiFile(_) => "MidiFile",
                _ => "Unknown",
            }));
        }
    };
    if parts.is_empty() || parts.iter().any(|part| part.items.is_empty()) {
        return Err(musicxml_error(
            "bounded MusicXML export requires at least one event in every part",
            None,
        ));
    }
    Ok(parts)
}

fn divisions_for(parts: &[ExportPart]) -> Result<i64, NotationError> {
    let mut divisions = 1i64;
    for item in parts.iter().flat_map(|part| &part.items) {
        let quarters = item_duration(item) * Ratio::from_integer(4);
        if quarters <= Ratio::from_integer(0) {
            return Err(musicxml_error(
                "bounded MusicXML export requires positive event durations",
                None,
            ));
        }
        divisions = checked_lcm(divisions, *quarters.denom()).ok_or_else(|| {
            musicxml_error(
                "MusicXML divisions overflow while preserving exact time",
                None,
            )
        })?;
        if divisions > MAX_DIVISIONS {
            return Err(musicxml_error(
                format!("exact score requires divisions above {MAX_DIVISIONS}"),
                None,
            ));
        }
    }
    Ok(divisions)
}

fn partition_measures(
    items: &[MelodyItem],
    measure_duration: Time,
) -> Result<Vec<&[MelodyItem]>, NotationError> {
    let mut measures = Vec::new();
    let mut start = 0usize;
    let mut elapsed = Ratio::from_integer(0);
    for (index, item) in items.iter().enumerate() {
        elapsed += item_duration(item);
        if elapsed > measure_duration {
            return Err(musicxml_error(
                "an event crosses a measure boundary; split it explicitly before MusicXML export",
                None,
            ));
        }
        if elapsed == measure_duration {
            measures.push(&items[start..=index]);
            start = index + 1;
            elapsed = Ratio::from_integer(0);
        }
    }
    if elapsed != Ratio::from_integer(0) {
        return Err(musicxml_error(
            "bounded MusicXML export requires complete measures",
            None,
        ));
    }
    Ok(measures)
}

fn render_attributes(
    output: &mut String,
    score: &Score,
    divisions: i64,
) -> Result<(), NotationError> {
    output.push_str("      <attributes>\n");
    output.push_str(&format!("        <divisions>{divisions}</divisions>\n"));
    if let Some(key) = score.key.as_deref() {
        let (fifths, mode) =
            fifths_from_key(key).ok_or_else(|| NotationError::InvalidKey(key.to_owned()))?;
        output.push_str(&format!(
            "        <key><fifths>{fifths}</fifths><mode>{mode}</mode></key>\n"
        ));
    }
    output.push_str(&format!(
        "        <time><beats>{}</beats><beat-type>{}</beat-type></time>\n",
        score.time_signature.0, score.time_signature.1
    ));
    output.push_str("      </attributes>\n");
    Ok(())
}

fn render_item(
    output: &mut String,
    item: &MelodyItem,
    id: &str,
    divisions: i64,
    key: Option<&str>,
    path: &str,
    losses: &mut Vec<NotationLoss>,
) -> Result<(), NotationError> {
    output.push_str(&format!("      <note id=\"{}\">\n", escape_xml(id)));
    let duration = item_duration(item) * Ratio::from_integer(divisions.saturating_mul(4));
    if *duration.denom() != 1 {
        return Err(musicxml_error(
            "internal MusicXML divisions failed to preserve an exact duration",
            None,
        ));
    }
    match item {
        MelodyItem::Rest(_) => output.push_str("        <rest/>\n"),
        MelodyItem::Note(note) => {
            render_pitch(output, spell_pitch_in_key(note.pitch, key)?)?;
            if note.velocity != 100 {
                losses.push(NotationLoss {
                    kind: NotationLossKind::Velocity,
                    canonical_path: Some(path.to_owned()),
                    detail: format!(
                        "note velocity {} is not represented by the bounded MusicXML profile",
                        note.velocity
                    ),
                });
            }
            if note.channel.0 != 0 {
                losses.push(NotationLoss {
                    kind: NotationLossKind::Channel,
                    canonical_path: Some(path.to_owned()),
                    detail: format!(
                        "MIDI channel {} is not represented by the bounded MusicXML profile",
                        note.channel.0
                    ),
                });
            }
        }
    }
    output.push_str(&format!(
        "        <duration>{}</duration>\n",
        duration.numer()
    ));
    output.push_str("        <voice>1</voice>\n");
    render_note_type(output, item_duration(item))?;
    if let MelodyItem::Note(note) = item {
        render_articulation(output, note.articulation)?;
    }
    output.push_str("      </note>\n");
    Ok(())
}

fn render_pitch(output: &mut String, pitch: SpelledPitch) -> Result<(), NotationError> {
    let step = match pitch.letter {
        Letter::C => "C",
        Letter::D => "D",
        Letter::E => "E",
        Letter::F => "F",
        Letter::G => "G",
        Letter::A => "A",
        Letter::B => "B",
    };
    output.push_str("        <pitch>");
    output.push_str(&format!("<step>{step}</step>"));
    if pitch.accidental != 0 {
        output.push_str(&format!("<alter>{}</alter>", pitch.accidental));
    }
    output.push_str(&format!("<octave>{}</octave></pitch>\n", pitch.octave));
    Ok(())
}

fn render_note_type(output: &mut String, duration: Time) -> Result<(), NotationError> {
    let (name, dotted) = match (duration.numer(), duration.denom()) {
        (1, 1) => ("whole", false),
        (1, 2) => ("half", false),
        (3, 4) => ("half", true),
        (1, 4) => ("quarter", false),
        (3, 8) => ("quarter", true),
        (1, 8) => ("eighth", false),
        (3, 16) => ("eighth", true),
        (1, 16) => ("16th", false),
        (3, 32) => ("16th", true),
        (1, 32) => ("32nd", false),
        (3, 64) => ("32nd", true),
        (1, 64) => ("64th", false),
        _ => {
            return Err(NotationError::UnsupportedDuration(duration.to_string()));
        }
    };
    output.push_str(&format!("        <type>{name}</type>\n"));
    if dotted {
        output.push_str("        <dot/>\n");
    }
    Ok(())
}

fn render_articulation(
    output: &mut String,
    articulation: Articulation,
) -> Result<(), NotationError> {
    let element = match articulation {
        Articulation::Normal => return Ok(()),
        Articulation::Staccato => "staccato",
        Articulation::Tenuto => "tenuto",
        Articulation::Accent => "accent",
        Articulation::Marcato => "strong-accent",
        Articulation::Legato => {
            return Err(musicxml_error(
                "Legato requires slur identity outside the bounded MusicXML profile",
                None,
            ));
        }
    };
    output.push_str(&format!(
        "        <notations><articulations><{element}/></articulations></notations>\n"
    ));
    Ok(())
}
