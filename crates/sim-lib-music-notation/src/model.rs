use sim_kernel::{Diagnostic, Severity, SourceId, Span};
use sim_lib_music_core::{MusicError, Score};
use thiserror::Error;

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
