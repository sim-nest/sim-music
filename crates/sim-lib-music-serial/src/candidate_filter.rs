//! Serial legality filtering over generic completion candidates.

use sim_lib_music_core::{StaffNote, Time};

use crate::allowance::{SerialAllowanceKind, SerialAllowanceMatch, SerialCompletionAllowances};
use crate::{OrdinalRef, PlannedSerialEvent, SerialRealization, SerialRole};

#[derive(Clone, Debug)]
pub(crate) struct SerialCandidateContext<'a> {
    pub realization: &'a SerialRealization,
    pub allowances: &'a SerialCompletionAllowances,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NoteAllowanceClassification {
    pub matches: Vec<SerialAllowanceMatch>,
    pub selected: Option<SerialAllowanceMatch>,
}

pub(crate) fn classify_note(
    context: &SerialCandidateContext<'_>,
    note: &StaffNote,
) -> NoteAllowanceClassification {
    let pitch_class = note.note.pitch.class;
    let mut matches = Vec::<SerialAllowanceMatch>::new();

    let current_partition = structural_ordinals_matching(
        context.realization,
        note.onset,
        |start, end| start <= note.onset && note.onset < end,
        pitch_class,
    );
    if !current_partition.is_empty() {
        matches.push(SerialAllowanceMatch {
            kind: SerialAllowanceKind::CurrentPartition,
            ordinals: current_partition,
        });
    }

    let stated = structural_ordinals_matching(
        context.realization,
        note.onset,
        |start, _| start < note.onset,
        pitch_class,
    );
    if !stated.is_empty() {
        matches.push(SerialAllowanceMatch {
            kind: SerialAllowanceKind::StatedPitchClasses,
            ordinals: stated,
        });
    }

    let remainder = structural_ordinals_matching(
        context.realization,
        note.onset,
        |start, _| start >= note.onset,
        pitch_class,
    );
    if !remainder.is_empty() {
        matches.push(SerialAllowanceMatch {
            kind: SerialAllowanceKind::AggregateRemainder,
            ordinals: remainder,
        });
    }

    for subset in &context.allowances.referential_subsets {
        if subset.pitch_classes.contains(&pitch_class) {
            matches.push(SerialAllowanceMatch {
                kind: SerialAllowanceKind::ReferentialSubset {
                    id: subset.id.clone(),
                },
                ordinals: Vec::new(),
            });
        }
    }

    if let Some(report) = context.realization.spine_report() {
        let ordinals = report
            .entries
            .iter()
            .filter(|entry| entry.landed_pitch.class == pitch_class)
            .map(|entry| entry.ordinal.clone())
            .collect::<Vec<_>>();
        if !ordinals.is_empty() {
            matches.push(SerialAllowanceMatch {
                kind: SerialAllowanceKind::ModalProjection,
                ordinals,
            });
        }
    }

    let derived = non_structural_ordinals_matching(context.realization, pitch_class, |event| {
        matches!(event.role, SerialRole::Derived | SerialRole::Ornamental)
    });
    if !derived.is_empty() {
        matches.push(SerialAllowanceMatch {
            kind: SerialAllowanceKind::DerivedReservoir,
            ordinals: derived,
        });
    }

    let foreign = non_structural_ordinals_matching(context.realization, pitch_class, |event| {
        event.role == SerialRole::External
    });
    if !foreign.is_empty() {
        matches.push(SerialAllowanceMatch {
            kind: SerialAllowanceKind::ExplicitForeignMaterial,
            ordinals: foreign,
        });
    }

    let selected = matches
        .iter()
        .find(|matched| allowance_enabled(context.allowances, &matched.kind))
        .cloned();
    NoteAllowanceClassification { matches, selected }
}

fn structural_ordinals_matching(
    realization: &SerialRealization,
    _onset: Time,
    include: impl Fn(Time, Time) -> bool,
    pitch_class: sim_lib_pitch_core::PitchClass,
) -> Vec<OrdinalRef> {
    realization
        .notes()
        .iter()
        .filter(|note| {
            let event = realization
                .plan()
                .event(&note.event_id)
                .expect("realization note references a known event");
            event.role == SerialRole::Structural
                && note.note.pitch.class == pitch_class
                && include(note.onset, note.note.duration + note.onset)
        })
        .map(|note| note.origin.source_ordinal.clone())
        .collect()
}

fn non_structural_ordinals_matching(
    realization: &SerialRealization,
    pitch_class: sim_lib_pitch_core::PitchClass,
    include: impl Fn(&PlannedSerialEvent) -> bool,
) -> Vec<OrdinalRef> {
    realization
        .plan()
        .events()
        .values()
        .filter(|event| include(event))
        .flat_map(|event| event.ordinals.iter())
        .filter(|ordinal| {
            realization
                .plan()
                .row(&ordinal.row_id)
                .and_then(|row| row.classes().get(ordinal.ordinal).copied())
                == Some(pitch_class)
        })
        .cloned()
        .collect()
}

fn allowance_enabled(
    allowances: &SerialCompletionAllowances,
    allowance: &SerialAllowanceKind,
) -> bool {
    match allowance {
        SerialAllowanceKind::CurrentPartition => allowances.current_partition,
        SerialAllowanceKind::StatedPitchClasses => allowances.stated_pitch_classes,
        SerialAllowanceKind::AggregateRemainder => allowances.aggregate_remainder,
        SerialAllowanceKind::ReferentialSubset { id } => allowances
            .referential_subsets
            .iter()
            .any(|subset| subset.id == *id),
        SerialAllowanceKind::ModalProjection => allowances.modal_projection,
        SerialAllowanceKind::DerivedReservoir => allowances.derived_reservoir,
        SerialAllowanceKind::ExplicitForeignMaterial => allowances.explicitly_foreign_material,
    }
}
