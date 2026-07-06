use thiserror::Error;

use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_pitch_set::PitchClassMask;

/// Error returned when a scale operation fails or a scale cannot be constructed.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PitchScaleError {
    /// A pitch class was requested as a scale degree but is not part of the scale.
    #[error("pitch class {0} is not in the scale")]
    PitchClassOutOfScale(u8),
    /// A custom scale was built from an empty interval list.
    #[error("scale must contain at least one pitch class")]
    EmptyScale,
    /// A custom scale interval fell outside the valid `0..12` semitone range.
    #[error("scale interval {0} is outside 0..12")]
    InvalidScaleInterval(u8),
}

/// A scale mode, defined by its semitone interval pattern from the tonic.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// The Ionian / major scale.
    Major,
    /// The natural minor scale (equivalent to [`Mode::Aeolian`]).
    MinorNatural,
    /// The harmonic minor scale (raised seventh).
    MinorHarmonic,
    /// The ascending melodic minor scale (raised sixth and seventh).
    MinorMelodic,
    /// The Dorian mode.
    Dorian,
    /// The Phrygian mode.
    Phrygian,
    /// The Lydian mode.
    Lydian,
    /// The Mixolydian mode.
    Mixolydian,
    /// The Aeolian mode (equivalent to [`Mode::MinorNatural`]).
    Aeolian,
    /// The Locrian mode.
    Locrian,
    /// The symmetric whole-tone scale (six notes).
    WholeTone,
    /// The symmetric octatonic / diminished scale (eight notes).
    Diminished,
    /// The full chromatic scale (all twelve pitch classes).
    Chromatic,
}

impl Mode {
    /// Returns the mode's semitone offsets from the tonic, in ascending order.
    pub fn intervals(self) -> &'static [u8] {
        match self {
            Self::Major => &[0, 2, 4, 5, 7, 9, 11],
            Self::MinorNatural | Self::Aeolian => &[0, 2, 3, 5, 7, 8, 10],
            Self::MinorHarmonic => &[0, 2, 3, 5, 7, 8, 11],
            Self::MinorMelodic => &[0, 2, 3, 5, 7, 9, 11],
            Self::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Self::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Self::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            Self::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            Self::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            Self::WholeTone => &[0, 2, 4, 6, 8, 10],
            Self::Diminished => &[0, 2, 3, 5, 6, 8, 9, 11],
            Self::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        }
    }

    /// Returns the canonical lowercase name of the mode (for example `"dorian"`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Major => "major",
            Self::MinorNatural => "minor-natural",
            Self::MinorHarmonic => "minor-harmonic",
            Self::MinorMelodic => "minor-melodic",
            Self::Dorian => "dorian",
            Self::Phrygian => "phrygian",
            Self::Lydian => "lydian",
            Self::Mixolydian => "mixolydian",
            Self::Aeolian => "aeolian",
            Self::Locrian => "locrian",
            Self::WholeTone => "whole-tone",
            Self::Diminished => "diminished",
            Self::Chromatic => "chromatic",
        }
    }
}

/// A musical key: a tonic pitch class paired with a [`Mode`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    /// The tonic pitch class.
    pub tonic: PitchClass,
    /// The mode of the key.
    pub mode: Mode,
}

/// A concrete scale: a [`Mode`] anchored to a tonic pitch class.
///
/// # Examples
///
/// ```
/// use sim_lib_pitch_core::PitchClass;
/// use sim_lib_pitch_scale::Scale;
///
/// let c_major = Scale::major(PitchClass::C);
/// assert_eq!(c_major.degree_of(PitchClass::G), Some(5));
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Scale {
    /// The tonic pitch class.
    pub tonic: PitchClass,
    /// The mode of the scale.
    pub mode: Mode,
}

impl Scale {
    /// Constructs a scale from a `tonic` and `mode`.
    pub const fn new(tonic: PitchClass, mode: Mode) -> Self {
        Self { tonic, mode }
    }

    /// Constructs a major scale on `tonic`.
    pub const fn major(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::Major)
    }
    /// Constructs a natural minor scale on `tonic`.
    pub const fn minor_natural(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::MinorNatural)
    }
    /// Constructs a harmonic minor scale on `tonic`.
    pub const fn minor_harmonic(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::MinorHarmonic)
    }
    /// Constructs a melodic minor scale on `tonic`.
    pub const fn minor_melodic(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::MinorMelodic)
    }
    /// Constructs a Dorian scale on `tonic`.
    pub const fn dorian(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::Dorian)
    }
    /// Constructs a Phrygian scale on `tonic`.
    pub const fn phrygian(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::Phrygian)
    }
    /// Constructs a Lydian scale on `tonic`.
    pub const fn lydian(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::Lydian)
    }
    /// Constructs a Mixolydian scale on `tonic`.
    pub const fn mixolydian(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::Mixolydian)
    }
    /// Constructs an Aeolian scale on `tonic`.
    pub const fn aeolian(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::Aeolian)
    }
    /// Constructs a Locrian scale on `tonic`.
    pub const fn locrian(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::Locrian)
    }
    /// Constructs a whole-tone scale on `tonic`.
    pub const fn whole_tone(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::WholeTone)
    }
    /// Constructs a diminished (octatonic) scale on `tonic`.
    pub const fn diminished(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::Diminished)
    }
    /// Constructs a chromatic scale on `tonic`.
    pub const fn chromatic(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::Chromatic)
    }

    /// Returns the scale's pitch classes in ascending degree order from the tonic.
    pub fn pitch_classes(self) -> Vec<PitchClass> {
        self.mode
            .intervals()
            .iter()
            .map(|step| self.tonic.transpose(i32::from(*step)))
            .collect()
    }

    /// Returns the scale's pitch classes as a [`PitchClassMask`].
    pub fn mask(self) -> PitchClassMask {
        PitchClassMask::from_pitch_classes(&self.pitch_classes())
    }

    /// Returns the one-based scale degree of `pitch_class`, or `None` if it is not
    /// in the scale.
    pub fn degree_of(self, pitch_class: PitchClass) -> Option<usize> {
        self.pitch_classes()
            .iter()
            .position(|candidate| *candidate == pitch_class)
            .map(|index| index + 1)
    }

    /// Returns the pitch class at the one-based `degree`, wrapping past the octave.
    pub fn pitch_at_degree(self, degree: usize) -> PitchClass {
        let intervals = self.mode.intervals();
        let index = degree.saturating_sub(1) % intervals.len();
        self.tonic.transpose(i32::from(intervals[index]))
    }

    /// Transposes `pitch` by `steps` scale degrees, staying within the scale and
    /// adjusting octaves as needed.
    ///
    /// Returns [`PitchScaleError::PitchClassOutOfScale`] if `pitch` is not a member
    /// of the scale.
    pub fn transpose_diatonic(self, pitch: Pitch, steps: i32) -> Result<Pitch, PitchScaleError> {
        let degree = self
            .degree_of(pitch.class)
            .ok_or(PitchScaleError::PitchClassOutOfScale(pitch.class.0))?;
        let intervals = self.mode.intervals();
        let start = degree as i32 - 1;
        let target = start + steps;
        let width = intervals.len() as i32;
        let octave_delta = target.div_euclid(width);
        let target_index = target.rem_euclid(width) as usize;
        let start_semitones = i32::from(intervals[start as usize]);
        let end_semitones = i32::from(intervals[target_index]) + octave_delta * 12;
        Ok(pitch.transpose(end_semitones - start_semitones))
    }

    /// Maps a one-based chord-tone index (root, third, fifth, ...) to the
    /// one-based scale degree it occupies in tertian stacking.
    pub fn chord_tone_to_scale_tone(chord_tone: usize) -> usize {
        1 + chord_tone.saturating_sub(1) * 2
    }

    /// Maps a one-based scale degree to its zero-based diatonic step offset from
    /// the tonic.
    pub fn scale_tone_to_diatonic(scale_tone: usize) -> i32 {
        scale_tone.saturating_sub(1) as i32
    }
}
