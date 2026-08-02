use sim_kernel::{Diagnostic, Severity, SourceId, Span, Symbol};
use sim_lib_music_core::{MusicError, Score};
use thiserror::Error;

/// Resource limits applied before and during MusicXML profile import.
///
/// The defaults match the public runtime example while bounding every
/// independently amplifiable dimension of the accepted tree.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MusicXmlLimits {
    /// Maximum UTF-8 source size.
    pub bytes: usize,
    /// Maximum XML nodes, including text nodes.
    pub nodes: usize,
    /// Maximum element nesting depth.
    pub depth: usize,
    /// Maximum aggregate text-node bytes.
    pub text: usize,
    /// Maximum score parts.
    pub parts: usize,
    /// Maximum note/rest events across all parts.
    pub events: usize,
}

impl Default for MusicXmlLimits {
    fn default() -> Self {
        Self {
            bytes: 4_000_000,
            nodes: 200_000,
            depth: 64,
            text: 1_000_000,
            parts: 256,
            events: 1_000_000,
        }
    }
}

/// Kind of stable MusicXML identity retained by an exchange report.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NotationIdentityKind {
    /// A MusicXML `part` identity.
    Part,
    /// A MusicXML `note` or rest-event identity.
    Event,
}

/// Stable MusicXML identity associated with a canonical structural path.
///
/// `Score` remains the one music model. This record is exchange sidecar
/// evidence used to reproduce source identifiers on a later export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotationIdentity {
    /// Kind of identified object.
    pub kind: NotationIdentityKind,
    /// Canonical zero-based path such as `part/0/event/3`.
    pub canonical_path: String,
    /// XML identifier retained from import or deterministically allocated by export.
    pub xml_id: String,
}

/// Machine-readable kind of information not carried by canonical `Score`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NotationLossKind {
    /// Clef/layout metadata has no effect on canonical score semantics.
    Clef,
    /// A single-part display name is not carried by a melody score body.
    PartName,
    /// Enharmonic source spelling is not carried by canonical chromatic pitch.
    PitchSpelling,
    /// A missing tempo was replaced by the profile default.
    DefaultedTempo,
    /// A missing meter was replaced by the profile default.
    DefaultedTimeSignature,
    /// A note velocity cannot be represented by the bounded MusicXML subset.
    Velocity,
    /// A MIDI channel cannot be represented by the bounded MusicXML subset.
    Channel,
}

/// One explicit piece of notation information outside canonical `Score`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotationLoss {
    /// Stable loss classification.
    pub kind: NotationLossKind,
    /// Closest stable exchange path, when applicable.
    pub canonical_path: Option<String>,
    /// Human-readable exact reason.
    pub detail: String,
}

/// Result of a notation operation paired with any diagnostics produced.
///
/// Carries the converted `value` alongside the diagnostics gathered while
/// importing or exporting, so callers can inspect warnings without losing the
/// successful result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotationReport<T> {
    /// Converted value (an exported string or an imported `Score`).
    pub value: T,
    /// Diagnostics gathered during the operation.
    pub diagnostics: Vec<Diagnostic>,
    /// Stable exchange identities retained outside the canonical score.
    pub identities: Vec<NotationIdentity>,
    /// Every accepted-but-unrepresentable notation fact.
    pub losses: Vec<NotationLoss>,
}

/// Error raised while importing or exporting LilyPond-subset notation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NotationError {
    /// A duration could not be expressed in the supported note-value set.
    #[error("unsupported duration {0}")]
    UnsupportedDuration(String),
    /// A music object kind has no LilyPond-subset rendering.
    #[error("unsupported music object {0}")]
    UnsupportedMusicObject(&'static str),
    /// A key signature string could not be parsed.
    #[error("invalid key signature {0}")]
    InvalidKey(String),
    /// The LilyPond source used syntax outside the supported subset.
    #[error("unsupported lilypond syntax")]
    UnsupportedSyntax {
        /// Diagnostics describing the offending syntax.
        diagnostics: Vec<Diagnostic>,
    },
    /// MusicXML used markup outside the declared partwise profile.
    #[error("unsupported musicxml-partwise profile input")]
    UnsupportedMusicXml {
        /// Diagnostics describing the first rejected construct.
        diagnostics: Vec<Diagnostic>,
    },
    /// A bounded MusicXML resource dimension exceeded its configured maximum.
    #[error("musicxml {limit} limit exceeded: {actual} > {maximum}")]
    MusicXmlLimit {
        /// Resource dimension that exceeded its limit.
        limit: &'static str,
        /// Observed resource count.
        actual: usize,
        /// Configured maximum.
        maximum: usize,
    },
    /// MusicXML bytes were not valid UTF-8.
    #[error("musicxml source is not valid UTF-8")]
    InvalidMusicXmlUtf8,
    /// The reused XML parser rejected malformed input.
    #[error("invalid musicxml: {0}")]
    InvalidMusicXml(String),
    /// An error surfaced from the underlying music-core model.
    #[error(transparent)]
    Music(#[from] MusicError),
}

/// Codec converting between a `Score` and its LilyPond-subset text rendering.
///
/// Acts as the stateless entry point for the notation surface; each method
/// delegates to the import or export pipeline.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct NotationCodec;

impl NotationCodec {
    /// Exports a score to LilyPond text, returning the rendering with diagnostics.
    pub fn export_lilypond_report(
        &self,
        score: &Score,
    ) -> Result<NotationReport<String>, NotationError> {
        crate::export::export_lilypond_report(score)
    }

    /// Exports a score to LilyPond text, discarding diagnostics.
    pub fn export_lilypond(&self, score: &Score) -> Result<String, NotationError> {
        crate::export::export_lilypond(score)
    }

    /// Imports a score from LilyPond text, returning the score with diagnostics.
    pub fn import_lilypond_report(
        &self,
        source: &str,
    ) -> Result<NotationReport<Score>, NotationError> {
        crate::import::import_lilypond_report(source)
    }

    /// Imports a score from LilyPond text, discarding diagnostics.
    pub fn import_lilypond(&self, source: &str) -> Result<Score, NotationError> {
        crate::import::import_lilypond(source)
    }

    /// Imports the bounded MusicXML partwise profile with explicit limits.
    pub fn import_musicxml_partwise_report(
        &self,
        source: &[u8],
        limits: MusicXmlLimits,
    ) -> Result<NotationReport<Score>, NotationError> {
        crate::musicxml::import_musicxml_partwise_report(source, limits)
    }

    /// Exports a score through the bounded MusicXML partwise profile.
    ///
    /// `identities` may be the sidecar returned by a prior import; matching
    /// canonical paths reproduce the original stable XML identifiers.
    pub fn export_musicxml_partwise_report(
        &self,
        score: &Score,
        identities: &[NotationIdentity],
    ) -> Result<NotationReport<String>, NotationError> {
        crate::musicxml::export_musicxml_partwise_report(score, identities)
    }
}

pub(crate) fn error_at(message: impl Into<String>, span: Span) -> NotationError {
    NotationError::UnsupportedSyntax {
        diagnostics: vec![Diagnostic {
            severity: Severity::Error,
            message: message.into(),
            source: Some(SourceId("notation:lilypond".to_owned())),
            span: Some(span),
            code: None,
            related: Vec::new(),
        }],
    }
}

pub(crate) fn musicxml_error(message: impl Into<String>, span: Option<Span>) -> NotationError {
    NotationError::UnsupportedMusicXml {
        diagnostics: vec![Diagnostic {
            severity: Severity::Error,
            message: message.into(),
            source: Some(SourceId("notation:musicxml-partwise".to_owned())),
            span,
            code: Some(Symbol::qualified("musicxml", "profile")),
            related: Vec::new(),
        }],
    }
}

pub(crate) fn loss_diagnostic(loss: &NotationLoss) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        message: loss.detail.clone(),
        source: Some(SourceId("notation:musicxml-partwise".to_owned())),
        span: None,
        code: Some(Symbol::qualified("musicxml", "loss")),
        related: Vec::new(),
    }
}
