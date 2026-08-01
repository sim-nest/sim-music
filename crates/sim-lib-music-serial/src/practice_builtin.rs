//! Built-in serial-practice rule evaluation helpers.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use sim_lib_pitch_core::PitchClass;

use crate::practice::{BuiltInRuleEvaluator, PracticeRuleSpec};
use crate::{
    EvidenceId, InvariantLedgerEntry, InvariantStatus, OrdinalRef, PlannedSerialEvent,
    PracticeRuleId, RowInstanceId, SerialPlan, SerialReading, SerialRole, SimultaneousGroupId,
    WaiverId,
};

pub(crate) fn evaluate_builtin(
    evaluator: &BuiltInRuleEvaluator,
    plan: &SerialPlan,
    reading: SerialReading,
    spec: &PracticeRuleSpec,
    waived: Option<WaiverId>,
) -> InvariantLedgerEntry<PracticeRuleId> {
    match evaluator {
        BuiltInRuleEvaluator::Aggregate => evaluate_aggregate(plan, reading, spec, waived),
        BuiltInRuleEvaluator::Order => evaluate_order(plan, reading, spec, waived),
        BuiltInRuleEvaluator::Repeats => evaluate_repeats(plan, reading, spec, waived),
        BuiltInRuleEvaluator::Doublings => evaluate_doublings(plan, reading, spec, waived),
        BuiltInRuleEvaluator::Simultaneity { allow } => {
            evaluate_simultaneity(plan, reading, spec, waived, *allow)
        }
        BuiltInRuleEvaluator::RowMixing => evaluate_row_mixing(plan, reading, spec, waived),
        BuiltInRuleEvaluator::ForeignMaterial { allow_external } => {
            evaluate_foreign_material(plan, reading, spec, waived, *allow_external)
        }
        BuiltInRuleEvaluator::ParameterExhaustion => {
            evaluate_parameter_exhaustion(plan, reading, spec, waived)
        }
    }
}

fn selected_events(plan: &SerialPlan, reading: SerialReading) -> Vec<&PlannedSerialEvent> {
    plan.events()
        .values()
        .filter(|event| reading.includes_event(event))
        .collect()
}

fn structural_ordinals<'a>(
    events: impl IntoIterator<Item = &'a PlannedSerialEvent>,
) -> Vec<&'a OrdinalRef> {
    events
        .into_iter()
        .filter(|event| matches!(event.role, SerialRole::Structural))
        .flat_map(|event| event.ordinals.iter())
        .collect()
}

fn invariant_outcome(
    violated: bool,
    waived: Option<WaiverId>,
) -> (InvariantStatus, Option<WaiverId>) {
    match (violated, waived) {
        (false, waiver) => (InvariantStatus::Preserved, waiver),
        (true, Some(waiver)) => (
            InvariantStatus::Relaxed {
                waiver: waiver.clone(),
            },
            Some(waiver),
        ),
        (true, None) => (InvariantStatus::Violated, None),
    }
}

fn evidence(id: &'static str) -> EvidenceId {
    EvidenceId::new(id).expect("static evidence id")
}

fn topological_event_ids(plan: &SerialPlan, reading: SerialReading) -> Vec<crate::SerialEventId> {
    let selected = selected_events(plan, reading)
        .into_iter()
        .map(|event| event.id.clone())
        .collect::<BTreeSet<_>>();
    let mut indegree = selected
        .iter()
        .cloned()
        .map(|id| (id, 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing: BTreeMap<crate::SerialEventId, BTreeSet<crate::SerialEventId>> =
        BTreeMap::new();
    for (before, after) in plan.precedence().edges() {
        if selected.contains(before) && selected.contains(after) {
            outgoing
                .entry(before.clone())
                .or_default()
                .insert(after.clone());
            *indegree.get_mut(after).expect("selected node") += 1;
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<VecDeque<_>>();
    let mut ordered = Vec::with_capacity(selected.len());
    while let Some(next) = ready.pop_front() {
        ordered.push(next.clone());
        if let Some(targets) = outgoing.get(&next) {
            for target in targets {
                let degree = indegree.get_mut(target).expect("selected node");
                *degree -= 1;
                if *degree == 0 {
                    ready.push_back(target.clone());
                }
            }
        }
    }
    ordered
}

fn describe_ordinals(ordinals: &[OrdinalRef]) -> String {
    ordinals
        .iter()
        .map(|ordinal| format!("{}/{}", ordinal.row_id, ordinal.ordinal))
        .collect::<Vec<_>>()
        .join(", ")
}

fn evaluate_aggregate(
    plan: &SerialPlan,
    reading: SerialReading,
    spec: &PracticeRuleSpec,
    waived: Option<WaiverId>,
) -> InvariantLedgerEntry<PracticeRuleId> {
    let ordinals = structural_ordinals(selected_events(plan, reading));
    if ordinals.is_empty() {
        return InvariantLedgerEntry::new(
            spec.id.clone(),
            spec.expected_fact.clone(),
            "reading exposes no structural ordinals".to_owned(),
            InvariantStatus::NotApplicable,
            vec![evidence("evidence/aggregate/not-applicable")],
            waived,
        );
    }
    let mut counts = BTreeMap::<OrdinalRef, usize>::new();
    for ordinal in ordinals {
        *counts.entry(ordinal.clone()).or_default() += 1;
    }
    let missing = plan
        .rows()
        .iter()
        .flat_map(|(row_id, row)| {
            (0..row.classes().len())
                .map(|ordinal| OrdinalRef::new(row_id.clone(), ordinal))
                .collect::<Vec<_>>()
        })
        .filter(|ordinal| !counts.contains_key(ordinal))
        .collect::<Vec<_>>();
    let duplicates = counts
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(ordinal, _)| ordinal.clone())
        .collect::<Vec<_>>();
    let violated = !missing.is_empty() || !duplicates.is_empty();
    let (status, declared_waiver) = invariant_outcome(violated, waived);
    InvariantLedgerEntry::new(
        spec.id.clone(),
        spec.expected_fact.clone(),
        format!(
            "missing [{}]; duplicates [{}]",
            describe_ordinals(&missing),
            describe_ordinals(&duplicates)
        ),
        status,
        vec![evidence("evidence/aggregate/coverage")],
        declared_waiver,
    )
}

fn evaluate_order(
    plan: &SerialPlan,
    reading: SerialReading,
    spec: &PracticeRuleSpec,
    waived: Option<WaiverId>,
) -> InvariantLedgerEntry<PracticeRuleId> {
    let mut evidence_ids = vec![evidence("evidence/order/topological-reading")];
    let ordered_ids = topological_event_ids(plan, reading);
    if ordered_ids.is_empty() {
        return InvariantLedgerEntry::new(
            spec.id.clone(),
            spec.expected_fact.clone(),
            "reading exposes no selected events".to_owned(),
            InvariantStatus::NotApplicable,
            evidence_ids,
            waived,
        );
    }
    let mut last_seen = BTreeMap::<RowInstanceId, usize>::new();
    let mut regressions = Vec::new();
    for event_id in ordered_ids {
        let event = plan.event(&event_id).expect("ordered event");
        for ordinal in event
            .ordinals
            .iter()
            .filter(|_| matches!(event.role, SerialRole::Structural))
        {
            let previous = last_seen.insert(ordinal.row_id.clone(), ordinal.ordinal);
            if let Some(previous) = previous
                && ordinal.ordinal < previous
            {
                regressions.push(ordinal.clone());
            }
        }
    }
    if !regressions.is_empty() {
        evidence_ids.push(evidence("evidence/order/regression"));
    }
    let (status, declared_waiver) = invariant_outcome(!regressions.is_empty(), waived);
    InvariantLedgerEntry::new(
        spec.id.clone(),
        spec.expected_fact.clone(),
        if regressions.is_empty() {
            "first structural appearances remained in order".to_owned()
        } else {
            format!("regressions [{}]", describe_ordinals(&regressions))
        },
        status,
        evidence_ids,
        declared_waiver,
    )
}

fn evaluate_repeats(
    plan: &SerialPlan,
    reading: SerialReading,
    spec: &PracticeRuleSpec,
    waived: Option<WaiverId>,
) -> InvariantLedgerEntry<PracticeRuleId> {
    let mut counts = BTreeMap::<OrdinalRef, usize>::new();
    for event in selected_events(plan, reading) {
        for ordinal in &event.ordinals {
            *counts.entry(ordinal.clone()).or_default() += 1;
        }
    }
    let repeated = counts
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(ordinal, _)| ordinal.clone())
        .collect::<Vec<_>>();
    let (status, declared_waiver) = invariant_outcome(!repeated.is_empty(), waived);
    InvariantLedgerEntry::new(
        spec.id.clone(),
        spec.expected_fact.clone(),
        if repeated.is_empty() {
            "no repeated ordinals".to_owned()
        } else {
            format!("repeated [{}]", describe_ordinals(&repeated))
        },
        status,
        vec![evidence("evidence/repeats/ordinal-counts")],
        declared_waiver,
    )
}

fn pitch_for(plan: &SerialPlan, ordinal: &OrdinalRef) -> Option<PitchClass> {
    plan.row(&ordinal.row_id)
        .and_then(|row| row.classes().get(ordinal.ordinal).copied())
}

fn evaluate_doublings(
    plan: &SerialPlan,
    reading: SerialReading,
    spec: &PracticeRuleSpec,
    waived: Option<WaiverId>,
) -> InvariantLedgerEntry<PracticeRuleId> {
    let mut doubled_groups = Vec::new();
    let mut groups = BTreeMap::<SimultaneousGroupId, Vec<&PlannedSerialEvent>>::new();
    for event in selected_events(plan, reading) {
        if let Some(group) = event.placement.simultaneous_group() {
            groups.entry(group.clone()).or_default().push(event);
        }
    }
    for (group_id, members) in groups {
        let mut pitches = BTreeSet::new();
        let mut doubled = false;
        for event in members {
            for ordinal in &event.ordinals {
                if let Some(pitch) = pitch_for(plan, ordinal)
                    && !pitches.insert(pitch)
                {
                    doubled = true;
                }
            }
        }
        if doubled {
            doubled_groups.push(group_id);
        }
    }
    let (status, declared_waiver) = invariant_outcome(!doubled_groups.is_empty(), waived);
    InvariantLedgerEntry::new(
        spec.id.clone(),
        spec.expected_fact.clone(),
        if doubled_groups.is_empty() {
            "no simultaneous doublings".to_owned()
        } else {
            format!(
                "doubled groups [{}]",
                doubled_groups
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        },
        status,
        vec![evidence("evidence/doublings/simultaneous-pitch-classes")],
        declared_waiver,
    )
}

fn evaluate_simultaneity(
    plan: &SerialPlan,
    reading: SerialReading,
    spec: &PracticeRuleSpec,
    waived: Option<WaiverId>,
    allow: bool,
) -> InvariantLedgerEntry<PracticeRuleId> {
    let groups = selected_events(plan, reading)
        .into_iter()
        .filter_map(|event| event.placement.simultaneous_group().cloned())
        .collect::<BTreeSet<_>>();
    let violated = !allow && !groups.is_empty();
    let (status, declared_waiver) = invariant_outcome(violated, waived);
    InvariantLedgerEntry::new(
        spec.id.clone(),
        spec.expected_fact.clone(),
        format!("simultaneous groups [{}]", groups.len()),
        status,
        vec![evidence("evidence/simultaneity/group-count")],
        declared_waiver,
    )
}

fn evaluate_row_mixing(
    plan: &SerialPlan,
    reading: SerialReading,
    spec: &PracticeRuleSpec,
    waived: Option<WaiverId>,
) -> InvariantLedgerEntry<PracticeRuleId> {
    let mixed_events = selected_events(plan, reading)
        .into_iter()
        .filter(|event| {
            event
                .ordinals
                .iter()
                .map(|ordinal| &ordinal.row_id)
                .collect::<BTreeSet<_>>()
                .len()
                > 1
        })
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    let (status, declared_waiver) = invariant_outcome(!mixed_events.is_empty(), waived);
    InvariantLedgerEntry::new(
        spec.id.clone(),
        spec.expected_fact.clone(),
        if mixed_events.is_empty() {
            "no event mixes row instances".to_owned()
        } else {
            format!(
                "mixed events [{}]",
                mixed_events
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        },
        status,
        vec![evidence("evidence/row-mixing/event-row-sets")],
        declared_waiver,
    )
}

fn evaluate_foreign_material(
    plan: &SerialPlan,
    reading: SerialReading,
    spec: &PracticeRuleSpec,
    waived: Option<WaiverId>,
    allow_external: bool,
) -> InvariantLedgerEntry<PracticeRuleId> {
    let external = selected_events(plan, reading)
        .into_iter()
        .filter(|event| matches!(event.role, SerialRole::External))
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    let violated = !allow_external && !external.is_empty();
    let (status, declared_waiver) = invariant_outcome(violated, waived);
    InvariantLedgerEntry::new(
        spec.id.clone(),
        spec.expected_fact.clone(),
        if external.is_empty() {
            "no external material".to_owned()
        } else {
            format!(
                "external events [{}]",
                external
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        },
        status,
        vec![evidence("evidence/foreign-material/external-events")],
        declared_waiver,
    )
}

fn evaluate_parameter_exhaustion(
    plan: &SerialPlan,
    reading: SerialReading,
    spec: &PracticeRuleSpec,
    waived: Option<WaiverId>,
) -> InvariantLedgerEntry<PracticeRuleId> {
    let exhausted = selected_events(plan, reading)
        .into_iter()
        .filter(|event| !matches!(event.role, SerialRole::Structural))
        .filter(|event| !event.ordinals.is_empty())
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    let violated = !exhausted.is_empty();
    let (status, declared_waiver) = invariant_outcome(violated, waived);
    InvariantLedgerEntry::new(
        spec.id.clone(),
        spec.expected_fact.clone(),
        if exhausted.is_empty() {
            "no non-structural event reuses exhausted parameters".to_owned()
        } else {
            format!(
                "non-structural reuse [{}]",
                exhausted
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        },
        status,
        vec![evidence(
            "evidence/parameter-exhaustion/non-structural-reuse",
        )],
        declared_waiver,
    )
}
