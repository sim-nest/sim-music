use std::str::FromStr;

use thiserror::Error;

/// Error returned when a pitch, pitch class, or interval cannot be constructed
/// or parsed.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PitchError {
    /// A pitch-class value of 12 or greater was supplied where only `0..12` is valid.
    #[error("invalid pitch class {0}")]
    InvalidPitchClass(u8),
    /// A pitch spelling could not be parsed into a letter, accidental, and octave.
    #[error("invalid pitch spelling")]
    InvalidPitch,
    /// An interval spelling was not one of the recognized tokens.
    #[error("invalid interval spelling")]
    InvalidInterval,
}

/// A mod-12 pitch class, where `C = 0` and values increase by semitone to `B = 11`.
///
/// Pitch classes are octave-agnostic: every C, regardless of register, shares the
/// pitch class `C`. The inner `u8` is always in the range `0..12`.
///
/// # Examples
///
/// ```
/// use sim_lib_pitch_core::PitchClass;
///
/// assert_eq!(PitchClass::C.transpose(7), PitchClass::G);
/// assert_eq!(PitchClass::E.interval_class(PitchClass::C), 4);
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PitchClass(pub u8);

impl PitchClass {
    /// The pitch class C (0).
    pub const C: Self = Self(0);
    /// The pitch class C-sharp / D-flat (1).
    pub const CS: Self = Self(1);
    /// The pitch class D (2).
    pub const D: Self = Self(2);
    /// The pitch class D-sharp / E-flat (3).
    pub const DS: Self = Self(3);
    /// The pitch class E (4).
    pub const E: Self = Self(4);
    /// The pitch class F (5).
    pub const F: Self = Self(5);
    /// The pitch class F-sharp / G-flat (6).
    pub const FS: Self = Self(6);
    /// The pitch class G (7).
    pub const G: Self = Self(7);
    /// The pitch class G-sharp / A-flat (8).
    pub const GS: Self = Self(8);
    /// The pitch class A (9).
    pub const A: Self = Self(9);
    /// The pitch class A-sharp / B-flat (10).
    pub const AS: Self = Self(10);
    /// The pitch class B (11).
    pub const B: Self = Self(11);

    /// Constructs a pitch class from a raw value, rejecting values of 12 or more.
    pub fn new(value: u8) -> Result<Self, PitchError> {
        if value < 12 {
            Ok(Self(value))
        } else {
            Err(PitchError::InvalidPitchClass(value))
        }
    }

    /// Returns this pitch class shifted up by `semitones` (or down if negative),
    /// wrapping within the mod-12 octave.
    pub fn transpose(self, semitones: i32) -> Self {
        Self(((self.0 as i32 + semitones).rem_euclid(12)) as u8)
    }

    /// Returns the inversion of this pitch class about `axis`, wrapping within the
    /// mod-12 octave.
    pub fn invert(self, axis: PitchClass) -> Self {
        Self(((2 * axis.0 as i32 - self.0 as i32).rem_euclid(12)) as u8)
    }

    /// Returns the interval class (0..=6) between this pitch class and `other`,
    /// the smaller of the ascending and descending distances.
    pub fn interval_class(self, other: PitchClass) -> u8 {
        let delta = (other.0 as i32 - self.0 as i32).rem_euclid(12) as u8;
        delta.min(12 - delta)
    }

    /// Returns the canonical sharp-spelled name of this pitch class (for example
    /// `"C#"` for pitch class 1).
    pub fn canonical_name(self) -> &'static str {
        match self.0 {
            0 => "C",
            1 => "C#",
            2 => "D",
            3 => "D#",
            4 => "E",
            5 => "F",
            6 => "F#",
            7 => "G",
            8 => "G#",
            9 => "A",
            10 => "A#",
            11 => "B",
            _ => unreachable!(),
        }
    }
}

/// An octave-aware pitch: a [`PitchClass`] together with an octave number.
///
/// The octave follows the MIDI convention in which middle C (`C4`) is MIDI note
/// 60, so [`Pitch::semitone`] returns a continuous semitone index across octaves.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pitch {
    /// The mod-12 pitch class.
    pub class: PitchClass,
    /// The octave number, with `C4` (MIDI 60) in octave 4.
    pub octave: i16,
}

impl Pitch {
    /// Returns the absolute semitone index of this pitch, where MIDI 60 (`C4`) is 60.
    pub fn semitone(self) -> i32 {
        (self.octave as i32 + 1) * 12 + self.class.0 as i32
    }

    /// Constructs a pitch from an absolute semitone index, the inverse of
    /// [`Pitch::semitone`].
    pub fn from_semitone(semitone: i32) -> Self {
        Self {
            class: PitchClass(semitone.rem_euclid(12) as u8),
            octave: (semitone.div_euclid(12) - 1) as i16,
        }
    }

    /// Returns the MIDI note number for this pitch, or `None` if it falls outside
    /// the playable range `0..=127`.
    pub fn to_midi(self) -> Option<u8> {
        let semitone = self.semitone();
        (0..=127).contains(&semitone).then_some(semitone as u8)
    }

    /// Constructs a pitch from a MIDI note number.
    pub fn from_midi(value: u8) -> Self {
        Self::from_semitone(value as i32)
    }

    /// Returns this pitch shifted by `semitones`, preserving the MIDI mapping.
    pub fn transpose(self, semitones: i32) -> Self {
        Self::from_semitone(self.semitone() + semitones)
    }

    /// Returns the inversion of this pitch about `axis`.
    pub fn invert(self, axis: Pitch) -> Self {
        Self::from_semitone(2 * axis.semitone() - self.semitone())
    }
}

/// A diatonic letter name, independent of accidental.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Letter {
    /// The letter C.
    C,
    /// The letter D.
    D,
    /// The letter E.
    E,
    /// The letter F.
    F,
    /// The letter G.
    G,
    /// The letter A.
    A,
    /// The letter B.
    B,
}

/// A spelled pitch: a diatonic [`Letter`], a chromatic accidental, and an octave.
///
/// Unlike [`Pitch`], a spelled pitch retains its enharmonic spelling, so `Cs4`
/// and `Db4` are distinct even though they map to the same [`Pitch`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SpelledPitch {
    /// The diatonic letter name.
    pub letter: Letter,
    /// The accidental offset in semitones (positive for sharps, negative for flats).
    pub accidental: i8,
    /// The octave number, following the MIDI convention.
    pub octave: i16,
}

impl SpelledPitch {
    /// Resolves this spelled pitch to its octave-aware [`Pitch`], discarding the
    /// enharmonic spelling.
    pub fn to_pitch(self) -> Pitch {
        let base = match self.letter {
            Letter::C => 0,
            Letter::D => 2,
            Letter::E => 4,
            Letter::F => 5,
            Letter::G => 7,
            Letter::A => 9,
            Letter::B => 11,
        };
        Pitch {
            class: PitchClass((base + self.accidental as i32).rem_euclid(12) as u8),
            octave: self.octave,
        }
    }
}

/// A pitch interval measured in semitones.
///
/// Positive values are ascending and negative values descending. The signed
/// distance is preserved; use [`Interval::class`] to collapse to an interval class.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Interval {
    /// The signed distance in semitones.
    pub semitones: i32,
}

impl Interval {
    /// The perfect unison (0 semitones).
    pub const UNISON: Self = Self { semitones: 0 };
    /// The minor third (3 semitones).
    pub const MINOR_3: Self = Self { semitones: 3 };
    /// The major third (4 semitones).
    pub const MAJOR_3: Self = Self { semitones: 4 };
    /// The perfect fifth (7 semitones).
    pub const PERFECT_5: Self = Self { semitones: 7 };
    /// The tritone (6 semitones).
    pub const TRITONE: Self = Self { semitones: 6 };
    /// The major seventh (11 semitones).
    pub const MAJOR_7: Self = Self { semitones: 11 };

    /// Returns the directed interval from `a` to `b`.
    pub fn between(a: Pitch, b: Pitch) -> Self {
        Self {
            semitones: b.semitone() - a.semitone(),
        }
    }

    /// Returns the interval class (0..=6) of this interval, the smaller of the
    /// ascending and descending mod-12 distances.
    pub fn class(self) -> u8 {
        let delta = self.semitones.rem_euclid(12) as u8;
        delta.min(12 - delta)
    }
}

impl FromStr for Pitch {
    type Err = PitchError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        parse_pitch(value)
    }
}

impl FromStr for Interval {
    type Err = PitchError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        parse_interval(value)
    }
}

/// Parses a pitch spelling such as `"C4"`, `"Eb5"`, or `"Cs4"` into a [`Pitch`].
///
/// Accidentals accept `#` or `s` for sharp and `b` for flat; an octave number is
/// required. Returns [`PitchError::InvalidPitch`] on malformed input.
///
/// # Examples
///
/// ```
/// use sim_lib_pitch_core::{parse_pitch, Pitch};
///
/// assert_eq!(parse_pitch("Eb5").unwrap(), Pitch::from_semitone(75));
/// ```
pub fn parse_pitch(value: &str) -> Result<Pitch, PitchError> {
    let mut chars = value.chars();
    let letter = match chars.next() {
        Some('C') => Letter::C,
        Some('D') => Letter::D,
        Some('E') => Letter::E,
        Some('F') => Letter::F,
        Some('G') => Letter::G,
        Some('A') => Letter::A,
        Some('B') => Letter::B,
        _ => return Err(PitchError::InvalidPitch),
    };
    let rest = chars.as_str();
    let (accidental, octave_str) = if let Some(rest) = rest.strip_prefix('#') {
        (1, rest)
    } else if let Some(rest) = rest.strip_prefix('s') {
        (1, rest)
    } else if let Some(rest) = rest.strip_prefix('b') {
        (-1, rest)
    } else {
        (0, rest)
    };
    if octave_str.is_empty() {
        return Err(PitchError::InvalidPitch);
    }
    let octave = octave_str
        .parse::<i16>()
        .map_err(|_| PitchError::InvalidPitch)?;
    Ok(SpelledPitch {
        letter,
        accidental,
        octave,
    }
    .to_pitch())
}

/// Parses one of the recognized interval tokens (`"P5"`, `"m3"`, `"M7"`, `"TT"`)
/// into an [`Interval`].
///
/// Returns [`PitchError::InvalidInterval`] for any unrecognized token.
pub fn parse_interval(value: &str) -> Result<Interval, PitchError> {
    match value {
        "P5" => Ok(Interval::PERFECT_5),
        "m3" => Ok(Interval::MINOR_3),
        "M7" => Ok(Interval::MAJOR_7),
        "TT" => Ok(Interval::TRITONE),
        _ => Err(PitchError::InvalidInterval),
    }
}
