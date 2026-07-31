use sim_lib_music_core::{Articulation, Channel, ObjectId, Pitch, Time};
use sim_lib_pitch_dissonance::ContextualSonanceOptions;
use sim_lib_pitch_namer::LabelContext;
use sim_lib_sound_tuning::EqualTemperament;
use thiserror::Error;

/// Exact half-open musical span `[start, end)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeSpan {
    /// Inclusive start.
    pub start: Time,
    /// Exclusive end.
    pub end: Time,
}

impl TimeSpan {
    /// Builds a non-negative, ordered span.
    pub fn new(start: Time, end: Time) -> Result<Self, ConsonanceError> {
        let zero = Time::from_integer(0);
        if start < zero || end < start {
            return Err(ConsonanceError::InvalidSpan { start, end });
        }
        Ok(Self { start, end })
    }

    /// Returns the exact span length.
    pub fn duration(&self) -> Time {
        self.end - self.start
    }
}

/// Kind of source from which a consonance report was derived.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ProvenanceKind {
    /// A canonical [`sim_lib_music_core::Score`].
    Score,
    /// An identity-bearing [`sim_lib_music_core::Staff`].
    Staff,
    /// A pedal- and overlap-realized MIDI timeline.
    MidiTimeline,
}

/// Report-level source and identity evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provenance {
    /// Source representation.
    pub kind: ProvenanceKind,
    /// Stable source label.
    pub source: String,
    /// How source voice, note, and event identities were obtained.
    pub identity_policy: String,
    /// Exact conversion, realization, or source facts.
    pub facts: Vec<String>,
}

/// One sounding note with full source identity and exact lifetime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundingNote {
    /// Voice identity.
    pub voice_id: ObjectId,
    /// Logical note identity.
    pub note_id: ObjectId,
    /// Event identity.
    pub event_id: ObjectId,
    /// Octave-aware pitch.
    pub pitch: Pitch,
    /// Exact source onset.
    pub onset: Time,
    /// Exact half-open source release.
    pub release: Time,
    /// MIDI velocity, retained independently from acoustic amplitude policy.
    pub velocity: u8,
    /// MIDI channel.
    pub channel: Channel,
    /// Notated or realized articulation.
    pub articulation: Articulation,
    /// Note-local source facts.
    pub provenance: Vec<String>,
}

/// One maximal exact interval with a constant sounding-note multiset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundingWindow {
    /// Exact half-open span.
    pub span: TimeSpan,
    /// Every note sounding throughout the span, including equal-pitch events.
    pub notes: Vec<SoundingNote>,
}

/// Independently inspectable output from one named metric model.
#[derive(Clone, Debug, PartialEq)]
pub struct MetricReport {
    /// Stable model name.
    pub model: String,
    /// Total roughness or conflict mass before density normalization.
    pub roughness_mass: f64,
    /// Density normalized by the model's named opportunity policy.
    pub normalized_density: f64,
    /// Harmonic, commonality, ratio, or continuity context component.
    pub harmonic_context: f64,
    /// Named normalization policy.
    pub normalization: String,
    /// Named aggregation policy.
    pub aggregation: String,
    /// Model-specific provenance.
    pub evidence: Vec<String>,
}

/// All metric families for one sounding window.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowSonance {
    /// Source window, including multiplicity and identities.
    pub window: SoundingWindow,
    /// Set-domain pitch models, each retained separately.
    pub pitch: Vec<MetricReport>,
    /// Frequency- and amplitude-domain models, each retained separately.
    pub acoustic: Vec<MetricReport>,
    /// Exact-ratio contextual model.
    pub ratio: MetricReport,
    /// Event-commonality contextual model.
    pub commonality: MetricReport,
    /// Voice-leading contextual model.
    pub leading: MetricReport,
}

/// Complete consonance evaluation without an implicit aggregate score.
#[derive(Clone, Debug, PartialEq)]
pub struct ConsonanceReport {
    /// Event-boundary windows and their separate metrics.
    pub windows: Vec<WindowSonance>,
    /// Source and identity evidence.
    pub provenance: Provenance,
}

/// Explicit policy for score consonance evaluation.
#[derive(Clone, Debug, PartialEq)]
pub struct ConsonancePolicy {
    /// Context for pitch-domain models.
    pub pitch_context: LabelContext,
    /// Duplicate, normalization, ratio, and voice policy for contextual models.
    pub contextual: ContextualSonanceOptions,
    /// Equal-temperament acoustic realization used before sound analysis.
    pub tuning: EqualTemperament,
    /// Requested pitch model names.
    pub pitch_models: Vec<String>,
    /// Requested acoustic model names.
    pub acoustic_models: Vec<String>,
}

impl Default for ConsonancePolicy {
    fn default() -> Self {
        Self {
            pitch_context: LabelContext::default(),
            contextual: ContextualSonanceOptions::standard(),
            tuning: EqualTemperament::default(),
            pitch_models: [
                "interval-vector",
                "forte-complexity",
                "tonal-function",
                "tritone-density",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            acoustic_models: [
                "harmonic-entropy",
                "helmholtz-beating",
                "plomp-levelt",
                "sethares",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        }
    }
}

/// Error raised by exact window construction or metric evaluation.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ConsonanceError {
    /// A half-open span was negative or reversed.
    #[error("invalid half-open span [{start}, {end})")]
    InvalidSpan {
        /// Invalid start.
        start: Time,
        /// Invalid end.
        end: Time,
    },
    /// A requested model was not installed.
    #[error("unknown {domain} consonance model {model}")]
    UnknownModel {
        /// Metric domain.
        domain: &'static str,
        /// Requested model.
        model: String,
    },
    /// Raw MIDI score bodies require the realization-aware entry point.
    #[error("raw MIDI score bodies must be realized before consonance evaluation")]
    MidiRequiresRealization,
    /// Existing exact score conversion rejected the source.
    #[error("score conversion failed: {0}")]
    ScoreConversion(String),
    /// An identity derived from a source was invalid.
    #[error("invalid consonance identity: {0}")]
    Identity(String),
    /// Acoustic analysis rejected the realized tones.
    #[error("acoustic consonance evaluation failed: {0}")]
    Acoustic(String),
}
