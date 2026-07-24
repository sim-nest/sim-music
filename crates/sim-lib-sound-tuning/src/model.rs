use std::str::FromStr;

use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_sound_core::Frequency;
use thiserror::Error;

/// Error raised by tuning construction and conversion routines.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum SoundTuningError {
    /// The number of octave divisions was zero.
    #[error("tuning divisions must be positive")]
    InvalidDivisions,
    /// A degree index fell outside the range of its equal division of the
    /// octave.
    #[error("pitch-class index {index} is out of range for {divisions}-EDO")]
    InvalidPitchClassN {
        /// Number of equal divisions of the octave.
        divisions: u32,
        /// Offending degree index.
        index: u32,
    },
    /// A 12-chroma-only operation received a non-12-division degree.
    #[error("12-chroma-only operation received PitchClassN({divisions}, {index})")]
    TwelveChromaOnly {
        /// Number of equal divisions of the octave.
        divisions: u32,
        /// Degree index that could not be mapped to a 12-tone chroma.
        index: u32,
    },
    /// A Scala scale parsed to zero degrees.
    #[error("scala scale is empty")]
    EmptyScala,
    /// Scala input could not be parsed.
    #[error("invalid scala data")]
    InvalidScala,
    /// A reference frequency was zero, negative, or non-finite.
    #[error("reference frequency must be positive")]
    InvalidReferenceFrequency,
}

/// A pitch-class degree within an arbitrary equal division of the octave.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PitchClassN {
    /// Number of equal divisions of the octave (e.g. 12 for standard chroma).
    pub divisions: u32,
    /// Degree index in `0..divisions`.
    pub index: u32,
}

impl PitchClassN {
    /// Builds a degree, rejecting zero divisions and out-of-range indices.
    pub fn new(divisions: u32, index: u32) -> Result<Self, SoundTuningError> {
        if divisions == 0 {
            return Err(SoundTuningError::InvalidDivisions);
        }
        if index >= divisions {
            return Err(SoundTuningError::InvalidPitchClassN { divisions, index });
        }
        Ok(Self { divisions, index })
    }

    /// Returns the degree shifted by `steps`, wrapping within the octave.
    pub fn transpose(self, steps: i32) -> Self {
        Self {
            divisions: self.divisions,
            index: ((self.index as i32 + steps).rem_euclid(self.divisions as i32)) as u32,
        }
    }

    /// Converts a 12-division degree to a standard [`PitchClass`], failing for
    /// any other division count.
    pub fn to_pitch_class(self) -> Result<PitchClass, SoundTuningError> {
        if self.divisions == 12 {
            PitchClass::new(self.index as u8).map_err(|_| SoundTuningError::TwelveChromaOnly {
                divisions: self.divisions,
                index: self.index,
            })
        } else {
            Err(SoundTuningError::TwelveChromaOnly {
                divisions: self.divisions,
                index: self.index,
            })
        }
    }
}

/// A tuning system mapping pitches to and from concrete frequencies.
pub trait Tuning: Send + Sync {
    /// Returns the stable identifier of this tuning system.
    fn name(&self) -> &'static str;

    /// Returns the anchor pitch and its frequency.
    fn reference(&self) -> (Pitch, Frequency);

    /// Returns the frequency assigned to `pitch`.
    fn frequency_of(&self, pitch: Pitch) -> Frequency;

    /// Returns the pitch nearest to `frequency`.
    fn pitch_of(&self, frequency: Frequency) -> Pitch;

    /// Returns the number of equal divisions of the octave (12 by default).
    fn divisions(&self) -> u32 {
        12
    }

    /// Returns the frequency of `degree` in the given `octave`.
    fn frequency_of_degree(
        &self,
        degree: PitchClassN,
        octave: i16,
    ) -> Result<Frequency, SoundTuningError>;

    /// Returns the tuning degree corresponding to `pitch`.
    fn degree_of_pitch(&self, pitch: Pitch) -> Result<PitchClassN, SoundTuningError> {
        if self.divisions() == 12 {
            PitchClassN::new(12, u32::from(pitch.class.value()))
        } else {
            PitchClassN::new(
                self.divisions(),
                map_pitch_class_to_degree(self.divisions(), pitch.class),
            )
        }
    }

    /// Returns the pitch for `degree` in the given `octave`.
    fn pitch_from_degree(
        &self,
        degree: PitchClassN,
        octave: i16,
    ) -> Result<Pitch, SoundTuningError> {
        Ok(Pitch {
            class: degree.to_pitch_class()?,
            octave,
        })
    }
}

/// An equal-temperament tuning that divides the octave into equal steps.
///
/// # Examples
///
/// ```
/// use sim_lib_pitch_core::Pitch;
/// use sim_lib_sound_tuning::{EqualTemperament, Tuning};
///
/// let tuning = EqualTemperament::default();
/// let a4 = tuning.frequency_of(Pitch::from_midi(69));
/// assert!((a4.0 - 440.0).abs() < 1e-6);
/// ```
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EqualTemperament {
    /// Number of equal divisions of the octave.
    pub divisions: u32,
    /// Anchor pitch and its frequency.
    pub reference: (Pitch, Frequency),
}

impl EqualTemperament {
    /// Builds an equal-temperament tuning, rejecting zero divisions.
    pub fn new(divisions: u32, reference: (Pitch, Frequency)) -> Result<Self, SoundTuningError> {
        if divisions == 0 {
            return Err(SoundTuningError::InvalidDivisions);
        }
        let reference_frequency = Frequency::new(reference.1.0)
            .map_err(|_| SoundTuningError::InvalidReferenceFrequency)?;
        Ok(Self {
            divisions,
            reference: (reference.0, reference_frequency),
        })
    }
}

impl Default for EqualTemperament {
    fn default() -> Self {
        Self {
            divisions: 12,
            reference: (Pitch::from_midi(69), Frequency(440.0)),
        }
    }
}

impl Tuning for EqualTemperament {
    fn name(&self) -> &'static str {
        "equal-temperament"
    }

    fn reference(&self) -> (Pitch, Frequency) {
        self.reference
    }

    fn frequency_of(&self, pitch: Pitch) -> Frequency {
        let steps = pitch.semitone() - self.reference.0.semitone();
        self.reference.1.shift_cents(steps as f64 * 100.0)
    }

    fn pitch_of(&self, frequency: Frequency) -> Pitch {
        let cents = frequency.cents_above(self.reference.1);
        let steps = (cents / 100.0).round() as i32;
        self.reference.0.transpose(steps)
    }

    fn divisions(&self) -> u32 {
        self.divisions
    }

    fn frequency_of_degree(
        &self,
        degree: PitchClassN,
        octave: i16,
    ) -> Result<Frequency, SoundTuningError> {
        if degree.divisions != self.divisions {
            return Err(SoundTuningError::InvalidPitchClassN {
                divisions: self.divisions,
                index: degree.index,
            });
        }
        let ref_degree = map_pitch_class_to_degree(self.divisions, self.reference.0.class) as i32;
        let octave_delta = octave as i32 - self.reference.0.octave as i32;
        let steps = octave_delta * self.divisions as i32 + degree.index as i32 - ref_degree;
        Ok(Frequency(
            self.reference.1.0 * 2.0_f64.powf(steps as f64 / self.divisions as f64),
        ))
    }
}

/// A just-intonation tuning defined by rational frequency ratios from a root.
#[derive(Clone, Debug, PartialEq)]
pub struct JustIntonation {
    /// Pitch class that the ratio table is anchored to.
    pub root: PitchClass,
    /// Frequency ratios for the twelve chromatic degrees above the root.
    pub ratios: [f64; 12],
    /// Anchor pitch and its frequency.
    pub reference: (Pitch, Frequency),
}

/// Pythagorean tuning, built from a stack of pure perfect fifths.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PythagoreanTuning {
    /// Anchor pitch and its frequency.
    pub reference: (Pitch, Frequency),
}

/// Quarter-comma meantone temperament.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MeantoneQuarterComma {
    /// Anchor pitch and its frequency.
    pub reference: (Pitch, Frequency),
}

/// Werckmeister III well temperament.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct WerckmeisterIII {
    /// Anchor pitch and its frequency.
    pub reference: (Pitch, Frequency),
}

/// Young's well temperament.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct YoungTemperament {
    /// Anchor pitch and its frequency.
    pub reference: (Pitch, Frequency),
}

/// A tuning defined by a Scala (`.scl`) cents table.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalaScl {
    /// Cents value for each scale degree.
    pub cents: Vec<f64>,
    /// Anchor pitch and its frequency.
    pub reference: (Pitch, Frequency),
}

impl ScalaScl {
    /// Parses Scala `.scl` text into a tuning, accepting cents or ratio
    /// degrees and skipping comments and blank lines.
    pub fn parse(input: &str, reference: (Pitch, Frequency)) -> Result<Self, SoundTuningError> {
        let mut lines = input
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('!'));
        let _name = lines.next().ok_or(SoundTuningError::InvalidScala)?;
        let count = lines
            .next()
            .ok_or(SoundTuningError::InvalidScala)?
            .parse::<usize>()
            .map_err(|_| SoundTuningError::InvalidScala)?;
        let mut cents = Vec::with_capacity(count);
        for line in lines.take(count) {
            cents.push(parse_scala_step(line)?);
        }
        if cents.is_empty() {
            return Err(SoundTuningError::EmptyScala);
        }
        Ok(Self { cents, reference })
    }
}

macro_rules! table_tuning {
    ($name:ident, $label:literal, $table:expr) => {
        impl Tuning for $name {
            fn name(&self) -> &'static str {
                $label
            }

            fn reference(&self) -> (Pitch, Frequency) {
                self.reference
            }

            fn frequency_of(&self, pitch: Pitch) -> Frequency {
                frequency_from_cents_table(self.reference, pitch, &$table)
            }

            fn pitch_of(&self, frequency: Frequency) -> Pitch {
                nearest_pitch_from_table(self.reference, frequency, &$table)
            }

            fn frequency_of_degree(
                &self,
                degree: PitchClassN,
                octave: i16,
            ) -> Result<Frequency, SoundTuningError> {
                let pitch = self.pitch_from_degree(degree, octave)?;
                Ok(self.frequency_of(pitch))
            }
        }
    };
}

impl Tuning for JustIntonation {
    fn name(&self) -> &'static str {
        "just-intonation"
    }

    fn reference(&self) -> (Pitch, Frequency) {
        self.reference
    }

    fn frequency_of(&self, pitch: Pitch) -> Frequency {
        let table = just_intonation_cents(self.root, &self.ratios);
        frequency_from_cents_table(self.reference, pitch, &table)
    }

    fn pitch_of(&self, frequency: Frequency) -> Pitch {
        let table = just_intonation_cents(self.root, &self.ratios);
        nearest_pitch_from_table(self.reference, frequency, &table)
    }

    fn frequency_of_degree(
        &self,
        degree: PitchClassN,
        octave: i16,
    ) -> Result<Frequency, SoundTuningError> {
        let pitch = self.pitch_from_degree(degree, octave)?;
        Ok(self.frequency_of(pitch))
    }
}

table_tuning!(
    PythagoreanTuning,
    "pythagorean",
    [
        0.0, 113.685, 203.91, 317.595, 407.82, 521.505, 611.73, 701.955, 815.64, 905.865, 1019.55,
        1109.775
    ]
);
table_tuning!(
    MeantoneQuarterComma,
    "meantone-quarter-comma",
    [
        0.0, 76.049, 193.157, 310.264, 386.314, 503.421, 579.471, 696.578, 772.627, 889.735,
        1006.842, 1082.892
    ]
);
table_tuning!(
    WerckmeisterIII,
    "werckmeister-iii",
    [
        0.0, 90.225, 192.18, 294.135, 390.225, 498.045, 588.27, 696.09, 792.18, 888.27, 996.09,
        1092.18
    ]
);
table_tuning!(
    YoungTemperament,
    "young-temperament",
    [
        0.0, 92.18, 195.09, 294.14, 391.14, 498.05, 590.23, 697.14, 792.18, 890.23, 997.14, 1092.18
    ]
);

impl Tuning for ScalaScl {
    fn name(&self) -> &'static str {
        "scala-scl"
    }

    fn reference(&self) -> (Pitch, Frequency) {
        self.reference
    }

    fn frequency_of(&self, pitch: Pitch) -> Frequency {
        let degree = map_pitch_class_to_degree(self.divisions(), pitch.class);
        self.frequency_of_degree(
            PitchClassN::new(self.divisions(), degree).expect("mapped degree"),
            pitch.octave,
        )
        .expect("mapped pitch is valid")
    }

    fn pitch_of(&self, frequency: Frequency) -> Pitch {
        nearest_pitch_by_frequency(self.reference, frequency, |pitch| self.frequency_of(pitch))
    }

    fn divisions(&self) -> u32 {
        self.cents.len() as u32
    }

    fn frequency_of_degree(
        &self,
        degree: PitchClassN,
        octave: i16,
    ) -> Result<Frequency, SoundTuningError> {
        if self.cents.is_empty() {
            return Err(SoundTuningError::EmptyScala);
        }
        if degree.divisions != self.divisions() || degree.index as usize >= self.cents.len() {
            return Err(SoundTuningError::InvalidPitchClassN {
                divisions: self.divisions(),
                index: degree.index,
            });
        }
        let ref_degree = map_pitch_class_to_degree(self.divisions(), self.reference.0.class) as i32;
        let octave_delta = octave as i32 - self.reference.0.octave as i32;
        let degree_cents = self.cents[degree.index as usize];
        let ref_cents = self.cents[ref_degree as usize];
        let cents = octave_delta as f64 * 1200.0 + degree_cents - ref_cents;
        Ok(self.reference.1.shift_cents(cents))
    }
}

fn map_pitch_class_to_degree(divisions: u32, class: PitchClass) -> u32 {
    (((f64::from(class.value()) / 12.0) * f64::from(divisions)).round() as u32) % divisions
}

fn parse_scala_step(value: &str) -> Result<f64, SoundTuningError> {
    if let Some((num, den)) = value.split_once('/') {
        let numerator = f64::from_str(num.trim()).map_err(|_| SoundTuningError::InvalidScala)?;
        let denominator = f64::from_str(den.trim()).map_err(|_| SoundTuningError::InvalidScala)?;
        if denominator == 0.0 {
            return Err(SoundTuningError::InvalidScala);
        }
        Ok(1200.0 * (numerator / denominator).log2())
    } else {
        f64::from_str(value.trim()).map_err(|_| SoundTuningError::InvalidScala)
    }
}

fn just_intonation_cents(root: PitchClass, ratios: &[f64; 12]) -> [f64; 12] {
    let mut table = [0.0; 12];
    for (semitone, value) in table.iter_mut().enumerate() {
        let offset = (semitone + 12 - usize::from(root.value())) % 12;
        *value = 1200.0 * ratios[offset].log2();
    }
    normalize_monotonic(&mut table);
    table
}

fn normalize_monotonic(table: &mut [f64; 12]) {
    let mut previous = table[0];
    for value in table.iter_mut().skip(1) {
        while *value < previous {
            *value += 1200.0;
        }
        previous = *value;
    }
}

fn frequency_from_cents_table(
    reference: (Pitch, Frequency),
    pitch: Pitch,
    table: &[f64; 12],
) -> Frequency {
    let absolute =
        (f64::from(pitch.octave) + 1.0) * 1200.0 + table[usize::from(pitch.class.value())];
    let reference_absolute = (f64::from(reference.0.octave) + 1.0) * 1200.0
        + table[usize::from(reference.0.class.value())];
    reference.1.shift_cents(absolute - reference_absolute)
}

fn nearest_pitch_from_table(
    reference: (Pitch, Frequency),
    frequency: Frequency,
    table: &[f64; 12],
) -> Pitch {
    nearest_pitch_by_frequency(reference, frequency, |pitch| {
        frequency_from_cents_table(reference, pitch, table)
    })
}

fn nearest_pitch_by_frequency(
    reference: (Pitch, Frequency),
    frequency: Frequency,
    resolver: impl Fn(Pitch) -> Frequency,
) -> Pitch {
    let estimated = reference
        .0
        .transpose((frequency.cents_above(reference.1) / 100.0).round() as i32);
    (-2..=2)
        .map(|delta| estimated.transpose(delta))
        .min_by(|left, right| {
            let left_error = (resolver(*left).0 - frequency.0).abs();
            let right_error = (resolver(*right).0 - frequency.0).abs();
            left_error.total_cmp(&right_error)
        })
        .unwrap_or(estimated)
}
