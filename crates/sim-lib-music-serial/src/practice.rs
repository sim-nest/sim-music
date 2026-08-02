//! Inspectable serial-practice policies built from open rule components.

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use crate::practice_builtin::evaluate_builtin;
use crate::{
    InvariantLedger, InvariantLedgerEntry, SerialPlan, SerialPracticeReport, SerialReading,
    WaiverId,
};

/// Stable identity for one named serial practice.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PracticeId(String);

/// Stable identity for one inspectable practice rule.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PracticeRuleId(String);

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

            /// Returns the stable wire text.
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
    PracticeId,
    "practice-id",
    "Stable identity for one named serial practice."
);
stable_id!(
    PracticeRuleId,
    "practice-rule-id",
    "Stable identity for one serial practice rule."
);

/// Public category of one built-in practice rule.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PracticeRuleKind {
    /// Enforce one aggregate expectation over structural ordinals.
    Aggregate,
    /// Enforce one order expectation over first structural appearances.
    Order,
    /// Enforce one no-repeat expectation over ordinal references.
    Repeats,
    /// Enforce one no-doubling expectation inside simultaneous groups.
    Doublings,
    /// Enforce one simultaneity policy.
    Simultaneity,
    /// Enforce one no-row-mixing expectation inside an event.
    RowMixing,
    /// Enforce one policy for externally sourced material.
    ForeignMaterial,
    /// Enforce one policy for non-structural reuse after parameter exhaustion.
    ParameterExhaustion,
}

/// One inspectable parameter attached to a practice rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PracticeRuleParameter {
    /// Stable parameter name.
    pub name: String,
    /// Stable printable value.
    pub value: String,
}

/// Public inspectable description of one practice rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PracticeRuleSpec {
    /// Stable rule identity.
    pub id: PracticeRuleId,
    /// Built-in rule kind.
    pub kind: PracticeRuleKind,
    /// Expected fact enforced by the rule.
    pub expected_fact: String,
    /// Inspectable policy parameters.
    pub parameters: Vec<PracticeRuleParameter>,
}

/// Open rule component used by one serial practice.
pub trait PracticeRule: Send + Sync {
    /// Returns the stable rule identity.
    fn id(&self) -> &PracticeRuleId;

    /// Returns the inspectable rule specification.
    fn spec(&self) -> PracticeRuleSpec;

    /// Evaluates this rule over one named reading.
    fn evaluate(
        &self,
        plan: &SerialPlan,
        reading: SerialReading,
        waivers: &DeclaredWaivers,
    ) -> InvariantLedgerEntry<PracticeRuleId>;
}

/// Explicit waivers declared for one practice run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeclaredWaivers {
    by_rule: BTreeMap<PracticeRuleId, WaiverId>,
}

impl DeclaredWaivers {
    /// Creates a waiver set from stable rule/waiver pairs.
    pub fn new(entries: impl IntoIterator<Item = (PracticeRuleId, WaiverId)>) -> Self {
        Self {
            by_rule: entries.into_iter().collect(),
        }
    }

    pub(crate) fn waiver_for(&self, rule_id: &PracticeRuleId) -> Option<WaiverId> {
        self.by_rule.get(rule_id).cloned()
    }
}

/// Named serial practice composed from open rule components.
#[derive(Clone)]
pub struct SerialPractice {
    /// Stable practice identity.
    pub id: PracticeId,
    /// Open rule components evaluated in order.
    pub rules: Vec<Arc<dyn PracticeRule>>,
}

impl SerialPractice {
    /// Builds one named serial practice from open rule components.
    pub fn new(id: PracticeId, rules: Vec<Arc<dyn PracticeRule>>) -> Self {
        Self { id, rules }
    }

    /// Returns the inspectable built-in and custom rule specifications.
    pub fn rule_specs(&self) -> Vec<PracticeRuleSpec> {
        self.rules.iter().map(|rule| rule.spec()).collect()
    }

    /// Evaluates every rule over one named reading.
    pub fn evaluate(
        &self,
        plan: &SerialPlan,
        reading: SerialReading,
        waivers: &DeclaredWaivers,
    ) -> SerialPracticeReport {
        let entries = self
            .rules
            .iter()
            .map(|rule| rule.evaluate(plan, reading, waivers))
            .collect();
        SerialPracticeReport {
            practice_id: self.id.clone(),
            reading,
            ledger: InvariantLedger::new(entries),
        }
    }
}

/// Built-in inspectable rule implementations for common serial-practice checks.
#[derive(Clone, Debug)]
pub struct BuiltInPracticeRule {
    spec: PracticeRuleSpec,
    evaluator: BuiltInRuleEvaluator,
}

#[derive(Clone, Debug)]
pub(crate) enum BuiltInRuleEvaluator {
    Aggregate,
    Order,
    Repeats,
    Doublings,
    Simultaneity { allow: bool },
    RowMixing,
    ForeignMaterial { allow_external: bool },
    ParameterExhaustion,
}

impl BuiltInPracticeRule {
    /// Requires every structural ordinal to appear exactly once under the reading.
    pub fn aggregate(id: PracticeRuleId) -> Self {
        Self {
            spec: PracticeRuleSpec {
                id,
                kind: PracticeRuleKind::Aggregate,
                expected_fact: "each structural ordinal appears exactly once".to_owned(),
                parameters: Vec::new(),
            },
            evaluator: BuiltInRuleEvaluator::Aggregate,
        }
    }

    /// Requires first structural appearances to remain in row order.
    pub fn order(id: PracticeRuleId) -> Self {
        Self {
            spec: PracticeRuleSpec {
                id,
                kind: PracticeRuleKind::Order,
                expected_fact: "first structural appearances preserve row order".to_owned(),
                parameters: Vec::new(),
            },
            evaluator: BuiltInRuleEvaluator::Order,
        }
    }

    /// Forbids repeated ordinal references under the reading.
    pub fn repeats(id: PracticeRuleId) -> Self {
        Self {
            spec: PracticeRuleSpec {
                id,
                kind: PracticeRuleKind::Repeats,
                expected_fact: "no structural ordinal repeats".to_owned(),
                parameters: Vec::new(),
            },
            evaluator: BuiltInRuleEvaluator::Repeats,
        }
    }

    /// Forbids simultaneous doublings of one row pitch class.
    pub fn doublings(id: PracticeRuleId) -> Self {
        Self {
            spec: PracticeRuleSpec {
                id,
                kind: PracticeRuleKind::Doublings,
                expected_fact: "simultaneous groups avoid doubled row pitch classes".to_owned(),
                parameters: Vec::new(),
            },
            evaluator: BuiltInRuleEvaluator::Doublings,
        }
    }

    /// Controls whether simultaneous groups are accepted.
    pub fn simultaneity(id: PracticeRuleId, allow: bool) -> Self {
        Self {
            spec: PracticeRuleSpec {
                id,
                kind: PracticeRuleKind::Simultaneity,
                expected_fact: if allow {
                    "simultaneous groups are explicitly permitted".to_owned()
                } else {
                    "no simultaneous groups occur".to_owned()
                },
                parameters: vec![PracticeRuleParameter {
                    name: "allow".to_owned(),
                    value: allow.to_string(),
                }],
            },
            evaluator: BuiltInRuleEvaluator::Simultaneity { allow },
        }
    }

    /// Forbids one event from mixing ordinals from multiple row instances.
    pub fn row_mixing(id: PracticeRuleId) -> Self {
        Self {
            spec: PracticeRuleSpec {
                id,
                kind: PracticeRuleKind::RowMixing,
                expected_fact: "each event cites exactly one row instance".to_owned(),
                parameters: Vec::new(),
            },
            evaluator: BuiltInRuleEvaluator::RowMixing,
        }
    }

    /// Controls whether externally sourced material is accepted.
    pub fn foreign_material(id: PracticeRuleId, allow_external: bool) -> Self {
        Self {
            spec: PracticeRuleSpec {
                id,
                kind: PracticeRuleKind::ForeignMaterial,
                expected_fact: if allow_external {
                    "external material is explicitly permitted".to_owned()
                } else {
                    "no external material occurs".to_owned()
                },
                parameters: vec![PracticeRuleParameter {
                    name: "allow_external".to_owned(),
                    value: allow_external.to_string(),
                }],
            },
            evaluator: BuiltInRuleEvaluator::ForeignMaterial { allow_external },
        }
    }

    /// Requires non-structural reuse after full aggregate exposure to be declared as a relaxation.
    pub fn parameter_exhaustion(id: PracticeRuleId) -> Self {
        Self {
            spec: PracticeRuleSpec {
                id,
                kind: PracticeRuleKind::ParameterExhaustion,
                expected_fact: "non-structural events do not reuse exhausted structural parameters"
                    .to_owned(),
                parameters: Vec::new(),
            },
            evaluator: BuiltInRuleEvaluator::ParameterExhaustion,
        }
    }
}

impl PracticeRule for BuiltInPracticeRule {
    fn id(&self) -> &PracticeRuleId {
        &self.spec.id
    }

    fn spec(&self) -> PracticeRuleSpec {
        self.spec.clone()
    }

    fn evaluate(
        &self,
        plan: &SerialPlan,
        reading: SerialReading,
        waivers: &DeclaredWaivers,
    ) -> InvariantLedgerEntry<PracticeRuleId> {
        let waived = waivers.waiver_for(self.id());
        evaluate_builtin(&self.evaluator, plan, reading, &self.spec, waived)
    }
}
