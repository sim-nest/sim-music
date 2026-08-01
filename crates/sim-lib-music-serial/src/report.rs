//! Practice reports over named serial readings.

use crate::{InvariantLedger, PracticeId, PracticeRuleId, SerialReading};

/// Reproducible serial-practice report for one named reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialPracticeReport {
    /// Practice identity.
    pub practice_id: PracticeId,
    /// Reading that produced this report.
    pub reading: SerialReading,
    /// Invariant ledger for the reading.
    pub ledger: InvariantLedger<PracticeRuleId>,
}

impl SerialPracticeReport {
    /// Returns whether any invariant remains violated after considering waivers.
    pub fn has_unwaived_violations(&self) -> bool {
        self.ledger
            .entries()
            .iter()
            .any(|entry| matches!(entry.status, crate::InvariantStatus::Violated))
    }
}
