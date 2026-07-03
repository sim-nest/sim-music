use std::fmt;

use thiserror::Error;

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_scale::Key;
use sim_lib_pitch_set::PitchClassMask;

/// A school of chord and pitch-set naming.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NamingSchool {
    /// Forte set-class names (for example `4-27`).
    Forte,
    /// Functional roman-numeral analysis relative to a key.
    FunctionalRoman,
    /// Plain set-theory prime-form labels.
    SetTheory,
    /// Neo-Riemannian / functional transformation labels.
    Riemannian,
    /// Jazz chord symbols (for example `Cmaj7`).
    Jazz,
}

impl NamingSchool {
    /// Returns the school's stable string tag (for example `"forte"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Forte => "forte",
            Self::FunctionalRoman => "roman",
            Self::SetTheory => "set-theory",
            Self::Riemannian => "riemann",
            Self::Jazz => "jazz",
        }
    }
}

impl fmt::Display for NamingSchool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The context a namer may use: an optional chord root and an optional key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LabelContext {
    /// The chord root, when known.
    pub root: Option<PitchClass>,
    /// The key, when a key-relative analysis is requested.
    pub key: Option<Key>,
}

/// Metadata attached to a produced [`ClusterLabel`], recording the inputs used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterMeta {
    /// The normalized (prime-form) mask the label was computed from.
    pub canonical_mask: PitchClassMask,
    /// The chord root used, if any.
    pub root: Option<PitchClass>,
    /// The key used, if any.
    pub key: Option<Key>,
    /// An optional diagnostic explaining a degraded or partial label.
    pub diagnostic: Option<String>,
}

/// A label produced by one naming school for one pitch-class set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterLabel {
    /// The school that produced the label.
    pub school: NamingSchool,
    /// The label text.
    pub text: String,
    /// Metadata about how the label was derived.
    pub meta: ClusterMeta,
}

/// Error returned by naming-registry operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PitchNamerError {
    /// A requested naming school was not present in the registry.
    #[error("unknown naming school {0}")]
    UnknownSchool(String),
}

/// A namer that labels a pitch-class set within one [`NamingSchool`].
pub trait ClusterNamer {
    /// Returns the school this namer belongs to.
    fn school(&self) -> NamingSchool;

    /// Produces a [`ClusterLabel`] for `mask` using `context`.
    fn label(&self, mask: PitchClassMask, context: &LabelContext) -> ClusterLabel;
}
