use std::collections::BTreeSet;

use sim_lib_pitch_scale::Scale;
use sim_lib_pitch_set::PitchClassMask;

use crate::{ChordPalette, HarmonyError, VoicingChangePalette, validate_id};

/// Inclusive bound used by count-oriented harmony constraints.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CountRange {
    /// Smallest admitted count.
    pub min: usize,
    /// Largest admitted count.
    pub max: usize,
}

impl CountRange {
    /// Builds a checked inclusive range.
    pub fn new(min: usize, max: usize) -> Result<Self, HarmonyError> {
        if min > max {
            return Err(HarmonyError::InvalidField {
                field: "count-range",
                reason: format!("minimum {min} exceeds maximum {max}"),
            });
        }
        Ok(Self { min, max })
    }

    /// Returns whether `value` falls inside the inclusive range.
    pub fn contains(self, value: usize) -> bool {
        (self.min..=self.max).contains(&value)
    }
}

/// Serializable predicate covering catalog harmony and template filters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HarmonyPredicate {
    /// Accept every prefix and retain an evidence row.
    Always,
    /// Require current melody pitches to be a subset of the current chord.
    MelodyInChord,
    /// Require a chord at one absolute or end-relative position.
    ChordAt {
        /// Non-negative from start; negative from end of the melody.
        position: i32,
        /// Required pitch-set identity.
        chord: PitchClassMask,
    },
    /// Require a chord everywhere except one position.
    ChordEverywhereExcept {
        /// Exempt position.
        position: i32,
        /// Required pitch-set identity elsewhere.
        chord: PitchClassMask,
    },
    /// Forbid a chord everywhere except one position.
    ChordOnlyAt {
        /// Sole allowed position.
        position: i32,
        /// Chord unavailable elsewhere.
        chord: PitchClassMask,
    },
    /// Accept only while the current prefix is at one position.
    AtPosition {
        /// Accepted position.
        position: i32,
    },
    /// Constrain the current chord's distinct pitch-class count.
    DistinctPitchClasses {
        /// Inclusive admitted count.
        count: CountRange,
    },
    /// Constrain the common-note count of the last transition.
    CommonNotes {
        /// Inclusive admitted count.
        count: CountRange,
    },
    /// Read the exact common-note count from a repeating pattern.
    CommonNotePattern {
        /// Non-empty cyclic pattern.
        counts: Vec<usize>,
    },
    /// Forbid exact chord recurrence inside a preceding window.
    MinimumChordDistance {
        /// Number of preceding positions inspected.
        distance: usize,
    },
    /// Require exact chord recurrence inside a preceding window.
    MaximumChordDistance {
        /// Number of preceding positions inspected.
        distance: usize,
    },
    /// Forbid transposition-normalized chord-type recurrence in a window.
    MinimumTypeDistance {
        /// Number of preceding positions inspected.
        distance: usize,
    },
    /// Forbid equal chords at every positive multiple of a period.
    PeriodicVariation {
        /// Positive position period.
        period: usize,
    },
    /// Constrain commonality at every positive multiple of a period.
    PeriodicCommonality {
        /// Positive position period.
        period: usize,
        /// Inclusive admitted common-note count.
        count: CountRange,
    },
    /// Require a trailing window to fit some transposition of a scale map.
    InsideScaleWindow {
        /// Existing scale map whose tonic is rotated.
        scale: Scale,
        /// Positive trailing window length.
        length: usize,
    },
    /// Require a full trailing window not to fit any transposition of a scale map.
    OutsideScaleWindow {
        /// Existing scale map whose tonic is rotated.
        scale: Scale,
        /// Positive trailing window length.
        length: usize,
    },
    /// Require the flattened template chain not to exceed the melody length.
    TemplateLength,
    /// Require every adjacent pair of templates to share the joint chord.
    TemplatesConnect,
    /// Require melody subsets for each newly contributed template chord.
    TemplateMelodyInChord,
    /// Side-effect-free replacement for the catalog logging pseudo-filter.
    ObserveDepth,
    /// Require every nested predicate.
    All(Vec<HarmonyPredicate>),
    /// Require at least one nested predicate.
    Any(Vec<HarmonyPredicate>),
    /// Negate one nested predicate.
    Not(Box<HarmonyPredicate>),
}

impl HarmonyPredicate {
    pub(crate) fn validate(&self) -> Result<(), HarmonyError> {
        match self {
            Self::CommonNotePattern { counts } if counts.is_empty() => {
                Err(HarmonyError::Empty("common-note pattern"))
            }
            Self::MinimumChordDistance { distance }
            | Self::MaximumChordDistance { distance }
            | Self::MinimumTypeDistance { distance }
            | Self::PeriodicVariation { period: distance }
                if *distance == 0 =>
            {
                Err(HarmonyError::InvalidField {
                    field: "distance",
                    reason: "distance and period values must be positive".to_owned(),
                })
            }
            Self::PeriodicCommonality { period: 0, .. } => Err(HarmonyError::InvalidField {
                field: "period",
                reason: "period must be positive".to_owned(),
            }),
            Self::InsideScaleWindow { length: 0, .. }
            | Self::OutsideScaleWindow { length: 0, .. } => Err(HarmonyError::InvalidField {
                field: "scale-window.length",
                reason: "window length must be positive".to_owned(),
            }),
            Self::All(predicates) | Self::Any(predicates) if predicates.is_empty() => {
                Err(HarmonyError::Empty("predicate composition"))
            }
            Self::All(predicates) | Self::Any(predicates) => {
                for predicate in predicates {
                    predicate.validate()?;
                }
                Ok(())
            }
            Self::Not(predicate) => predicate.validate(),
            _ => Ok(()),
        }
    }
}

/// Named hard rule whose result is retained in evaluation evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarmonyConstraint {
    /// Stable rule identity.
    pub id: String,
    /// Declarative predicate.
    pub predicate: HarmonyPredicate,
}

impl HarmonyConstraint {
    /// Builds a named constraint.
    pub fn new(id: impl Into<String>, predicate: HarmonyPredicate) -> Self {
        Self {
            id: id.into(),
            predicate,
        }
    }
}

/// One declarative soft metric.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HarmonyMetric {
    /// Number of distinct pitch classes in the current chord.
    DistinctPitchClasses,
    /// Common pitch classes in the last transition.
    CommonNotes,
    /// Certified circular squared voice-leading cost.
    VoiceLeading,
    /// One built-in pitch-dissonance registry model.
    PitchDissonance {
        /// Stable registry model name.
        model: String,
    },
    /// One built-in contextual-sonance registry model.
    ContextualSonance {
        /// Stable registry model name.
        model: String,
    },
    /// Exact-ratio generalized-mean cost.
    RatioComplexity {
        /// Positive finite generalized-mean exponent encoded in milli-units.
        exponent_milli: u32,
    },
}

/// Named weighted value used by the soft-rule lane.
#[derive(Clone, Debug, PartialEq)]
pub struct Weighted<T> {
    /// Stable metric identity.
    pub id: String,
    /// Caller-declared multiplier. Negative values express rewards.
    pub weight: f64,
    /// Metric descriptor.
    pub value: T,
}

impl<T> Weighted<T> {
    /// Builds a weighted value.
    pub fn new(id: impl Into<String>, weight: f64, value: T) -> Self {
        Self {
            id: id.into(),
            weight,
            value,
        }
    }
}

/// Hard legality and soft scoring kept as separate declarative lanes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HarmonyRuleSet {
    /// Hard rules; every rule must pass for legality.
    pub hard: Vec<HarmonyConstraint>,
    /// Soft metrics; none can change legality.
    pub soft: Vec<Weighted<HarmonyMetric>>,
}

impl HarmonyRuleSet {
    /// Validates rule identities, predicate parameters, and metric weights.
    pub fn validate(&self) -> Result<(), HarmonyError> {
        let mut ids = BTreeSet::new();
        for rule in &self.hard {
            validate_id(&rule.id)?;
            if !ids.insert(&rule.id) {
                return Err(HarmonyError::InvalidId(rule.id.clone()));
            }
            rule.predicate.validate()?;
        }
        for metric in &self.soft {
            validate_id(&metric.id)?;
            if !ids.insert(&metric.id) {
                return Err(HarmonyError::InvalidId(metric.id.clone()));
            }
            if !metric.weight.is_finite() {
                return Err(HarmonyError::InvalidField {
                    field: "metric.weight",
                    reason: "weight must be finite".to_owned(),
                });
            }
            if matches!(
                metric.value,
                HarmonyMetric::RatioComplexity { exponent_milli: 0 }
            ) {
                return Err(HarmonyError::InvalidField {
                    field: "ratio-complexity.exponent",
                    reason: "exponent must be positive".to_owned(),
                });
            }
        }
        Ok(())
    }
}

/// Named data describing the catalog harmony export arrangement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarmonyRenderProfile {
    /// Stable fixture identity.
    pub id: String,
    /// Semitone lift for chord voices.
    pub chord_transpose: i32,
    /// Semitone lift for the source melody.
    pub melody_transpose: i32,
    /// Exact integer duration multiplier.
    pub duration_multiplier: u32,
    /// General MIDI program assigned to chord tracks.
    pub chord_program: u8,
    /// General MIDI program assigned to the melody track.
    pub melody_program: u8,
    /// Output tempo.
    pub tempo_bpm: u32,
    /// Output time signature.
    pub time_signature: (u8, u8),
}

impl HarmonyRenderProfile {
    /// Validates tempo, duration, programs, and time-signature bounds.
    pub fn validate(&self) -> Result<(), HarmonyError> {
        validate_id(&self.id)?;
        if self.duration_multiplier == 0 || self.tempo_bpm == 0 || self.time_signature.1 == 0 {
            return Err(HarmonyError::InvalidField {
                field: "render-profile",
                reason: "duration, tempo, and time-signature denominator must be positive"
                    .to_owned(),
            });
        }
        if self.chord_program > 127 || self.melody_program > 127 {
            return Err(HarmonyError::InvalidField {
                field: "render-profile.program",
                reason: "General MIDI program must fit 0..=127".to_owned(),
            });
        }
        Ok(())
    }
}

/// Complete data-only harmony program loadable from a general expression codec.
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonyProgram {
    /// Stable program identity.
    pub id: String,
    /// Materialized chord/template palette.
    pub palette: ChordPalette,
    /// Hard and soft rules.
    pub rules: HarmonyRuleSet,
    /// Optional materialized voicing changes.
    pub voicing_changes: VoicingChangePalette,
    /// Rendering policy consumed by music-combinator adapters.
    pub render: HarmonyRenderProfile,
}

impl HarmonyProgram {
    /// Validates the complete program.
    pub fn validate(&self) -> Result<(), HarmonyError> {
        validate_id(&self.id)?;
        self.palette.validate()?;
        self.rules.validate()?;
        self.voicing_changes.validate()?;
        self.render.validate()
    }
}
