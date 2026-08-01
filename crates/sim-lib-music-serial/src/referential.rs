//! Bounded referential and tonal-anchor analysis for serial subsets.

use sim_lib_pitch_chord::ChordTemplate;
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_namer_roman::label_roman;
use sim_lib_pitch_ratio::{PitchRatio, RatioPolicy, analyze_ratio_chord};
use sim_lib_pitch_scale::{Key, Scale};
use sim_lib_pitch_serial::RowForm;
use sim_lib_pitch_set::PitchClassMask;

use crate::{ReferentialEmphasis, techniques::referential::build_referential_report};

/// Bounded referential claims made over one row subset.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReferentialClaim {
    /// The named evidence is embedded in the requested subset.
    EmbeddedSubset,
    /// The named evidence does not match the requested subset.
    NoReferentialSubset,
}

/// The evidence category backing a referential claim.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReferentialEvidenceKind {
    /// Named chord evidence.
    Chord,
    /// Named scale evidence.
    Scale,
    /// Named key-region evidence.
    KeyRegion,
    /// Exact pitch-ratio evidence.
    PitchRatio,
    /// Caller-supplied pitch-class mask evidence.
    Mask,
}

/// Supported named evidence for one referential subset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReferentialEvidence {
    /// A named chord-template target.
    Chord {
        /// Stable caller-facing label for the chord evidence.
        label: String,
        /// Exact chord template used to name the subset.
        chord: ChordTemplate,
    },
    /// A named scale target.
    Scale {
        /// Stable caller-facing label for the scale evidence.
        label: String,
        /// Exact scale used to test subset membership.
        scale: Scale,
    },
    /// A named key-region target with optional preferred root for labeling.
    KeyRegion {
        /// Stable caller-facing label for the key-region evidence.
        label: String,
        /// Key context used for membership and roman-numeral naming.
        key: Key,
        /// Preferred root used when the subset needs an explicit harmonic root.
        preferred_root: Option<PitchClass>,
    },
    /// Exact ratio evidence for the subset's voices.
    PitchRatio {
        /// Stable caller-facing label for the ratio evidence.
        label: String,
        /// Exact root-relative ratios attached to the subset.
        ratios: Vec<PitchRatio>,
        /// Canonicalization policy used while evaluating the ratios.
        policy: RatioPolicy,
    },
    /// Caller-declared mask evidence.
    Mask {
        /// Stable caller-facing label for the mask evidence.
        label: String,
        /// Exact unordered pitch-class evidence supplied by the caller.
        mask: PitchClassMask,
    },
}

impl ReferentialEvidence {
    /// Returns the stable evidence label supplied by the caller.
    pub fn label(&self) -> &str {
        match self {
            Self::Chord { label, .. }
            | Self::Scale { label, .. }
            | Self::KeyRegion { label, .. }
            | Self::PitchRatio { label, .. }
            | Self::Mask { label, .. } => label,
        }
    }

    /// Returns the evidence kind.
    pub const fn kind(&self) -> ReferentialEvidenceKind {
        match self {
            Self::Chord { .. } => ReferentialEvidenceKind::Chord,
            Self::Scale { .. } => ReferentialEvidenceKind::Scale,
            Self::KeyRegion { .. } => ReferentialEvidenceKind::KeyRegion,
            Self::PitchRatio { .. } => ReferentialEvidenceKind::PitchRatio,
            Self::Mask { .. } => ReferentialEvidenceKind::Mask,
        }
    }
}

/// Exact ratio/sonance summary attached to one claim.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferentialRatioSummary {
    /// Policy used to admit and normalize ratios.
    pub policy: RatioPolicy,
    /// Number of admitted tones.
    pub admitted_tones: usize,
    /// Number of rejected tones.
    pub rejected_tones: usize,
    /// Generalized-mean complexity cost.
    pub cost: f64,
}

/// Exact naming and sonance context behind one claim.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferentialContextReport {
    /// Evidence category backing the analysis.
    pub evidence_kind: ReferentialEvidenceKind,
    /// Caller-supplied evidence label.
    pub evidence_label: String,
    /// Exact text claim, when one can be named.
    pub harmonic_claim: Option<String>,
    /// Exact pitch classes at the requested ordinals.
    pub subset_classes: Vec<PitchClass>,
    /// Unordered subset projection.
    pub subset_mask: PitchClassMask,
    /// One-based scale degrees reached under scale/key evidence.
    pub scale_degrees: Vec<usize>,
    /// Optional exact ratio/sonance summary.
    pub ratio_summary: Option<ReferentialRatioSummary>,
}

/// One request to analyze a row subset as bounded referential evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferentialRequest {
    /// Zero-based source ordinals whose subset is being named.
    pub ordinals: Vec<usize>,
    /// Named evidence attached to the subset.
    pub evidence: ReferentialEvidence,
    /// Optional non-pitch emphasis around the subset.
    pub emphasis: ReferentialEmphasis,
}

/// Full bounded report over one referential subset.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferentialReport {
    /// The bounded claim result.
    pub claim: ReferentialClaim,
    /// Requested source ordinals echoed exactly.
    pub ordinals: Vec<usize>,
    /// Exact context behind the claim.
    pub context: ReferentialContextReport,
    /// Non-pitch emphasis retained around the subset.
    pub emphasis: ReferentialEmphasis,
    /// Always `false`: this surface never escalates one subset into whole-passage tonality.
    pub claims_whole_passage_tonality: bool,
}

/// Analyzes one row subset against named evidence without asserting global tonality.
pub fn analyze_referential_subset(
    row_form: &RowForm,
    request: ReferentialRequest,
) -> Result<ReferentialReport, String> {
    build_referential_report(row_form, request)
}

pub(crate) fn roman_claim(
    subset_mask: PitchClassMask,
    key: Key,
    preferred_root: Option<PitchClass>,
) -> Option<String> {
    label_roman(subset_mask, Some(key), preferred_root).ok()
}

pub(crate) fn scale_degrees(scale: Scale, classes: &[PitchClass]) -> Vec<usize> {
    classes
        .iter()
        .filter_map(|class| scale.degree_of(*class))
        .collect()
}

pub(crate) fn ratio_summary(
    ratios: &[PitchRatio],
    policy: RatioPolicy,
) -> Result<ReferentialRatioSummary, String> {
    let report = analyze_ratio_chord(ratios, policy).map_err(|error| error.to_string())?;
    Ok(ReferentialRatioSummary {
        policy,
        admitted_tones: report.covered.admitted_tones,
        rejected_tones: report.covered.rejected_tones,
        cost: report.cost,
    })
}
