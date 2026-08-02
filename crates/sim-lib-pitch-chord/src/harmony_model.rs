use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_pitch_ratio::PitchRatio;
use sim_lib_pitch_scale::Scale;
use sim_lib_pitch_set::PitchClassMask;
use thiserror::Error;

use crate::{Chord, ChordSymbol, PitchChordError, VoicingPolicy};

/// Failure while validating, decoding, or evaluating declarative harmony data.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum HarmonyError {
    /// A required identifier was empty or repeated.
    #[error("invalid harmony identifier: {0}")]
    InvalidId(String),
    /// A palette or template had no usable material.
    #[error("empty harmony {0}")]
    Empty(&'static str),
    /// A declarative field was outside its supported range.
    #[error("invalid harmony field {field}: {reason}")]
    InvalidField {
        /// Field name.
        field: &'static str,
        /// Concrete reason.
        reason: String,
    },
    /// A chord descriptor could not be realized.
    #[error("cannot realize chord template: {0}")]
    Chord(String),
    /// A SIM expression did not match the harmony data contract.
    #[error("invalid harmony expression: {0}")]
    Expression(String),
    /// A named sonance or dissonance model was not installed.
    #[error("unknown harmony metric model: {0}")]
    UnknownMetricModel(String),
}

impl From<PitchChordError> for HarmonyError {
    fn from(error: PitchChordError) -> Self {
        Self::Chord(error.to_string())
    }
}

/// Serializable source of the pitches in a [`ChordTemplate`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChordTemplateSource {
    /// A supported chord symbol and its root octave.
    Symbol {
        /// Chord symbol such as `Cmaj7`.
        symbol: String,
        /// Octave assigned to the symbol root.
        octave: i16,
    },
    /// Exact ordered pitches, retaining register and repeated voices.
    Pitches {
        /// Pitches in exact voice order.
        pitches: Vec<Pitch>,
    },
    /// Ordered pitch classes, preserving repeated voices.
    PitchClasses {
        /// Pitch classes in voice order.
        classes: Vec<PitchClass>,
        /// Octave assigned to the first pitch class.
        root_octave: i16,
    },
    /// One-based degrees of an existing scale map.
    ScaleDegrees {
        /// Scale map used to resolve every degree.
        scale: Scale,
        /// One-based, octave-wrapping scale degrees.
        degrees: Vec<usize>,
        /// Octave assigned to the first resolved degree.
        root_octave: i16,
    },
    /// A pitch-set mask with an explicit root used to order its classes.
    PitchSet {
        /// Existing pitch-set value.
        mask: PitchClassMask,
        /// Root from which the set is voiced upward.
        root: PitchClass,
        /// Octave assigned to `root`.
        root_octave: i16,
    },
}

/// Serializable chord descriptor composed from current pitch-domain values.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChordTemplate {
    /// Stable data identity.
    pub id: String,
    /// Chord source: symbol, ordered classes, scale degrees, or pitch set.
    pub source: ChordTemplateSource,
    /// Register arrangement applied after source realization.
    pub voicing: VoicingPolicy,
    /// Optional exact ratio tones used by ratio scoring.
    pub ratios: Vec<PitchRatio>,
}

impl ChordTemplate {
    /// Builds a template from an existing chord-symbol surface.
    pub fn from_symbol(id: impl Into<String>, symbol: impl Into<String>, octave: i16) -> Self {
        Self {
            id: id.into(),
            source: ChordTemplateSource::Symbol {
                symbol: symbol.into(),
                octave,
            },
            voicing: VoicingPolicy::Preserve,
            ratios: Vec::new(),
        }
    }

    /// Builds a template from ordered pitch classes, retaining duplicates.
    pub fn from_pitch_classes(
        id: impl Into<String>,
        classes: Vec<PitchClass>,
        root_octave: i16,
    ) -> Self {
        Self {
            id: id.into(),
            source: ChordTemplateSource::PitchClasses {
                classes,
                root_octave,
            },
            voicing: VoicingPolicy::Preserve,
            ratios: Vec::new(),
        }
    }

    /// Builds a template from one-based degrees of an existing scale.
    pub fn from_scale_degrees(
        id: impl Into<String>,
        scale: Scale,
        degrees: Vec<usize>,
        root_octave: i16,
    ) -> Self {
        Self {
            id: id.into(),
            source: ChordTemplateSource::ScaleDegrees {
                scale,
                degrees,
                root_octave,
            },
            voicing: VoicingPolicy::Preserve,
            ratios: Vec::new(),
        }
    }

    /// Builds a template from an existing pitch-set mask and declared root.
    pub fn from_pitch_set(
        id: impl Into<String>,
        mask: PitchClassMask,
        root: PitchClass,
        root_octave: i16,
    ) -> Self {
        Self {
            id: id.into(),
            source: ChordTemplateSource::PitchSet {
                mask,
                root,
                root_octave,
            },
            voicing: VoicingPolicy::Preserve,
            ratios: Vec::new(),
        }
    }

    /// Builds a template from exact ordered, registered pitches.
    pub fn from_pitches(id: impl Into<String>, pitches: Vec<Pitch>) -> Self {
        Self {
            id: id.into(),
            source: ChordTemplateSource::Pitches { pitches },
            voicing: VoicingPolicy::Preserve,
            ratios: Vec::new(),
        }
    }

    /// Attaches an explicit voicing policy.
    pub fn with_voicing(mut self, voicing: VoicingPolicy) -> Self {
        self.voicing = voicing;
        self
    }

    /// Attaches root-relative exact ratio tones.
    pub fn with_ratios(mut self, ratios: Vec<PitchRatio>) -> Self {
        self.ratios = ratios;
        self
    }

    /// Validates identifier, source material, and ratio cardinality.
    pub fn validate(&self) -> Result<(), HarmonyError> {
        validate_id(&self.id)?;
        let voice_count = match &self.source {
            ChordTemplateSource::Symbol { symbol, .. } => {
                ChordSymbol::parse(symbol)?;
                self.realize()?.notes.len()
            }
            ChordTemplateSource::Pitches { pitches } => pitches.len(),
            ChordTemplateSource::PitchClasses { classes, .. } => classes.len(),
            ChordTemplateSource::ScaleDegrees { scale, degrees, .. } => {
                for degree in degrees {
                    scale
                        .pitch_at_degree(*degree)
                        .map_err(|error| HarmonyError::Chord(error.to_string()))?;
                }
                degrees.len()
            }
            ChordTemplateSource::PitchSet { mask, root, .. } => {
                if !mask.pitch_classes().contains(root) {
                    return Err(HarmonyError::InvalidField {
                        field: "root",
                        reason: "pitch-set root is not in the mask".to_owned(),
                    });
                }
                mask.count_bits() as usize
            }
        };
        if voice_count == 0 {
            return Err(HarmonyError::Empty("chord template"));
        }
        if !self.ratios.is_empty() && self.ratios.len() != voice_count {
            return Err(HarmonyError::InvalidField {
                field: "ratios",
                reason: format!(
                    "received {} ratios for {voice_count} voices",
                    self.ratios.len()
                ),
            });
        }
        Ok(())
    }

    /// Realizes the descriptor into the current concrete chord and voicing types.
    pub fn realize(&self) -> Result<Chord, HarmonyError> {
        let notes = match &self.source {
            ChordTemplateSource::Symbol { symbol, octave } => {
                ChordSymbol::parse(symbol)?.to_chord(*octave).pitches()
            }
            ChordTemplateSource::Pitches { pitches } => pitches.clone(),
            ChordTemplateSource::PitchClasses {
                classes,
                root_octave,
            } => voice_classes(classes, *root_octave),
            ChordTemplateSource::ScaleDegrees {
                scale,
                degrees,
                root_octave,
            } => {
                let classes = degrees
                    .iter()
                    .map(|degree| {
                        scale
                            .pitch_at_degree(*degree)
                            .map_err(|error| HarmonyError::Chord(error.to_string()))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                voice_classes(&classes, *root_octave)
            }
            ChordTemplateSource::PitchSet {
                mask,
                root,
                root_octave,
            } => {
                let mut classes = mask.pitch_classes();
                classes.sort_by_key(|class| (class.value() + 12 - root.value()) % 12);
                voice_classes(&classes, *root_octave)
            }
        };
        if notes.is_empty() {
            return Err(HarmonyError::Empty("chord template"));
        }
        Ok(Chord::new(self.voicing.apply(notes)))
    }

    /// Returns the current pitch-set projection of the realized chord.
    pub fn pitch_set(&self) -> Result<PitchClassMask, HarmonyError> {
        Ok(self.realize()?.pitch_classes())
    }

    /// Returns a materialized transposition while preserving voice multiplicity.
    pub fn transpose(&self, id: impl Into<String>, semitones: i32) -> Result<Self, HarmonyError> {
        let chord = self.realize()?.transpose(semitones);
        Ok(Self::from_pitches(id, chord.notes).with_ratios(self.ratios.clone()))
    }
}

pub(crate) fn validate_id(id: &str) -> Result<(), HarmonyError> {
    if id.trim().is_empty()
        || id
            .chars()
            .any(|character| character.is_whitespace() || !character.is_ascii())
    {
        return Err(HarmonyError::InvalidId(id.to_owned()));
    }
    Ok(())
}

fn voice_classes(classes: &[PitchClass], root_octave: i16) -> Vec<Pitch> {
    let Some(root) = classes.first().copied() else {
        return Vec::new();
    };
    let root = Pitch {
        class: root,
        octave: root_octave,
    };
    classes
        .iter()
        .map(|class| root.transpose(i32::from((class.value() + 12 - root.class.value()) % 12)))
        .collect()
}
