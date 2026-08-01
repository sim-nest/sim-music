//! Built-in strict chromatic realizer registered through the open registry.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use sim_lib_music_core::{Note, Pitch, Time};

use crate::{
    EvidenceId, InvariantLedger, InvariantLedgerEntry, InvariantStatus, RealizationContext,
    RealizedSerialEvent, RealizedSerialNote, RealizedSerialOrigin, RealizerId, SerialEventId,
    SerialPlan, SerialRealization, SerialRealizer, StrictEventSpec, StrictRealizationError,
    TiePolicy,
};

/// Stable id of the built-in strict chromatic realizer.
pub fn strict_chromatic_realizer_id() -> RealizerId {
    RealizerId::new("realizer/strict-chromatic").expect("built-in realizer id is valid")
}

/// Built-in strict chromatic serial realizer.
#[derive(Clone, Debug)]
pub struct ChromaticSerialRealizer {
    id: RealizerId,
}

impl Default for ChromaticSerialRealizer {
    fn default() -> Self {
        Self {
            id: strict_chromatic_realizer_id(),
        }
    }
}

impl SerialRealizer for ChromaticSerialRealizer {
    fn id(&self) -> &RealizerId {
        &self.id
    }

    fn realize(
        &self,
        plan: &SerialPlan,
        context: &RealizationContext,
    ) -> Result<SerialRealization, StrictRealizationError> {
        realize_chromatic(self.id(), plan, context)
    }
}

#[derive(Clone, Debug)]
struct RealizedEventState {
    event: RealizedSerialEvent,
    note_indexes: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct UnitKey {
    row_id: String,
    ordinal: usize,
    label: String,
}

#[derive(Clone, Debug)]
struct EventUnit {
    members: Vec<SerialEventId>,
    key: UnitKey,
}

fn realize_chromatic(
    realizer_id: &RealizerId,
    plan: &SerialPlan,
    context: &RealizationContext,
) -> Result<SerialRealization, StrictRealizationError> {
    for event_id in plan.events().keys() {
        let Some(spec) = context.specs.get(event_id) else {
            return Err(StrictRealizationError::MissingSpec(event_id.clone()));
        };
        if spec.duration <= Time::from_integer(0) {
            return Err(StrictRealizationError::NonPositiveDuration(
                event_id.clone(),
            ));
        }
    }

    let units = build_units(plan);
    let unit_index = units
        .iter()
        .enumerate()
        .flat_map(|(index, unit)| unit.members.iter().cloned().map(move |id| (id, index)))
        .collect::<BTreeMap<_, _>>();
    let order = topo_units(plan, &units, &unit_index);

    let mut notes = Vec::<RealizedSerialNote>::new();
    let mut events = BTreeMap::<SerialEventId, RealizedEventState>::new();
    let mut cursor = Time::from_integer(0);
    for unit_idx in order {
        let unit = &units[unit_idx];
        let unit_onset = cursor;
        let mut unit_duration = Time::from_integer(0);
        for event_id in &unit.members {
            let planned = plan.event(event_id).expect("unit event must exist");
            let spec = context
                .specs
                .get(event_id)
                .expect("validated event specs must exist");
            unit_duration = unit_duration.max(spec.duration);
            let mut note_indexes = Vec::new();
            if matches!(spec.sound, crate::EventSound::Notes) {
                let displacements = event_displacements(spec, planned.ordinals.len(), event_id)?;
                for (note_index, (ordinal, displacement)) in planned
                    .ordinals
                    .iter()
                    .cloned()
                    .zip(displacements)
                    .enumerate()
                {
                    let row_form = plan
                        .row(&ordinal.row_id)
                        .expect("validated ordinal row must exist");
                    let pitch_class = row_form.classes()[ordinal.ordinal];
                    let midi = 12
                        * (i16::from(spec.pitch_layout.register) + i16::from(displacement) + 1)
                        + i16::from(pitch_class.value());
                    if !(0..=127).contains(&midi) {
                        return Err(StrictRealizationError::MidiOutOfRange {
                            event_id: event_id.clone(),
                            midi,
                        });
                    }
                    let note = Note::new(
                        spec.duration,
                        Pitch::from_midi(midi as u8),
                        spec.velocity,
                        spec.channel,
                        spec.articulation,
                    )
                    .map_err(|error| StrictRealizationError::MusicCore(error.to_string()))?;
                    note_indexes.push(notes.len());
                    notes.push(RealizedSerialNote {
                        event_id: event_id.clone(),
                        voice: planned.voice.clone(),
                        note_index,
                        onset: unit_onset,
                        note,
                        origin: RealizedSerialOrigin {
                            realizer_id: realizer_id.clone(),
                            licenses: planned.licenses.clone(),
                            ordinals: planned.ordinals.clone(),
                            source_ordinal: ordinal.clone(),
                            row_forms: planned
                                .ordinals
                                .iter()
                                .map(|item| {
                                    (
                                        item.row_id.clone(),
                                        plan.row(&item.row_id)
                                            .expect("validated row must exist")
                                            .clone(),
                                    )
                                })
                                .collect(),
                        },
                    });
                }
            }
            events.insert(
                event_id.clone(),
                RealizedEventState {
                    event: RealizedSerialEvent {
                        event_id: event_id.clone(),
                        onset: unit_onset,
                        duration: spec.duration,
                        is_rest: matches!(spec.sound, crate::EventSound::Rest),
                        ties_into_next: matches!(spec.tie, TiePolicy::IntoNext),
                    },
                    note_indexes,
                },
            );
        }
        cursor += match context.simultaneous_policy {
            crate::SimultaneousRenderPolicy::PreserveOnset => unit_duration,
        };
    }

    apply_ties(plan, context, &mut events, &mut notes)?;

    let realized_events = events
        .into_values()
        .map(|state| state.event)
        .collect::<Vec<_>>();
    let evidence_ids = vec![
        EvidenceId::new("evidence/strict-specs").expect("evidence id"),
        EvidenceId::new("evidence/typed-origin").expect("evidence id"),
    ];
    let ledger = InvariantLedger::new(vec![InvariantLedgerEntry::new(
        realizer_id.clone(),
        "every realized note retains typed serial provenance and explicit strict event specs",
        format!(
            "realized {} events and {} sounding notes through {}",
            realized_events.len(),
            notes.len(),
            realizer_id
        ),
        InvariantStatus::Preserved,
        evidence_ids,
        None,
    )]);

    Ok(SerialRealization::new(
        plan.clone(),
        realized_events,
        notes,
        ledger,
    ))
}

fn event_displacements(
    spec: &StrictEventSpec,
    ordinals: usize,
    event_id: &SerialEventId,
) -> Result<Vec<i8>, StrictRealizationError> {
    match spec.pitch_layout.octave_displacements.len() {
        0 => Ok(vec![0; ordinals]),
        1 => Ok(vec![spec.pitch_layout.octave_displacements[0]; ordinals]),
        len if len == ordinals => Ok(spec.pitch_layout.octave_displacements.clone()),
        len => Err(StrictRealizationError::OctaveDisplacementMismatch {
            event_id: event_id.clone(),
            ordinals,
            displacements: len,
        }),
    }
}

fn build_units(plan: &SerialPlan) -> Vec<EventUnit> {
    let grouped = plan
        .simultaneous_groups()
        .into_iter()
        .map(|(group, events)| {
            let mut members = events
                .iter()
                .map(|event| event.id.clone())
                .collect::<Vec<_>>();
            members.sort();
            let label = format!("group/{group}");
            (group, event_unit(plan, label, members))
        })
        .collect::<BTreeMap<_, _>>();
    let mut units = grouped.into_values().collect::<Vec<_>>();
    let grouped_ids = units
        .iter()
        .flat_map(|unit| unit.members.iter().cloned())
        .collect::<BTreeSet<_>>();
    for event in plan.events().values() {
        if !grouped_ids.contains(&event.id) {
            units.push(event_unit(
                plan,
                event.id.as_str().to_owned(),
                vec![event.id.clone()],
            ));
        }
    }
    units.sort_by(|left, right| left.key.cmp(&right.key));
    units
}

fn event_unit(plan: &SerialPlan, label: String, members: Vec<SerialEventId>) -> EventUnit {
    let key = members
        .iter()
        .filter_map(|event_id| plan.event(event_id))
        .flat_map(|event| event.ordinals.iter())
        .min_by(|left, right| {
            left.row_id
                .cmp(&right.row_id)
                .then_with(|| left.ordinal.cmp(&right.ordinal))
        })
        .map(|ordinal| UnitKey {
            row_id: ordinal.row_id.as_str().to_owned(),
            ordinal: ordinal.ordinal,
            label: label.clone(),
        })
        .unwrap_or(UnitKey {
            row_id: String::new(),
            ordinal: 0,
            label: label.clone(),
        });
    EventUnit { members, key }
}

fn topo_units(
    plan: &SerialPlan,
    units: &[EventUnit],
    unit_index: &BTreeMap<SerialEventId, usize>,
) -> Vec<usize> {
    let mut indegree = vec![0usize; units.len()];
    let mut outgoing = vec![BTreeSet::<usize>::new(); units.len()];
    for (before, after) in plan.precedence().edges() {
        let before_idx = unit_index[before];
        let after_idx = unit_index[after];
        if before_idx != after_idx && outgoing[before_idx].insert(after_idx) {
            indegree[after_idx] += 1;
        }
    }
    let mut heap = BinaryHeap::<Reverse<(UnitKey, usize)>>::new();
    for (index, unit) in units.iter().enumerate() {
        if indegree[index] == 0 {
            heap.push(Reverse((unit.key.clone(), index)));
        }
    }
    let mut order = Vec::with_capacity(units.len());
    while let Some(Reverse((_, index))) = heap.pop() {
        order.push(index);
        for &target in &outgoing[index] {
            indegree[target] -= 1;
            if indegree[target] == 0 {
                heap.push(Reverse((units[target].key.clone(), target)));
            }
        }
    }
    order
}

fn apply_ties(
    plan: &SerialPlan,
    context: &RealizationContext,
    events: &mut BTreeMap<SerialEventId, RealizedEventState>,
    notes: &mut Vec<RealizedSerialNote>,
) -> Result<(), StrictRealizationError> {
    let mut by_voice = plan
        .events()
        .values()
        .map(|event| event.voice.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|voice| {
            let mut ids = events
                .values()
                .filter(|state| {
                    plan.event(&state.event.event_id)
                        .is_some_and(|event| event.voice == voice)
                })
                .map(|state| state.event.event_id.clone())
                .collect::<Vec<_>>();
            ids.sort_by(|left, right| {
                let left_state = &events[left];
                let right_state = &events[right];
                left_state
                    .event
                    .onset
                    .cmp(&right_state.event.onset)
                    .then_with(|| left.cmp(right))
            });
            (voice, ids)
        })
        .collect::<BTreeMap<_, _>>();

    for event_ids in by_voice.values_mut() {
        let mut index = 0usize;
        while index < event_ids.len() {
            let current_id = event_ids[index].clone();
            let spec = context
                .specs
                .get(&current_id)
                .expect("specs validated up front");
            if !matches!(spec.tie, TiePolicy::IntoNext) {
                index += 1;
                continue;
            }
            let Some(next_id) = event_ids.get(index + 1).cloned() else {
                return Err(StrictRealizationError::MissingTieTarget(current_id));
            };
            let current_indexes = events[&current_id].note_indexes.clone();
            let next_indexes = events[&next_id].note_indexes.clone();
            if current_indexes.len() != next_indexes.len() {
                return Err(StrictRealizationError::InvalidTieTarget {
                    source_event: current_id,
                    target_event: next_id,
                    reason: "pitch multiplicity differs",
                });
            }
            if current_indexes.is_empty() {
                return Err(StrictRealizationError::InvalidTieTarget {
                    source_event: current_id,
                    target_event: next_id,
                    reason: "rests cannot tie",
                });
            }
            let current_pitches = current_indexes
                .iter()
                .map(|&note_index| notes[note_index].note.pitch)
                .collect::<Vec<_>>();
            let next_pitches = next_indexes
                .iter()
                .map(|&note_index| notes[note_index].note.pitch)
                .collect::<Vec<_>>();
            if current_pitches != next_pitches {
                return Err(StrictRealizationError::InvalidTieTarget {
                    source_event: current_id,
                    target_event: next_id,
                    reason: "tied pitches differ",
                });
            }
            let extension = events[&next_id].event.duration;
            for &note_index in &current_indexes {
                let note = &mut notes[note_index];
                note.note.duration += extension;
            }
            for &note_index in next_indexes.iter().rev() {
                notes.remove(note_index);
                for state in events.values_mut() {
                    for index in &mut state.note_indexes {
                        if *index > note_index {
                            *index -= 1;
                        }
                    }
                }
            }
            events
                .get_mut(&next_id)
                .expect("event must exist")
                .note_indexes
                .clear();
            events
                .get_mut(&next_id)
                .expect("event must exist")
                .event
                .is_rest = true;
            index += 2;
        }
    }
    Ok(())
}
