//! Immutable serial-plan construction and validation.

use std::collections::{BTreeMap, BTreeSet};

use sim_lib_pitch_serial::RowForm;

use crate::{
    OrdinalRef, PlannedSerialEvent, PrecedenceGraph, RowInstanceId, SerialEventId, SerialOrigin,
    SerialPlanError, SerialRole, SimultaneousGroupId,
};

/// Immutable structural source for serial practice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialPlan {
    rows: BTreeMap<RowInstanceId, RowForm>,
    events: BTreeMap<SerialEventId, PlannedSerialEvent>,
    precedence: PrecedenceGraph<SerialEventId>,
}

impl SerialPlan {
    /// Validates and freezes one complete serial plan.
    pub fn try_new(
        rows: BTreeMap<RowInstanceId, RowForm>,
        events: BTreeMap<SerialEventId, PlannedSerialEvent>,
        precedence_edges: impl IntoIterator<Item = (SerialEventId, SerialEventId)>,
    ) -> Result<Self, SerialPlanError> {
        let event_ids = events.keys().cloned().collect::<BTreeSet<_>>();
        let precedence = PrecedenceGraph::try_new(precedence_edges, &event_ids)?;
        validate_events(&rows, &events)?;
        validate_parent_graph(&events)?;
        validate_simultaneous_constraints(&events, &precedence)?;
        validate_structural_coverage(&rows, &events)?;
        Ok(Self {
            rows,
            events,
            precedence,
        })
    }

    /// Returns the immutable row instances by stable id.
    pub fn rows(&self) -> &BTreeMap<RowInstanceId, RowForm> {
        &self.rows
    }

    /// Returns the immutable planned events by stable id.
    pub fn events(&self) -> &BTreeMap<SerialEventId, PlannedSerialEvent> {
        &self.events
    }

    /// Returns the validated precedence DAG.
    pub fn precedence(&self) -> &PrecedenceGraph<SerialEventId> {
        &self.precedence
    }

    /// Returns every simultaneous placement group and its canonical event members.
    pub fn simultaneous_groups(&self) -> BTreeMap<SimultaneousGroupId, Vec<&PlannedSerialEvent>> {
        let mut groups: BTreeMap<SimultaneousGroupId, Vec<&PlannedSerialEvent>> = BTreeMap::new();
        for event in self.events.values() {
            if let Some(group) = event.placement.simultaneous_group() {
                groups.entry(group.clone()).or_default().push(event);
            }
        }
        groups
    }
}

fn validate_events(
    rows: &BTreeMap<RowInstanceId, RowForm>,
    events: &BTreeMap<SerialEventId, PlannedSerialEvent>,
) -> Result<(), SerialPlanError> {
    for (event_id, event) in events {
        if &event.id != event_id {
            return Err(SerialPlanError::InvalidId {
                kind: "serial-event-key",
                value: event.id.as_str().to_owned(),
                reason: "map key and event id must match",
            });
        }
        if event.ordinals.is_empty() {
            return Err(SerialPlanError::EmptyOrdinalSet(event.id.clone()));
        }
        let mut seen_ordinals = BTreeSet::new();
        for ordinal in &event.ordinals {
            if !seen_ordinals.insert(ordinal.clone()) {
                return Err(SerialPlanError::DuplicateOrdinal {
                    event_id: event.id.clone(),
                    ordinal: ordinal.clone(),
                });
            }
            let Some(row) = rows.get(&ordinal.row_id) else {
                return Err(SerialPlanError::UnknownRow {
                    event_id: event.id.clone(),
                    row_id: ordinal.row_id.clone(),
                });
            };
            let row_len = row.classes().len();
            if ordinal.ordinal >= row_len {
                return Err(SerialPlanError::OrdinalOutOfRange {
                    event_id: event.id.clone(),
                    row_id: ordinal.row_id.clone(),
                    ordinal: ordinal.ordinal,
                    row_len,
                });
            }
        }
        validate_role_origin(event_id, event)?;
    }
    Ok(())
}

fn validate_role_origin(
    event_id: &SerialEventId,
    event: &PlannedSerialEvent,
) -> Result<(), SerialPlanError> {
    let parents_empty = event.parents.is_empty();
    match (event.role, &event.origin, parents_empty) {
        (SerialRole::Structural, SerialOrigin::Structural { rationale }, true)
            if !rationale.trim().is_empty() =>
        {
            Ok(())
        }
        (SerialRole::Structural, SerialOrigin::Structural { .. }, false) => {
            Err(SerialPlanError::RoleOriginMismatch {
                event_id: event_id.clone(),
                reason: "structural events cannot name parents",
            })
        }
        (SerialRole::Structural, _, _) => Err(SerialPlanError::RoleOriginMismatch {
            event_id: event_id.clone(),
            reason: "structural role requires structural origin",
        }),
        (SerialRole::Derived, SerialOrigin::Derived { technique }, false)
            if !technique.trim().is_empty() =>
        {
            Ok(())
        }
        (SerialRole::Derived, SerialOrigin::Derived { .. }, true) => {
            Err(SerialPlanError::MissingParents {
                event_id: event_id.clone(),
                role: SerialRole::Derived.as_str(),
            })
        }
        (SerialRole::Derived, _, _) => Err(SerialPlanError::RoleOriginMismatch {
            event_id: event_id.clone(),
            reason: "derived role requires derived origin",
        }),
        (SerialRole::Ornamental, SerialOrigin::Ornamental { technique }, false)
            if !technique.trim().is_empty() =>
        {
            Ok(())
        }
        (SerialRole::Ornamental, SerialOrigin::Ornamental { .. }, true) => {
            Err(SerialPlanError::MissingParents {
                event_id: event_id.clone(),
                role: SerialRole::Ornamental.as_str(),
            })
        }
        (SerialRole::Ornamental, _, _) => Err(SerialPlanError::RoleOriginMismatch {
            event_id: event_id.clone(),
            reason: "ornamental role requires ornamental origin",
        }),
        (SerialRole::External, SerialOrigin::External { source }, false)
            if !source.trim().is_empty() =>
        {
            Ok(())
        }
        (SerialRole::External, SerialOrigin::External { .. }, true) => {
            Err(SerialPlanError::MissingParents {
                event_id: event_id.clone(),
                role: SerialRole::External.as_str(),
            })
        }
        (SerialRole::External, _, _) => Err(SerialPlanError::RoleOriginMismatch {
            event_id: event_id.clone(),
            reason: "external role requires external origin",
        }),
    }
}

fn validate_parent_graph(
    events: &BTreeMap<SerialEventId, PlannedSerialEvent>,
) -> Result<(), SerialPlanError> {
    #[derive(Copy, Clone, PartialEq, Eq)]
    enum Mark {
        Visiting,
        Done,
    }

    fn visit(
        id: &SerialEventId,
        events: &BTreeMap<SerialEventId, PlannedSerialEvent>,
        marks: &mut BTreeMap<SerialEventId, Mark>,
    ) -> Result<(), SerialPlanError> {
        match marks.get(id) {
            Some(Mark::Done) => return Ok(()),
            Some(Mark::Visiting) => return Err(SerialPlanError::ParentCycle(id.clone())),
            None => {}
        }
        marks.insert(id.clone(), Mark::Visiting);
        let event = events.get(id).expect("known event");
        for parent in &event.parents {
            if parent == id {
                return Err(SerialPlanError::SelfParent(id.clone()));
            }
            if !events.contains_key(parent) {
                return Err(SerialPlanError::UnknownParent {
                    event_id: id.clone(),
                    parent_id: parent.clone(),
                });
            }
            visit(parent, events, marks)?;
        }
        marks.insert(id.clone(), Mark::Done);
        Ok(())
    }

    let mut marks = BTreeMap::new();
    for event_id in events.keys() {
        visit(event_id, events, &mut marks)?;
    }
    Ok(())
}

fn validate_simultaneous_constraints(
    events: &BTreeMap<SerialEventId, PlannedSerialEvent>,
    precedence: &PrecedenceGraph<SerialEventId>,
) -> Result<(), SerialPlanError> {
    let mut by_group: BTreeMap<&SimultaneousGroupId, Vec<&PlannedSerialEvent>> = BTreeMap::new();
    for event in events.values() {
        if let Some(group) = event.placement.simultaneous_group() {
            by_group.entry(group).or_default().push(event);
        }
    }
    for (group_id, members) in by_group {
        for left in 0..members.len() {
            for right in (left + 1)..members.len() {
                let before = &members[left].id;
                let after = &members[right].id;
                if precedence.contains_edge(before, after)
                    || precedence.contains_edge(after, before)
                {
                    return Err(SerialPlanError::SimultaneousPrecedenceConflict {
                        group_id: group_id.clone(),
                        before: before.clone(),
                        after: after.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_structural_coverage(
    rows: &BTreeMap<RowInstanceId, RowForm>,
    events: &BTreeMap<SerialEventId, PlannedSerialEvent>,
) -> Result<(), SerialPlanError> {
    let mut covered: BTreeMap<RowInstanceId, BTreeSet<usize>> = BTreeMap::new();
    for event in events
        .values()
        .filter(|event| event.role == SerialRole::Structural)
    {
        for OrdinalRef { row_id, ordinal } in &event.ordinals {
            covered.entry(row_id.clone()).or_default().insert(*ordinal);
        }
    }
    for (row_id, row) in rows {
        let row_len = row.classes().len();
        let missing = (0..row_len)
            .filter(|ordinal| !covered.get(row_id).is_some_and(|set| set.contains(ordinal)))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(SerialPlanError::MissingStructuralCoverage {
                row_id: row_id.clone(),
                ordinals: missing,
            });
        }
    }
    Ok(())
}
