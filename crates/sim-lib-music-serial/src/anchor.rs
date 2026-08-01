//! Referential emphasis metadata that preserves row identity.

/// Non-pitch emphasis metadata attached around a referential subset.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReferentialEmphasis {
    /// Preferred register focus around the subset, if any.
    pub register_focus: Option<i16>,
    /// Rhythmic emphasis description, such as `hemiola` or `long-short-short`.
    pub rhythm_profile: Option<String>,
    /// Dynamic emphasis description, such as `sforzando` or `terraced`.
    pub dynamic_profile: Option<String>,
    /// Timbral emphasis description, such as `muted-brass`.
    pub timbral_profile: Option<String>,
    /// Harmonic emphasis description, such as `pedal` or `voicing spread`.
    pub harmonic_profile: Option<String>,
}
