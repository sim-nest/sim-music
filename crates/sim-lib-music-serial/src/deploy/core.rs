use std::collections::BTreeMap;
use std::sync::Arc;

use sim_lib_pitch_serial::RowForm;
use thiserror::Error;

use crate::{
    BuiltInPracticeRule, EventPlacement, OrdinalRef, PlannedSerialEvent, PracticeRule,
    PracticeRuleId, RowInstanceId, SerialEventId, SerialOrigin, SerialPlan, SerialPractice,
    SerialRole, StructuralLicense, VoiceId,
};

/// Failure while composing one inspectable serial deployment plan.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum SerialDeployError {
    /// The supplied technique id was empty or used an invalid character.
    #[error("{0}")]
    InvalidTechniqueId(String),
    /// The technique attempted to build without any deployers.
    #[error("technique {0} must contain at least one deployer")]
    EmptyTechnique(String),
    /// A deployer named a row instance missing from the deployment input.
    #[error("deployer {deployer} references unknown row {row_id}")]
    UnknownRow {
        /// Stable deployer label.
        deployer: String,
        /// Missing row id.
        row_id: RowInstanceId,
    },
    /// A deployer attempted to reuse an event id already emitted earlier.
    #[error("deployer {deployer} attempted to reuse event id {event_id}")]
    DuplicateEventId {
        /// Stable deployer label.
        deployer: String,
        /// Duplicate event id.
        event_id: SerialEventId,
    },
    /// A deployer requiring one voice per block or form received the wrong count.
    #[error("deployer {deployer} expected {expected} voices but received {actual}")]
    VoiceCountMismatch {
        /// Stable deployer label.
        deployer: String,
        /// Required voice count.
        expected: usize,
        /// Received voice count.
        actual: usize,
    },
    /// Aggregate-rotation block lengths failed to cover exactly one row.
    #[error("aggregate rotation block lengths must sum to 12, received {0}")]
    InvalidRotationCoverage(usize),
    /// The caller requested an interlocking deployment without an interlocking witness.
    #[error("deployer {deployer} requires an interlocking partition witness")]
    NotInterlocking {
        /// Stable deployer label.
        deployer: String,
    },
    /// The requested simultaneous forms were not combinatorial at the block size.
    #[error(
        "rows {source_row_id} and {partner_row_id} are not combinatorial at block size {block_size}"
    )]
    NotCombinatorial {
        /// Source row id.
        source_row_id: RowInstanceId,
        /// Partner row id.
        partner_row_id: RowInstanceId,
        /// Requested contiguous block size.
        block_size: usize,
    },
    /// Building or validating a pitch-serial partition failed.
    #[error("partition build failed: {0}")]
    Partition(String),
    /// Final immutable serial-plan validation failed.
    #[error("serial plan validation failed: {0}")]
    Plan(String),
}

pub(crate) fn validate_technique_id(value: impl Into<String>) -> Result<String, SerialDeployError> {
    let value = value.into();
    if value.trim().is_empty() {
        return Err(SerialDeployError::InvalidTechniqueId(
            "technique id cannot be empty".to_owned(),
        ));
    }
    if value
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-' | '_' | '.')))
    {
        return Err(SerialDeployError::InvalidTechniqueId(
            "technique id must use ASCII letters, digits, /, -, _, or .".to_owned(),
        ));
    }
    Ok(value)
}

/// Inspectable parameter attached to one deployer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialDeployerParameter {
    /// Stable parameter name.
    pub name: String,
    /// Stable printable value.
    pub value: String,
}

/// Public category of one serial deployer.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SerialDeployerKind {
    /// Present one complete row in sequence.
    CompleteHorizontalStatement,
    /// Present a caller-declared partition sequentially.
    MotivicPartition,
    /// Present selected blocks vertically as chordal events.
    VerticalBlocks,
    /// Require an interlocking partition witness before deployment.
    InterlockingPartition,
    /// Distribute each block between melody and accompaniment.
    MelodyAccompanimentDistribution,
    /// Rotate the aggregate before reblocking it.
    AggregateRotation,
    /// Present several forms simultaneously by aligned blocks.
    SimultaneousForms,
}

/// Inspectable public description of one deployment component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialDeployerSpec {
    /// Public deployer category.
    pub kind: SerialDeployerKind,
    /// Stable deployer label.
    pub label: String,
    /// Human-facing expected fact or usage.
    pub expected_fact: String,
    /// Inspectable deployer parameters.
    pub parameters: Vec<SerialDeployerParameter>,
}

#[derive(Clone)]
pub(crate) enum SerialDeployerInner {
    Horizontal(super::components::HorizontalStatementSpec),
    Motivic(super::components::MotivicPartitionSpec),
    Vertical(super::components::VerticalBlocksSpec),
    Interlocking(super::components::InterlockingPartitionSpec),
    MelodyAccompaniment(super::components::MelodyAccompanimentSpec),
    AggregateRotation(super::components::AggregateRotationSpec),
    SimultaneousForms(super::components::SimultaneousFormsSpec),
}

/// One reusable inspectable deployment component.
#[derive(Clone)]
pub struct SerialDeployer {
    pub(crate) inner: SerialDeployerInner,
}

impl SerialDeployer {
    pub(crate) fn spec(&self) -> SerialDeployerSpec {
        match &self.inner {
            SerialDeployerInner::Horizontal(spec) => SerialDeployerSpec {
                kind: SerialDeployerKind::CompleteHorizontalStatement,
                label: spec.event_id.as_str().to_owned(),
                expected_fact: "one complete row sounds in sequence".to_owned(),
                parameters: vec![
                    param("row", spec.row_id.as_str()),
                    param("voice", &spec.voice.to_string()),
                    param("reading", spec.license.reading_id.as_str()),
                ],
            },
            SerialDeployerInner::Motivic(spec) => SerialDeployerSpec {
                kind: SerialDeployerKind::MotivicPartition,
                label: spec.event_prefix.clone(),
                expected_fact: "partition blocks sound sequentially as inspectable motives"
                    .to_owned(),
                parameters: vec![
                    param("row", spec.row_id.as_str()),
                    param("blocks", &spec.partition.block_count().to_string()),
                    param("reading", spec.license.reading_id.as_str()),
                ],
            },
            SerialDeployerInner::Vertical(spec) => SerialDeployerSpec {
                kind: SerialDeployerKind::VerticalBlocks,
                label: spec.event_prefix.clone(),
                expected_fact: "selected blocks sound as chordal vertical events".to_owned(),
                parameters: vec![
                    param("row", spec.row_id.as_str()),
                    param("blocks", &format!("{:?}", spec.selected_blocks)),
                    param("reading", spec.license.reading_id.as_str()),
                ],
            },
            SerialDeployerInner::Interlocking(spec) => SerialDeployerSpec {
                kind: SerialDeployerKind::InterlockingPartition,
                label: spec.event_prefix.clone(),
                expected_fact: "interlocking block evidence licenses sequential partition exchange"
                    .to_owned(),
                parameters: vec![
                    param("row", spec.row_id.as_str()),
                    param("blocks", &spec.partition.block_count().to_string()),
                    param("reading", spec.license.reading_id.as_str()),
                ],
            },
            SerialDeployerInner::MelodyAccompaniment(spec) => SerialDeployerSpec {
                kind: SerialDeployerKind::MelodyAccompanimentDistribution,
                label: spec.event_prefix.clone(),
                expected_fact: "each block splits into melody lead and accompaniment residue"
                    .to_owned(),
                parameters: vec![
                    param("row", spec.row_id.as_str()),
                    param("blocks", &spec.partition.block_count().to_string()),
                    param("reading", spec.license.reading_id.as_str()),
                ],
            },
            SerialDeployerInner::AggregateRotation(spec) => SerialDeployerSpec {
                kind: SerialDeployerKind::AggregateRotation,
                label: spec.event_prefix.clone(),
                expected_fact: "a rotated aggregate is reblocked without losing row coverage"
                    .to_owned(),
                parameters: vec![
                    param("row", spec.row_id.as_str()),
                    param("rotation", &spec.rotation.to_string()),
                    param("reading", spec.license.reading_id.as_str()),
                ],
            },
            SerialDeployerInner::SimultaneousForms(spec) => SerialDeployerSpec {
                kind: SerialDeployerKind::SimultaneousForms,
                label: spec.event_prefix.clone(),
                expected_fact: "simultaneous form blocks preserve each form identity and alignment"
                    .to_owned(),
                parameters: vec![
                    param(
                        "forms",
                        &spec
                            .row_ids
                            .iter()
                            .map(RowInstanceId::as_str)
                            .collect::<Vec<_>>()
                            .join(","),
                    ),
                    param("block-size", &spec.block_size.to_string()),
                    param("reading", spec.license.reading_id.as_str()),
                ],
            },
        }
    }

    pub(crate) fn apply(
        &self,
        rows: &BTreeMap<RowInstanceId, RowForm>,
        builder: &mut PlanBuilder,
    ) -> Result<(), SerialDeployError> {
        match &self.inner {
            SerialDeployerInner::Horizontal(spec) => spec.apply(rows, builder),
            SerialDeployerInner::Motivic(spec) => spec.apply(rows, builder),
            SerialDeployerInner::Vertical(spec) => spec.apply(rows, builder),
            SerialDeployerInner::Interlocking(spec) => spec.apply(rows, builder),
            SerialDeployerInner::MelodyAccompaniment(spec) => spec.apply(rows, builder),
            SerialDeployerInner::AggregateRotation(spec) => spec.apply(rows, builder),
            SerialDeployerInner::SimultaneousForms(spec) => spec.apply(rows, builder),
        }
    }
}

pub(crate) fn param(name: &str, value: &str) -> SerialDeployerParameter {
    SerialDeployerParameter {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PlanBuilder {
    pub(crate) events: BTreeMap<SerialEventId, PlannedSerialEvent>,
    pub(crate) precedence: Vec<(SerialEventId, SerialEventId)>,
}

impl PlanBuilder {
    pub(crate) fn add_event(
        &mut self,
        deployer: &str,
        event: PlannedSerialEvent,
    ) -> Result<(), SerialDeployError> {
        if self.events.contains_key(&event.id) {
            return Err(SerialDeployError::DuplicateEventId {
                deployer: deployer.to_owned(),
                event_id: event.id,
            });
        }
        self.events.insert(event.id.clone(), event);
        Ok(())
    }

    pub(crate) fn add_precedence(&mut self, before: &SerialEventId, after: &SerialEventId) {
        self.precedence.push((before.clone(), after.clone()));
    }
}

/// Inspectable deployment plan composed from ordinary practice rules and deployers.
#[derive(Clone)]
pub struct TechniquePlan {
    id: String,
    rules: Vec<Arc<dyn PracticeRule>>,
    deployers: Vec<SerialDeployer>,
}

impl TechniquePlan {
    /// Starts a builder for one inspectable deployment technique.
    pub fn builder(id: impl Into<String>) -> Result<TechniquePlanBuilder, SerialDeployError> {
        Ok(TechniquePlanBuilder {
            id: validate_technique_id(id)?,
            rules: Vec::new(),
            deployers: Vec::new(),
        })
    }

    /// Returns the stable technique id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the inspectable practice rule specifications.
    pub fn rule_specs(&self) -> Vec<crate::PracticeRuleSpec> {
        self.practice().rule_specs()
    }

    /// Returns the inspectable deployer specifications.
    pub fn deployer_specs(&self) -> Vec<SerialDeployerSpec> {
        self.deployers.iter().map(SerialDeployer::spec).collect()
    }

    /// Returns the inspectable practice carried alongside the deployers.
    pub fn practice(&self) -> SerialPractice {
        SerialPractice::new(
            crate::PracticeId::new(self.id.clone()).expect("validated technique id"),
            self.rules.clone(),
        )
    }

    /// Deploys the supplied row instances into one validated immutable serial plan.
    pub fn deploy(
        &self,
        rows: BTreeMap<RowInstanceId, RowForm>,
    ) -> Result<SerialPlan, SerialDeployError> {
        let mut builder = PlanBuilder {
            events: BTreeMap::new(),
            precedence: Vec::new(),
        };
        for deployer in &self.deployers {
            deployer.apply(&rows, &mut builder)?;
        }
        SerialPlan::try_new(rows, builder.events, builder.precedence)
            .map_err(|error| SerialDeployError::Plan(error.to_string()))
    }
}

/// Builder for one inspectable technique plan.
pub struct TechniquePlanBuilder {
    id: String,
    rules: Vec<Arc<dyn PracticeRule>>,
    deployers: Vec<SerialDeployer>,
}

impl TechniquePlanBuilder {
    /// Adds one inspectable practice rule.
    pub fn rule(mut self, rule: Arc<dyn PracticeRule>) -> Self {
        self.rules.push(rule);
        self
    }

    /// Adds one inspectable deployment component.
    pub fn deployer(mut self, deployer: SerialDeployer) -> Self {
        self.deployers.push(deployer);
        self
    }

    /// Finishes the technique plan after validating its minimum structure.
    pub fn build(self) -> Result<TechniquePlan, SerialDeployError> {
        if self.deployers.is_empty() {
            return Err(SerialDeployError::EmptyTechnique(self.id));
        }
        Ok(TechniquePlan {
            id: self.id,
            rules: self.rules,
            deployers: self.deployers,
        })
    }
}

/// Convenience helper for the built-in strict aggregate rule.
pub fn strict_aggregate() -> Arc<dyn PracticeRule> {
    Arc::new(BuiltInPracticeRule::aggregate(
        PracticeRuleId::new("rule/aggregate").expect("static rule id"),
    ))
}

pub(crate) fn require_row(
    deployer: &str,
    rows: &BTreeMap<RowInstanceId, RowForm>,
    row_id: &RowInstanceId,
) -> Result<(), SerialDeployError> {
    if rows.contains_key(row_id) {
        Ok(())
    } else {
        Err(SerialDeployError::UnknownRow {
            deployer: deployer.to_owned(),
            row_id: row_id.clone(),
        })
    }
}

pub(crate) fn require_voices(
    deployer: &str,
    expected: usize,
    actual: usize,
) -> Result<(), SerialDeployError> {
    if expected == actual {
        Ok(())
    } else {
        Err(SerialDeployError::VoiceCountMismatch {
            deployer: deployer.to_owned(),
            expected,
            actual,
        })
    }
}

pub(crate) fn structural_event(
    event_id: SerialEventId,
    ordinals: &[u8],
    row_id: RowInstanceId,
    voice: VoiceId,
    rationale: String,
    license: StructuralLicense,
    placement: EventPlacement,
) -> PlannedSerialEvent {
    PlannedSerialEvent {
        id: event_id,
        ordinals: ordinals
            .iter()
            .copied()
            .map(|ordinal| OrdinalRef::new(row_id.clone(), usize::from(ordinal)))
            .collect(),
        role: SerialRole::Structural,
        origin: SerialOrigin::Structural { rationale },
        voice,
        placement,
        parents: Vec::new(),
        licenses: vec![license],
    }
}
