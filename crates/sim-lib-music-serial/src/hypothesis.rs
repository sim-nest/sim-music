//! Ranked serial-row extraction hypotheses and stable evidence records.

use sim_lib_music_core::{ObjectId, Time};
use sim_lib_pitch_serial::{RowClassAlias, ToneRow};

/// Exact half-open span used by extracted serial evidence.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SerialTimeSpan {
    /// Inclusive start.
    pub start: Time,
    /// Exclusive end.
    pub end: Time,
}

impl SerialTimeSpan {
    /// Builds an ordered half-open span.
    pub fn new(start: Time, end: Time) -> Self {
        debug_assert!(start <= end);
        Self { start, end }
    }

    /// Returns the exact duration.
    pub fn duration(&self) -> Time {
        self.end - self.start
    }
}

/// Stable sort key used when ranking extraction hypotheses.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SerialStableRank {
    /// Count of missing pitch classes from a complete twelve-tone aggregate.
    pub omissions: usize,
    /// Count of repeated pitch classes observed before aggregate completion.
    pub duplicates_before_completion: usize,
    /// Count of repeated pitch classes observed after aggregate completion.
    pub order_errors: usize,
    /// Exact span from the first contributing attack to the last contributing release.
    pub occupied_span: Time,
    /// Stable tie-breaker derived from the chosen attack ordering.
    pub stable_key: String,
}

/// Ordering policy selected for one same-onset attack block.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SerialReadingOrder {
    /// Preserve the canonical exact-window note order.
    WindowOrder,
    /// Sort equal-onset attacks by pitch, low to high.
    PitchAscending,
    /// Sort equal-onset attacks by pitch, high to low.
    PitchDescending,
    /// Sort equal-onset attacks by voice id, low to high.
    VoiceAscending,
    /// Sort equal-onset attacks by voice id, high to low.
    VoiceDescending,
}

impl SerialReadingOrder {
    /// Stable machine-readable name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WindowOrder => "window-order",
            Self::PitchAscending => "pitch-ascending",
            Self::PitchDescending => "pitch-descending",
            Self::VoiceAscending => "voice-ascending",
            Self::VoiceDescending => "voice-descending",
        }
    }
}

/// One note attack cited by a ranked hypothesis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialObservation {
    /// Voice identity.
    pub voice_id: ObjectId,
    /// Source note identity.
    pub note_id: ObjectId,
    /// Source event identity.
    pub event_id: ObjectId,
    /// Zero-based pitch-class ordinal in the hypothesis row.
    pub ordinal: usize,
    /// Exact source attack span inherited from the sounding-window partition.
    pub span: SerialTimeSpan,
}

/// One exact same-onset block chosen during extraction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialObservationBlock {
    /// Exact span for the block's source sounding window.
    pub span: SerialTimeSpan,
    /// Ordering policy chosen for this block.
    pub order: SerialReadingOrder,
    /// Notes contributing new or repeated row evidence in chosen order.
    pub observations: Vec<SerialObservation>,
}

/// Alias evidence attached to a ranked row hypothesis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialAliasEvidence {
    /// The alias operation preserved by the observed row class.
    pub alias: RowClassAlias,
    /// Stable printable form such as `P0` or `RI11`.
    pub label: String,
}

/// One ranked hypothesis extracted from exact source attacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankedSerialHypothesis {
    /// Stable ranking key; lower values are stronger.
    pub stable_rank: SerialStableRank,
    /// Observed row order inferred from first pitch-class occurrence.
    pub row: ToneRow,
    /// Exact same-onset blocks cited by the hypothesis.
    pub blocks: Vec<SerialObservationBlock>,
    /// Count of repeated pitch classes seen before aggregate completion.
    pub duplicates_before_completion: usize,
    /// Count of repeated pitch classes seen after aggregate completion.
    pub order_errors: usize,
    /// Missing pitch classes relative to an exact twelve-tone aggregate.
    pub omissions: usize,
    /// Exact span covered by the hypothesis.
    pub span: SerialTimeSpan,
    /// P/I/R/RI alias evidence for the observed row class.
    pub aliases: Vec<SerialAliasEvidence>,
}
