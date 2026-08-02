//! Event role and provenance contracts.

/// Structural role in the serial plan.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SerialRole {
    /// One or more structural row ordinals presented directly.
    Structural,
    /// Material derived from prior structural or derived events.
    Derived,
    /// Non-structural ornament built around explicit parent material.
    Ornamental,
    /// Imported or externally motivated material attached to explicit parents.
    External,
}

impl SerialRole {
    /// Returns the stable role token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Structural => "structural",
            Self::Derived => "derived",
            Self::Ornamental => "ornamental",
            Self::External => "external",
        }
    }
}

/// Additional immutable provenance for one planned event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SerialOrigin {
    /// The event states structural row material directly.
    Structural {
        /// Human-facing note about the structural statement.
        rationale: String,
    },
    /// The event derives from explicit parents through a named technique.
    Derived {
        /// Stable technique or transform name.
        technique: String,
    },
    /// The event ornaments explicit parents without claiming structural coverage.
    Ornamental {
        /// Stable ornament technique name.
        technique: String,
    },
    /// The event imports external material but still cites explicit parent evidence.
    External {
        /// Stable external source or rationale tag.
        source: String,
    },
}
