//! Note/rest parsing for the bounded MusicXML profile.

use num_rational::Ratio;
use roxmltree::Node;
use sim_lib_music_core::{Articulation, Channel, MelodyItem, Note, Rest, Time};
use sim_lib_pitch_core::{Letter, Pitch, SpelledPitch};

use crate::{model::NotationError, musicxml_support::*, spell::spell_pitch_in_key};

pub(crate) fn parse_note(
    node: Node<'_, '_>,
    divisions: i64,
    default_id: String,
    key: Option<&str>,
) -> Result<((MelodyItem, Option<String>), String), NotationError> {
    ensure_attrs(node, &["id"])?;
    ensure_children(
        node,
        &[
            "pitch",
            "rest",
            "duration",
            "voice",
            "type",
            "dot",
            "notations",
        ],
    )?;
    let id = node
        .attribute("id")
        .map(str::to_owned)
        .unwrap_or(default_id);
    if let Some(voice) = unique_optional_child(node, "voice")?
        && required_text(voice)? != "1"
    {
        return Err(node_error(
            voice,
            "bounded partwise profile supports one voice per part",
        ));
    }
    let duration_node = unique_child(node, "duration")?;
    let duration_units =
        parse_positive_i64(required_text(duration_node)?, duration_node, "duration")?;
    let duration = Ratio::new(duration_units, divisions.saturating_mul(4));
    validate_type_duration(node, duration)?;
    let articulation = parse_articulation(node)?;
    let pitch = unique_optional_child(node, "pitch")?;
    let rest = unique_optional_child(node, "rest")?;
    let (item, spelling_loss) = match (pitch, rest) {
        (Some(pitch), None) => {
            let (pitch, source_spelling) = parse_pitch(pitch)?;
            let canonical = spell_pitch_in_key(pitch, key)?;
            let spelling_loss = (source_spelling != canonical).then(|| {
                format!(
                    "MusicXML pitch spelling {} is canonicalized to {} for the active key",
                    display_spelling(source_spelling),
                    display_spelling(canonical),
                )
            });
            (
                MelodyItem::Note(Note::new(
                    duration,
                    pitch,
                    100,
                    Channel::new(0).expect("channel zero is valid"),
                    articulation,
                )?),
                spelling_loss,
            )
        }
        (None, Some(rest)) => {
            ensure_attrs(rest, &[])?;
            ensure_children(rest, &[])?;
            if articulation != Articulation::Normal {
                return Err(node_error(
                    node,
                    "rest events cannot carry articulations in the bounded profile",
                ));
            }
            (MelodyItem::Rest(Rest::new(duration)?), None)
        }
        _ => {
            return Err(node_error(
                node,
                "note must contain exactly one pitch or rest element",
            ));
        }
    };
    Ok(((item, spelling_loss), id))
}

fn parse_pitch(node: Node<'_, '_>) -> Result<(Pitch, SpelledPitch), NotationError> {
    ensure_attrs(node, &[])?;
    ensure_children(node, &["step", "alter", "octave"])?;
    let step = required_text(unique_child(node, "step")?)?;
    let (base, letter) = match step {
        "C" => (0, Letter::C),
        "D" => (2, Letter::D),
        "E" => (4, Letter::E),
        "F" => (5, Letter::F),
        "G" => (7, Letter::G),
        "A" => (9, Letter::A),
        "B" => (11, Letter::B),
        _ => return Err(node_error(node, "pitch step must be A through G")),
    };
    let alter = unique_optional_child(node, "alter")?
        .map(|value| {
            required_text(value)?
                .parse::<i8>()
                .map_err(|_| node_error(value, "pitch alter must be an integer"))
        })
        .transpose()?
        .unwrap_or(0);
    if !(-2..=2).contains(&alter) {
        return Err(node_error(node, "pitch alter must be between -2 and 2"));
    }
    let octave = required_text(unique_child(node, "octave")?)?
        .parse::<i16>()
        .map_err(|_| node_error(node, "pitch octave must be an integer"))?;
    Ok((
        Pitch::from_semitone((i32::from(octave) + 1) * 12 + base + i32::from(alter)),
        SpelledPitch {
            letter,
            accidental: alter,
            octave,
        },
    ))
}

fn display_spelling(pitch: SpelledPitch) -> String {
    let letter = match pitch.letter {
        Letter::C => "C",
        Letter::D => "D",
        Letter::E => "E",
        Letter::F => "F",
        Letter::G => "G",
        Letter::A => "A",
        Letter::B => "B",
    };
    let accidental = match pitch.accidental {
        -2 => "bb",
        -1 => "b",
        0 => "",
        1 => "#",
        2 => "##",
        _ => "?",
    };
    format!("{letter}{accidental}{}", pitch.octave)
}

fn parse_articulation(node: Node<'_, '_>) -> Result<Articulation, NotationError> {
    let Some(notations) = unique_optional_child(node, "notations")? else {
        return Ok(Articulation::Normal);
    };
    ensure_attrs(notations, &[])?;
    ensure_children(notations, &["articulations"])?;
    let articulations = unique_child(notations, "articulations")?;
    ensure_attrs(articulations, &[])?;
    ensure_children(
        articulations,
        &["staccato", "tenuto", "accent", "strong-accent"],
    )?;
    let values = articulations
        .children()
        .filter(Node::is_element)
        .collect::<Vec<_>>();
    if values.len() != 1 {
        return Err(node_error(
            articulations,
            "bounded profile accepts exactly one articulation",
        ));
    }
    ensure_attrs(values[0], &[])?;
    ensure_children(values[0], &[])?;
    Ok(match values[0].tag_name().name() {
        "staccato" => Articulation::Staccato,
        "tenuto" => Articulation::Tenuto,
        "accent" => Articulation::Accent,
        "strong-accent" => Articulation::Marcato,
        _ => unreachable!("articulation vocabulary checked above"),
    })
}

fn validate_type_duration(node: Node<'_, '_>, duration: Time) -> Result<(), NotationError> {
    let Some(note_type) = unique_optional_child(node, "type")? else {
        if optional_child(node, "dot").is_some() {
            return Err(node_error(node, "dot requires a note type"));
        }
        return Ok(());
    };
    let base = match required_text(note_type)? {
        "whole" => Ratio::new(1, 1),
        "half" => Ratio::new(1, 2),
        "quarter" => Ratio::new(1, 4),
        "eighth" => Ratio::new(1, 8),
        "16th" => Ratio::new(1, 16),
        "32nd" => Ratio::new(1, 32),
        "64th" => Ratio::new(1, 64),
        _ => return Err(node_error(note_type, "unsupported MusicXML note type")),
    };
    let dots = children_named(node, "dot").count();
    if dots > 1 {
        return Err(node_error(node, "bounded profile accepts at most one dot"));
    }
    let expected = if dots == 1 {
        base * Ratio::new(3, 2)
    } else {
        base
    };
    if duration != expected {
        return Err(node_error(
            node,
            "MusicXML duration and type/dot notation disagree",
        ));
    }
    Ok(())
}
