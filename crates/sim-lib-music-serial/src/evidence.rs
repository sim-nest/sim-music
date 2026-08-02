//! Search receipts and public outcomes for serial-row extraction.

use sim_lib_discrete_search::SearchReceipt;

use crate::RankedSerialHypothesis;

/// Search evidence returned alongside every extraction result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractionEvidence {
    /// Exact bounded-search receipt from the generic discrete owner.
    pub search: SearchReceipt,
    /// Stable description of the source attack groups considered.
    pub source_summary: Vec<String>,
}

/// Public extraction result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtractionOutcome {
    /// Exactly one strongest hypothesis survived and the search exhausted.
    Complete {
        /// Selected hypothesis.
        hypothesis: Box<RankedSerialHypothesis>,
        /// Ranked alternatives, including `hypothesis` as the first entry.
        ranked: Vec<RankedSerialHypothesis>,
        /// Search evidence.
        evidence: ExtractionEvidence,
    },
    /// Multiple plausible hypotheses remain after a complete bounded run.
    Ambiguous {
        /// Ranked hypotheses in stable ascending order.
        ranked: Vec<RankedSerialHypothesis>,
        /// Search evidence.
        evidence: ExtractionEvidence,
    },
    /// The generic search control stopped the run before exhaustion.
    BudgetExhausted {
        /// Ranked hypotheses found before the bound stopped the run.
        ranked: Vec<RankedSerialHypothesis>,
        /// Search evidence.
        evidence: ExtractionEvidence,
    },
}
