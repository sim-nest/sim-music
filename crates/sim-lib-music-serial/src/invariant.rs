//! Practice invariant ledgers and explicit relaxation evidence.

use std::fmt::{Display, Formatter};

/// Stable identity for one declared waiver.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WaiverId(String);

/// Stable identity for one evidence item attached to a practice finding.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceId(String);

fn validate_id(kind: &'static str, value: impl Into<String>) -> Result<String, String> {
    let value = value.into();
    if value.trim().is_empty() {
        return Err(format!("{kind} cannot be empty"));
    }
    if value
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-' | '_' | '.')))
    {
        return Err(format!(
            "{kind} must use ASCII letters, digits, /, -, _, or ."
        ));
    }
    Ok(value)
}

macro_rules! stable_id {
    ($name:ident, $kind:literal, $doc:literal) => {
        #[doc = $doc]
        impl $name {
            /// Creates a validated stable identifier.
            pub fn new(value: impl Into<String>) -> Result<Self, String> {
                Ok(Self(validate_id($kind, value)?))
            }

            /// Returns the stable text identity.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Display for $name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

stable_id!(
    WaiverId,
    "waiver-id",
    "Stable identity for one declared practice waiver."
);
stable_id!(
    EvidenceId,
    "evidence-id",
    "Stable identity for one invariant-evidence record."
);

/// Status of one declared serial-practice invariant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvariantStatus {
    /// The expected fact held exactly under the selected reading.
    Preserved,
    /// The invariant would fail, but one explicit waiver declared the relaxation.
    Relaxed {
        /// Stable waiver id that authorized the relaxation.
        waiver: WaiverId,
    },
    /// The invariant failed without a declared waiver.
    Violated,
    /// The selected reading did not expose evidence for this invariant.
    NotApplicable,
    /// The invariant could not be classified decisively.
    Unknown,
}

/// One inspectable invariant result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvariantLedgerEntry<R> {
    /// Stable rule identity.
    pub rule_id: R,
    /// Human-readable expected fact.
    pub expected_fact: String,
    /// Human-readable observed fact.
    pub observed_fact: String,
    /// Classified invariant status.
    pub status: InvariantStatus,
    /// Stable evidence items supporting the observation.
    pub evidence_ids: Vec<EvidenceId>,
    /// Explicit declared waiver, if any.
    pub declared_waiver: Option<WaiverId>,
}

impl<R> InvariantLedgerEntry<R> {
    pub(crate) fn new(
        rule_id: R,
        expected_fact: impl Into<String>,
        observed_fact: impl Into<String>,
        status: InvariantStatus,
        evidence_ids: Vec<EvidenceId>,
        declared_waiver: Option<WaiverId>,
    ) -> Self {
        Self {
            rule_id,
            expected_fact: expected_fact.into(),
            observed_fact: observed_fact.into(),
            status,
            evidence_ids,
            declared_waiver,
        }
    }
}

/// Complete invariant bundle for one selected reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvariantLedger<R> {
    entries: Vec<InvariantLedgerEntry<R>>,
}

impl<R> InvariantLedger<R> {
    /// Creates one immutable invariant ledger.
    pub fn new(entries: Vec<InvariantLedgerEntry<R>>) -> Self {
        Self { entries }
    }

    /// Returns the recorded invariant entries in stable rule order.
    pub fn entries(&self) -> &[InvariantLedgerEntry<R>] {
        &self.entries
    }
}
