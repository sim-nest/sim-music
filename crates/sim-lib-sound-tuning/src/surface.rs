use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_sound_core::Frequency;

use crate::{
    EqualTemperament, JustIntonation, MeantoneQuarterComma, PythagoreanTuning, ScalaScl,
    SoundTuningError, Tuning, WerckmeisterIII, YoungTemperament,
};

/// A serializable description of a tuning system, used to build a concrete
/// [`Tuning`] without depending on its Rust type directly.
#[derive(Clone, Debug, PartialEq)]
pub enum TuningDescriptor {
    /// Equal temperament with the given division count and reference anchor.
    EqualTemperament {
        /// Number of equal divisions of the octave.
        divisions: u32,
        /// MIDI note number of the reference anchor.
        reference_midi: u8,
        /// Frequency of the reference anchor, in hertz.
        reference_hz: f64,
    },
    /// Just intonation with an explicit ratio table.
    JustIntonation {
        /// Root pitch-class index the ratios are anchored to.
        root: u8,
        /// Frequency ratios for the twelve chromatic degrees.
        ratios: [f64; 12],
        /// MIDI note number of the reference anchor.
        reference_midi: u8,
        /// Frequency of the reference anchor, in hertz.
        reference_hz: f64,
    },
    /// Pythagorean tuning.
    PythagoreanTuning {
        /// MIDI note number of the reference anchor.
        reference_midi: u8,
        /// Frequency of the reference anchor, in hertz.
        reference_hz: f64,
    },
    /// Quarter-comma meantone temperament.
    MeantoneQuarterComma {
        /// MIDI note number of the reference anchor.
        reference_midi: u8,
        /// Frequency of the reference anchor, in hertz.
        reference_hz: f64,
    },
    /// Werckmeister III well temperament.
    WerckmeisterIII {
        /// MIDI note number of the reference anchor.
        reference_midi: u8,
        /// Frequency of the reference anchor, in hertz.
        reference_hz: f64,
    },
    /// Young's well temperament.
    YoungTemperament {
        /// MIDI note number of the reference anchor.
        reference_midi: u8,
        /// Frequency of the reference anchor, in hertz.
        reference_hz: f64,
    },
    /// A Scala cents-table tuning.
    ScalaScl {
        /// Cents value for each scale degree.
        cents: Vec<f64>,
        /// MIDI note number of the reference anchor.
        reference_midi: u8,
        /// Frequency of the reference anchor, in hertz.
        reference_hz: f64,
    },
}

impl TuningDescriptor {
    /// Builds the concrete boxed [`Tuning`] described by this descriptor.
    pub fn to_tuning(&self) -> Result<Box<dyn Tuning>, SoundTuningError> {
        Ok(match self {
            Self::EqualTemperament {
                divisions,
                reference_midi,
                reference_hz,
            } => Box::new(EqualTemperament::new(
                *divisions,
                (Pitch::from_midi(*reference_midi), Frequency(*reference_hz)),
            )?),
            Self::JustIntonation {
                root,
                ratios,
                reference_midi,
                reference_hz,
            } => Box::new(JustIntonation {
                root: PitchClass::new(*root).map_err(|_| SoundTuningError::InvalidScala)?,
                ratios: *ratios,
                reference: (Pitch::from_midi(*reference_midi), Frequency(*reference_hz)),
            }),
            Self::PythagoreanTuning {
                reference_midi,
                reference_hz,
            } => Box::new(PythagoreanTuning {
                reference: (Pitch::from_midi(*reference_midi), Frequency(*reference_hz)),
            }),
            Self::MeantoneQuarterComma {
                reference_midi,
                reference_hz,
            } => Box::new(MeantoneQuarterComma {
                reference: (Pitch::from_midi(*reference_midi), Frequency(*reference_hz)),
            }),
            Self::WerckmeisterIII {
                reference_midi,
                reference_hz,
            } => Box::new(WerckmeisterIII {
                reference: (Pitch::from_midi(*reference_midi), Frequency(*reference_hz)),
            }),
            Self::YoungTemperament {
                reference_midi,
                reference_hz,
            } => Box::new(YoungTemperament {
                reference: (Pitch::from_midi(*reference_midi), Frequency(*reference_hz)),
            }),
            Self::ScalaScl {
                cents,
                reference_midi,
                reference_hz,
            } => Box::new(ScalaScl {
                cents: cents.clone(),
                reference: (Pitch::from_midi(*reference_midi), Frequency(*reference_hz)),
            }),
        })
    }
}

/// Returns the standard 5-limit just-intonation tuning rooted at C with A4 at
/// 440 Hz.
pub fn default_just_intonation() -> JustIntonation {
    JustIntonation {
        root: PitchClass::C,
        ratios: [
            1.0,
            16.0 / 15.0,
            9.0 / 8.0,
            6.0 / 5.0,
            5.0 / 4.0,
            4.0 / 3.0,
            45.0 / 32.0,
            3.0 / 2.0,
            8.0 / 5.0,
            5.0 / 3.0,
            9.0 / 5.0,
            15.0 / 8.0,
        ],
        reference: (Pitch::from_midi(69), Frequency(440.0)),
    }
}

/// Returns the frequency of `pitch` under `tuning`.
pub fn render_pitch_with_tuning(pitch: Pitch, tuning: &dyn Tuning) -> Frequency {
    tuning.frequency_of(pitch)
}

/// Returns the interval from `a` to `b` in cents under `tuning`.
pub fn cents_between(a: Pitch, b: Pitch, tuning: &dyn Tuning) -> f64 {
    tuning.frequency_of(b).cents_above(tuning.frequency_of(a))
}

/// Returns the frequency of `pitch` under `tuning`, shifted by `cents`.
pub fn detune(tuning: &dyn Tuning, pitch: Pitch, cents: f64) -> Frequency {
    tuning.frequency_of(pitch).shift_cents(cents)
}
