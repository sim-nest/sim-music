use sim_lib_pitch_core::{Letter, Pitch, SpelledPitch};

use crate::model::NotationError;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum KeyFlavor {
    Sharps,
    Flats,
    Neutral,
}

pub(crate) fn lily_key_spec(
    key: Option<&str>,
) -> Result<Option<(String, &'static str, KeyFlavor)>, NotationError> {
    let Some(key) = key else {
        return Ok(None);
    };
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let (tonic_raw, mode) = if let Some(value) = trimmed.strip_suffix(" major") {
        (value.trim(), "major")
    } else if let Some(value) = trimmed.strip_suffix(" minor") {
        (value.trim(), "minor")
    } else if let Some(value) = trimmed.strip_suffix('m') {
        (value.trim(), "minor")
    } else {
        (trimmed, "major")
    };
    let tonic = tonic_raw.to_owned();
    let flavor = key_flavor(&tonic, mode);
    Ok(Some((to_lily_tonic(&tonic)?, mode, flavor)))
}

fn key_flavor(tonic: &str, mode: &str) -> KeyFlavor {
    let tonic = tonic.to_ascii_lowercase();
    match (tonic.as_str(), mode) {
        ("f", "major")
        | ("bb", "major")
        | ("eb", "major")
        | ("ab", "major")
        | ("db", "major")
        | ("gb", "major")
        | ("cb", "major")
        | ("d", "minor")
        | ("g", "minor")
        | ("c", "minor")
        | ("f", "minor")
        | ("bb", "minor")
        | ("eb", "minor")
        | ("ab", "minor") => KeyFlavor::Flats,
        ("c", "major") | ("a", "minor") => KeyFlavor::Neutral,
        _ => KeyFlavor::Sharps,
    }
}

fn to_lily_tonic(value: &str) -> Result<String, NotationError> {
    let normalized = value.to_ascii_lowercase();
    let lily = match normalized.as_str() {
        "c" => "c",
        "c#" => "cis",
        "db" => "des",
        "d" => "d",
        "d#" => "dis",
        "eb" => "ees",
        "e" => "e",
        "f" => "f",
        "f#" => "fis",
        "gb" => "ges",
        "g" => "g",
        "g#" => "gis",
        "ab" => "aes",
        "a" => "a",
        "a#" => "ais",
        "bb" => "bes",
        "b" => "b",
        _ => return Err(NotationError::InvalidKey(value.to_owned())),
    };
    Ok(lily.to_owned())
}

pub(crate) fn spell_pitch_in_key(
    pitch: Pitch,
    key: Option<&str>,
) -> Result<SpelledPitch, NotationError> {
    let flavor = lily_key_spec(key)?
        .map(|(_, _, flavor)| flavor)
        .unwrap_or(KeyFlavor::Neutral);
    let table = match flavor {
        KeyFlavor::Sharps | KeyFlavor::Neutral => SHARP_SPELLINGS,
        KeyFlavor::Flats => FLAT_SPELLINGS,
    };
    let (letter, accidental) = table[usize::from(pitch.class.value())];
    Ok(SpelledPitch {
        letter,
        accidental,
        octave: pitch.octave,
    })
}

pub(crate) fn encode_lily_pitch(spelled: SpelledPitch) -> String {
    let mut out = match spelled.letter {
        Letter::C => "c",
        Letter::D => "d",
        Letter::E => "e",
        Letter::F => "f",
        Letter::G => "g",
        Letter::A => "a",
        Letter::B => "b",
    }
    .to_owned();
    out.push_str(match spelled.accidental {
        -2 => "eses",
        -1 => "es",
        0 => "",
        1 => "is",
        2 => "isis",
        _ => "",
    });
    match spelled.octave.cmp(&3) {
        std::cmp::Ordering::Greater => {
            for _ in 0..(spelled.octave - 3) {
                out.push('\'');
            }
        }
        std::cmp::Ordering::Less => {
            for _ in 0..(3 - spelled.octave) {
                out.push(',');
            }
        }
        std::cmp::Ordering::Equal => {}
    }
    out
}

pub(crate) fn decode_lily_pitch(token: &str) -> Option<SpelledPitch> {
    let mut chars = token.chars();
    let letter = match chars.next()? {
        'c' => Letter::C,
        'd' => Letter::D,
        'e' => Letter::E,
        'f' => Letter::F,
        'g' => Letter::G,
        'a' => Letter::A,
        'b' => Letter::B,
        _ => return None,
    };
    let mut rest = chars.as_str();
    let mut accidental = 0i8;
    while let Some(slice) = rest.strip_prefix("isis") {
        accidental += 2;
        rest = slice;
    }
    while let Some(slice) = rest.strip_prefix("eses") {
        accidental -= 2;
        rest = slice;
    }
    while let Some(slice) = rest.strip_prefix("is") {
        accidental += 1;
        rest = slice;
    }
    while let Some(slice) = rest.strip_prefix("es") {
        accidental -= 1;
        rest = slice;
    }
    let mut octave = 3i16;
    for ch in rest.chars() {
        match ch {
            '\'' => octave += 1,
            ',' => octave -= 1,
            _ => return None,
        }
    }
    Some(SpelledPitch {
        letter,
        accidental,
        octave,
    })
}

pub(crate) fn key_from_lily(tonic: &str, mode: &str) -> Option<String> {
    let tonic = match tonic {
        "c" => "C",
        "cis" => "C#",
        "des" => "Db",
        "d" => "D",
        "dis" => "D#",
        "ees" => "Eb",
        "e" => "E",
        "f" => "F",
        "fis" => "F#",
        "ges" => "Gb",
        "g" => "G",
        "gis" => "G#",
        "aes" => "Ab",
        "a" => "A",
        "ais" => "A#",
        "bes" => "Bb",
        "b" => "B",
        _ => return None,
    };
    Some(format!("{tonic} {mode}"))
}

const SHARP_SPELLINGS: [(Letter, i8); 12] = [
    (Letter::C, 0),
    (Letter::C, 1),
    (Letter::D, 0),
    (Letter::D, 1),
    (Letter::E, 0),
    (Letter::F, 0),
    (Letter::F, 1),
    (Letter::G, 0),
    (Letter::G, 1),
    (Letter::A, 0),
    (Letter::A, 1),
    (Letter::B, 0),
];

const FLAT_SPELLINGS: [(Letter, i8); 12] = [
    (Letter::C, 0),
    (Letter::D, -1),
    (Letter::D, 0),
    (Letter::E, -1),
    (Letter::E, 0),
    (Letter::F, 0),
    (Letter::G, -1),
    (Letter::G, 0),
    (Letter::A, -1),
    (Letter::A, 0),
    (Letter::B, -1),
    (Letter::B, 0),
];
