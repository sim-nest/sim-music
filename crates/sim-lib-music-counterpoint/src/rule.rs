use sim_lib_music_core::{Pitch, Time};
use thiserror::Error;

/// Conventional species label attached to a rule set.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Species {
    /// One note against one note.
    First,
    /// Two notes against one note.
    Second,
    /// Four notes against one note.
    Third,
    /// Suspensions and tied syncopation.
    Fourth,
    /// Caller-authored non-species policy.
    Open,
}

/// Allowed pitch range, inclusive at both ends.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PitchRange {
    /// Lowest allowed pitch.
    pub low: Pitch,
    /// Highest allowed pitch.
    pub high: Pitch,
}

impl PitchRange {
    /// Full MIDI pitch range.
    pub fn midi() -> Self {
        Self {
            low: Pitch::from_midi(0),
            high: Pitch::from_midi(127),
        }
    }

    /// Returns whether `pitch` lies in this inclusive range.
    pub fn contains(self, pitch: Pitch) -> bool {
        self.low <= pitch && pitch <= self.high
    }
}

/// Interval constraints for melodic and simultaneous motion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntervalRules {
    /// Largest allowed absolute melodic leap in semitones.
    pub max_melodic_semitones: u8,
    /// Explicitly forbidden absolute melodic intervals.
    pub forbidden_melodic_semitones: Vec<u8>,
    /// Accepted harmonic interval classes in `0..=6`.
    pub consonant_harmonic_classes: Vec<u8>,
    /// Harmonic classes treated as perfect for motion checks.
    pub perfect_harmonic_classes: Vec<u8>,
}

/// Relative-motion policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MotionRules {
    /// Reject similar motion between repeated perfect interval classes.
    pub forbid_parallel_perfects: bool,
    /// Reject similar motion into a perfect interval when either voice leaps.
    pub forbid_direct_perfects: bool,
    /// Semitone distance above which a voice movement is a leap.
    pub leap_threshold: u8,
}

/// Register, crossing, and overlap policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceRules {
    /// Default range for voices without an indexed override.
    pub default_range: PitchRange,
    /// Per-voice range overrides by source index.
    pub ranges: Vec<PitchRange>,
    /// Whether lower source indices are expected to remain at higher pitches.
    pub highest_voice_first: bool,
    /// Whether voices may exchange vertical order while sounding.
    pub allow_crossing: bool,
    /// Whether a voice may move beyond the other voice's previous pitch.
    pub allow_overlap: bool,
}

/// Exact-duration policy expressed as ratios of one caller-visible pulse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurationRules {
    /// Exact reference pulse.
    pub pulse: Time,
    /// Allowed note durations divided by `pulse`; empty means unrestricted.
    pub allowed_pulse_ratios: Vec<Time>,
}

/// Recognized context in which a dissonant class may be accepted.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DissonanceContext {
    /// Stepwise motion continues in the same direction.
    Passing,
    /// Stepwise motion leaves and returns to the same pitch.
    Neighbor,
    /// A prepared held note resolves downward by step.
    Suspension,
}

/// Preparation and resolution policy for non-consonant intervals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DissonanceRules {
    /// Contexts that may legalize a non-consonant interval.
    pub allowed_contexts: Vec<DissonanceContext>,
    /// Maximum absolute semitone distance considered stepwise.
    pub max_step_semitones: u8,
    /// Whether passing and neighboring dissonances must begin off the pulse.
    pub require_weak_attack: bool,
}

/// Complete inspectable counterpoint rule set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleSet {
    /// Stable rule-set identifier.
    pub id: String,
    /// Species label.
    pub species: Species,
    /// Melodic and harmonic interval data.
    pub intervals: IntervalRules,
    /// Relative-motion data.
    pub motion: MotionRules,
    /// Range, crossing, and overlap data.
    pub voices: VoiceRules,
    /// Exact duration data.
    pub durations: DurationRules,
    /// Dissonance preparation and resolution data.
    pub dissonance: DissonanceRules,
}

/// Invalid caller-authored rule data.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RuleError {
    /// Rule-set id was empty.
    #[error("counterpoint rule-set id cannot be empty")]
    EmptyId,
    /// Duration pulse was not positive.
    #[error("counterpoint duration pulse must be positive")]
    InvalidPulse,
    /// A pitch range was reversed.
    #[error("counterpoint pitch range {index} is reversed")]
    ReversedRange {
        /// Index in the effective range list.
        index: usize,
    },
    /// An interval class was outside `0..=6`.
    #[error("counterpoint harmonic interval class {value} is outside 0..=6")]
    InvalidIntervalClass {
        /// Invalid class.
        value: u8,
    },
}

impl RuleSet {
    /// First-species rules with whole-pulse durations.
    pub fn species_one(pulse: Time) -> Self {
        species_rules(
            "species-one",
            Species::First,
            pulse,
            vec![Time::from_integer(1)],
            vec![],
        )
    }

    /// Second-species rules with weak-beat passing and neighbor tones.
    pub fn species_two(pulse: Time) -> Self {
        species_rules(
            "species-two",
            Species::Second,
            pulse,
            vec![Time::new(1, 2), Time::from_integer(1)],
            vec![DissonanceContext::Passing, DissonanceContext::Neighbor],
        )
    }

    /// Third-species rules with quarter-pulse motion.
    pub fn species_three(pulse: Time) -> Self {
        species_rules(
            "species-three",
            Species::Third,
            pulse,
            vec![Time::new(1, 4), Time::new(1, 2), Time::from_integer(1)],
            vec![DissonanceContext::Passing, DissonanceContext::Neighbor],
        )
    }

    /// Fourth-species rules allowing prepared suspensions.
    pub fn species_four(pulse: Time) -> Self {
        species_rules(
            "species-four",
            Species::Fourth,
            pulse,
            vec![Time::new(1, 2), Time::from_integer(1)],
            vec![DissonanceContext::Suspension],
        )
    }

    /// Open rule set that accepts all pitch classes, durations, crossings, and motion.
    pub fn open() -> Self {
        Self {
            id: "open".to_owned(),
            species: Species::Open,
            intervals: IntervalRules {
                max_melodic_semitones: u8::MAX,
                forbidden_melodic_semitones: Vec::new(),
                consonant_harmonic_classes: (0..=6).collect(),
                perfect_harmonic_classes: vec![0, 5],
            },
            motion: MotionRules {
                forbid_parallel_perfects: false,
                forbid_direct_perfects: false,
                leap_threshold: u8::MAX,
            },
            voices: VoiceRules {
                default_range: PitchRange::midi(),
                ranges: Vec::new(),
                highest_voice_first: true,
                allow_crossing: true,
                allow_overlap: true,
            },
            durations: DurationRules {
                pulse: Time::from_integer(1),
                allowed_pulse_ratios: Vec::new(),
            },
            dissonance: DissonanceRules {
                allowed_contexts: Vec::new(),
                max_step_semitones: 2,
                require_weak_attack: false,
            },
        }
    }

    /// Checks caller-authored rule data before analysis.
    pub fn validate(&self) -> Result<(), RuleError> {
        if self.id.trim().is_empty() {
            return Err(RuleError::EmptyId);
        }
        if self.durations.pulse <= Time::from_integer(0) {
            return Err(RuleError::InvalidPulse);
        }
        for (index, range) in std::iter::once(&self.voices.default_range)
            .chain(&self.voices.ranges)
            .enumerate()
        {
            if range.low > range.high {
                return Err(RuleError::ReversedRange { index });
            }
        }
        for value in self
            .intervals
            .consonant_harmonic_classes
            .iter()
            .chain(&self.intervals.perfect_harmonic_classes)
        {
            if *value > 6 {
                return Err(RuleError::InvalidIntervalClass { value: *value });
            }
        }
        Ok(())
    }

    /// Effective pitch range for one source voice.
    pub fn range_for_voice(&self, index: usize) -> PitchRange {
        self.voices
            .ranges
            .get(index)
            .copied()
            .unwrap_or(self.voices.default_range)
    }
}

fn species_rules(
    id: &str,
    species: Species,
    pulse: Time,
    allowed_pulse_ratios: Vec<Time>,
    allowed_contexts: Vec<DissonanceContext>,
) -> RuleSet {
    RuleSet {
        id: id.to_owned(),
        species,
        intervals: IntervalRules {
            max_melodic_semitones: 12,
            forbidden_melodic_semitones: vec![6, 10, 11],
            consonant_harmonic_classes: vec![0, 3, 4, 5],
            perfect_harmonic_classes: vec![0, 5],
        },
        motion: MotionRules {
            forbid_parallel_perfects: true,
            forbid_direct_perfects: true,
            leap_threshold: 2,
        },
        voices: VoiceRules {
            default_range: PitchRange::midi(),
            ranges: Vec::new(),
            highest_voice_first: true,
            allow_crossing: false,
            allow_overlap: false,
        },
        durations: DurationRules {
            pulse,
            allowed_pulse_ratios,
        },
        dissonance: DissonanceRules {
            allowed_contexts,
            max_step_semitones: 2,
            require_weak_attack: !matches!(species, Species::Fourth),
        },
    }
}
