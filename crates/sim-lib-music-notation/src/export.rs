use num_rational::Ratio;

use sim_lib_music_core::{Chord, Counterpoint, Melody, MelodyItem, Music, Note, Rest, Score, Time};

use crate::{
    model::{NotationError, NotationReport},
    spell::{encode_lily_pitch, lily_key_spec, spell_pitch_in_key},
};

/// Renders a score as a LilyPond `\score` block, returning the text with diagnostics.
pub fn export_lilypond_report(score: &Score) -> Result<NotationReport<String>, NotationError> {
    let mut lines = vec!["\\score {".to_owned()];
    lines.push(format!("  \\tempo 4 = {}", score.tempo_bpm));
    if let Some((tonic, mode, _)) = lily_key_spec(score.key.as_deref())? {
        lines.push(format!("  \\key {tonic} \\{mode}"));
    }
    lines.push(format!(
        "  \\time {}/{}",
        score.time_signature.0, score.time_signature.1
    ));
    lines.push(indent_block(
        &render_music(&score.body, score.key.as_deref())?,
        2,
    ));
    lines.push("}".to_owned());
    Ok(NotationReport {
        value: lines.join("\n"),
        diagnostics: Vec::new(),
    })
}

/// Renders a score as a LilyPond `\score` block, discarding diagnostics.
pub fn export_lilypond(score: &Score) -> Result<String, NotationError> {
    Ok(export_lilypond_report(score)?.value)
}

/// Renders a melody as a LilyPond note sequence, spelling pitches in `key`.
pub fn export_melody_lilypond(melody: &Melody, key: Option<&str>) -> Result<String, NotationError> {
    render_melody(melody, key)
}

/// Renders counterpoint as parallel LilyPond voices, spelling pitches in `key`.
pub fn export_counterpoint_lilypond(
    counterpoint: &Counterpoint,
    key: Option<&str>,
) -> Result<String, NotationError> {
    render_counterpoint(counterpoint, key)
}

/// Renders a chord progression as a LilyPond chord sequence, spelling pitches in `key`.
pub fn export_progression_lilypond(
    progression: &sim_lib_music_core::Progression,
    key: Option<&str>,
) -> Result<String, NotationError> {
    render_progression(progression, key)
}

fn render_music(value: &Music, key: Option<&str>) -> Result<String, NotationError> {
    match value {
        Music::Note(note) => render_melody(
            &Melody {
                items: vec![MelodyItem::Note(note.clone())],
            },
            key,
        ),
        Music::Rest(rest) => render_melody(
            &Melody {
                items: vec![MelodyItem::Rest(rest.clone())],
            },
            key,
        ),
        Music::Chord(chord) => render_progression(
            &sim_lib_music_core::Progression {
                key: key.map(str::to_owned),
                chords: vec![chord.clone()],
            },
            key,
        ),
        Music::Melody(melody) => render_melody(melody, key),
        Music::Progression(progression) => render_progression(progression, key),
        Music::Counterpoint(counterpoint) => render_counterpoint(counterpoint, key),
        _ => Err(NotationError::UnsupportedMusicObject(match value {
            Music::Par(_) => "Par",
            Music::Seq(_) => "Seq",
            Music::PianoRoll(_) => "PianoRoll",
            Music::MidiTrack(_) => "MidiTrack",
            Music::MidiFile(_) => "MidiFile",
            _ => "Unknown",
        })),
    }
}

fn render_melody(melody: &Melody, key: Option<&str>) -> Result<String, NotationError> {
    let mut tokens = Vec::with_capacity(melody.items.len());
    for item in &melody.items {
        match item {
            MelodyItem::Note(note) => tokens.push(render_note(note, key)?),
            MelodyItem::Rest(rest) => tokens.push(render_rest(rest)?),
        }
    }
    Ok(format!("{{ {} }}", tokens.join(" ")))
}

fn render_progression(
    progression: &sim_lib_music_core::Progression,
    key: Option<&str>,
) -> Result<String, NotationError> {
    let key = progression.key.as_deref().or(key);
    let mut tokens = Vec::with_capacity(progression.chords.len());
    for chord in &progression.chords {
        tokens.push(render_chord(chord, key)?);
    }
    Ok(format!("{{ {} }}", tokens.join(" ")))
}

fn render_counterpoint(
    counterpoint: &Counterpoint,
    key: Option<&str>,
) -> Result<String, NotationError> {
    let mut voices = Vec::with_capacity(counterpoint.voices.len());
    for (index, melody) in counterpoint.voices.iter().enumerate() {
        let name = counterpoint
            .voice_names
            .get(index)
            .cloned()
            .unwrap_or_else(|| format!("Voice {}", index + 1));
        voices.push(format!(
            "\\new Voice = \"{}\" {}",
            name.replace('"', "\\\""),
            render_melody(melody, key)?,
        ));
    }
    Ok(format!("<< {} >>", voices.join(" ")))
}

fn render_note(note: &Note, key: Option<&str>) -> Result<String, NotationError> {
    let pitch = encode_lily_pitch(spell_pitch_in_key(note.pitch, key)?);
    render_tied_segments(
        duration_segments(note.duration)?,
        |denom| format!("{pitch}{denom}"),
        true,
    )
}

fn render_rest(rest: &Rest) -> Result<String, NotationError> {
    render_tied_segments(
        duration_segments(rest.duration)?,
        |denom| format!("r{denom}"),
        false,
    )
}

fn render_chord(chord: &Chord, key: Option<&str>) -> Result<String, NotationError> {
    let pitches = chord
        .pitches
        .iter()
        .map(|pitch| spell_pitch_in_key(*pitch, key).map(encode_lily_pitch))
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    render_tied_segments(
        duration_segments(chord.duration)?,
        |denom| format!("<{pitches}>{denom}"),
        true,
    )
}

fn render_tied_segments(
    segments: Vec<Time>,
    render: impl Fn(u64) -> String,
    use_ties: bool,
) -> Result<String, NotationError> {
    let mut out = Vec::with_capacity(segments.len());
    for segment in segments {
        let denom = lily_duration_number(segment)?;
        out.push(render(denom));
    }
    if use_ties {
        Ok(out.join(" ~ "))
    } else {
        Ok(out.join(" "))
    }
}

fn duration_segments(duration: Time) -> Result<Vec<Time>, NotationError> {
    if duration <= Time::from_integer(0) {
        return Err(NotationError::UnsupportedDuration(format_ratio(duration)));
    }
    let mut reduced = duration.denom().abs();
    while reduced % 2 == 0 {
        reduced /= 2;
    }
    if reduced != 1 {
        return Err(NotationError::UnsupportedDuration(format_ratio(duration)));
    }
    let mut remaining = duration;
    let mut segments = Vec::new();
    while remaining > Time::from_integer(0) {
        let segment = largest_dyadic_leq(remaining);
        remaining -= segment;
        segments.push(segment);
    }
    Ok(segments)
}

fn largest_dyadic_leq(limit: Time) -> Time {
    let mut candidate = Time::from_integer(1);
    while candidate > limit {
        candidate /= Ratio::from_integer(2);
    }
    candidate
}

fn lily_duration_number(segment: Time) -> Result<u64, NotationError> {
    if *segment.numer() != 1 {
        return Err(NotationError::UnsupportedDuration(format_ratio(segment)));
    }
    u64::try_from(*segment.denom())
        .map_err(|_| NotationError::UnsupportedDuration(format_ratio(segment)))
}

fn format_ratio(value: Time) -> String {
    format!("{}/{}", value.numer(), value.denom())
}

fn indent_block(value: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    value
        .lines()
        .map(|line| format!("{pad}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
