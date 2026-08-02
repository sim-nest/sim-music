//! Serial completion adapter over typed additive score edits.

#[path = "completion_search.rs"]
mod completion_search;
#[path = "completion_types.rs"]
mod completion_types;

use std::collections::BTreeSet;

use sim_lib_discrete_search::{SearchControl, SearchInterrupt, solve};
use sim_lib_music_core::{ObjectId, Pitch, Staff, StaffNote, Time};

use crate::additive::remove_additive_staff_patch;
use crate::allowance::{SerialAllowanceKind, SerialAllowanceMatch};
use crate::candidate_filter::{SerialCandidateContext, classify_note};
use crate::practice::PracticeRuleKind;
use crate::{
    DeclaredWaivers, EventPlacement, SerialEventId, SerialOrigin, SerialPlan, SerialPractice,
    SerialReading, SerialRealization, SerialRole, SimultaneousGroupId, WaiverId,
    render_serial_staff,
};
use completion_search::{GenericCompletionProblem, compile_patch};
pub use completion_types::*;

/// Completes one realized serial staff while preserving the structural plan and generic receipt.
pub fn complete_serial(
    realization: &SerialRealization,
    practice: &SerialPractice,
    waivers: &DeclaredWaivers,
    request: &SerialCompletionRequest,
    control: SearchControl,
    interrupt: &dyn SearchInterrupt,
) -> Result<SerialCompletionResult, SerialCompletionError> {
    let source = render_serial_staff(realization)?;
    let context = SerialCandidateContext {
        realization,
        allowances: &request.allowances,
    };
    let (filtered_indexes, filtered_candidates) =
        filter_candidates(&context, practice, waivers, &request.completion.candidates);
    if filtered_candidates.is_empty() && !request.completion.candidates.is_empty() {
        let reasons = request
            .completion
            .candidates
            .iter()
            .map(|candidate| candidate_reasons(&context, practice, waivers, candidate))
            .filter(|reason| !reason.is_empty())
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(SerialCompletionError::NoLegalCandidates(reasons));
    }
    let filtered_request = CompletionRequest {
        candidates: filtered_candidates,
        min_candidates: request.completion.min_candidates,
        max_candidates: request.completion.max_candidates,
        pitch_ranges: request.completion.pitch_ranges.clone(),
    };
    let structural_before = practice
        .evaluate(realization.plan(), SerialReading::StructuralPlan, waivers)
        .ledger
        .clone();
    let mut generic = complete_staff(&source, &filtered_request, control, interrupt)?;
    let selected_original_indexes = generic
        .provenance
        .selected_candidates
        .iter()
        .map(|index| filtered_indexes[*index])
        .collect::<Vec<_>>();
    generic.provenance.selected_candidates = selected_original_indexes.clone();
    let accepted_additions = selected_original_indexes
        .iter()
        .map(|index| {
            accepted_addition(
                &context,
                practice,
                waivers,
                *index,
                &request.completion.candidates[*index],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let augmented = augment_plan(
        realization,
        &request.completion.candidates,
        &accepted_additions,
    )?;
    let structural_after = practice
        .evaluate(&augmented, SerialReading::StructuralPlan, waivers)
        .ledger
        .clone();
    let sounding_after = practice
        .evaluate(&augmented, SerialReading::AllSounding, waivers)
        .ledger
        .clone();
    Ok(SerialCompletionResult {
        structural_plan: realization.plan().clone(),
        generic,
        accepted_additions,
        structural_before,
        structural_after,
        sounding_after,
    })
}

fn filter_candidates(
    context: &SerialCandidateContext<'_>,
    practice: &SerialPractice,
    waivers: &DeclaredWaivers,
    candidates: &[CompletionCandidate],
) -> (Vec<usize>, Vec<CompletionCandidate>) {
    let mut kept_indexes = Vec::new();
    let mut kept_candidates = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate_reasons(context, practice, waivers, candidate).is_empty() {
            kept_indexes.push(index);
            kept_candidates.push(candidate.clone());
        }
    }
    (kept_indexes, kept_candidates)
}

fn candidate_reasons(
    context: &SerialCandidateContext<'_>,
    practice: &SerialPractice,
    waivers: &DeclaredWaivers,
    candidate: &CompletionCandidate,
) -> String {
    let mut reasons = Vec::new();
    for note in candidate.notes() {
        let classification = classify_note(context, note);
        let Some(selected) = classification.selected else {
            reasons.push(format!(
                "note {} at {}/{} pitch {} has no admitted serial allowance",
                note.event_id,
                note.onset.numer(),
                note.onset.denom(),
                pitch_label(note.note.pitch)
            ));
            continue;
        };
        if matches!(selected.kind, SerialAllowanceKind::ExplicitForeignMaterial)
            && foreign_waiver(practice, waivers).is_none()
        {
            reasons.push(format!(
                "note {} reuses foreign material without a declared foreign-material waiver",
                note.event_id
            ));
        }
    }
    if let Some(reason) = addition_specific_reason(context.realization, candidate) {
        reasons.push(reason);
    }
    reasons.join("; ")
}

fn addition_specific_reason(
    realization: &SerialRealization,
    candidate: &CompletionCandidate,
) -> Option<String> {
    match candidate {
        CompletionCandidate::Ornament(OrnamentAddition {
            anchor_event_id,
            notes,
        }) => {
            let anchor = serial_event_id(anchor_event_id).ok()?;
            if realization.plan().event(&anchor).is_none() {
                return Some(format!(
                    "ornament anchor {} is not a serial event in the realized plan",
                    anchor_event_id
                ));
            }
            let end = notes.iter().map(StaffNote::end).max()?;
            if first_future_event(realization, end).is_none() {
                return Some(format!(
                    "ornament anchor {} has no explicit resolution event after the ornament span",
                    anchor_event_id
                ));
            }
            None
        }
        CompletionCandidate::Doubling(DoublingAddition {
            source_event_id,
            note,
        }) => {
            let source = serial_event_id(source_event_id).ok()?;
            let Some(event) = realization.plan().event(&source) else {
                return Some(format!(
                    "doubling source {} is not a serial event in the realized plan",
                    source_event_id
                ));
            };
            let Some(realized) = realization
                .events()
                .iter()
                .find(|event| event.event_id == source)
            else {
                return Some(format!(
                    "doubling source {} is not present in the realized event set",
                    source_event_id
                ));
            };
            let source_pitch_classes = realization
                .notes()
                .iter()
                .filter(|realized_note| realized_note.event_id == source)
                .map(|realized_note| realized_note.note.pitch.class)
                .collect::<Vec<_>>();
            if note.onset != realized.onset
                || note.note.duration != realized.duration
                || !source_pitch_classes.contains(&note.note.pitch.class)
            {
                return Some(format!(
                    "doubling source {} must retain realized onset, duration, and pitch class",
                    source_event_id
                ));
            }
            if !event.ordinals.iter().any(|ordinal| {
                realization
                    .plan()
                    .row(&ordinal.row_id)
                    .and_then(|row| row.classes().get(ordinal.ordinal).copied())
                    == Some(note.note.pitch.class)
            }) {
                return Some(format!(
                    "doubling source {} does not license pitch class {}",
                    source_event_id,
                    note.note.pitch.class.value()
                ));
            }
            None
        }
        _ => None,
    }
}

fn accepted_addition(
    context: &SerialCandidateContext<'_>,
    practice: &SerialPractice,
    waivers: &DeclaredWaivers,
    candidate_index: usize,
    candidate: &CompletionCandidate,
) -> Result<AcceptedSerialAddition, SerialCompletionError> {
    let notes = candidate
        .notes()
        .map(|note| {
            let classification = classify_note(context, note);
            let selected = classification.selected.ok_or_else(|| {
                SerialCompletionError::NoLegalCandidates(format!(
                    "selected generic completion note {} lost its serial allowance",
                    note.event_id
                ))
            })?;
            let category = accepted_category(practice, waivers, &selected.kind)?;
            Ok::<AcceptedSerialNote, SerialCompletionError>(AcceptedSerialNote {
                event_id: note.event_id.clone(),
                category,
                allowance: selected,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AcceptedSerialAddition {
        candidate_index,
        kind: candidate.kind(),
        notes,
    })
}

fn accepted_category(
    practice: &SerialPractice,
    waivers: &DeclaredWaivers,
    kind: &SerialAllowanceKind,
) -> Result<AcceptedSerialCategory, SerialCompletionError> {
    match kind {
        SerialAllowanceKind::CurrentPartition
        | SerialAllowanceKind::StatedPitchClasses
        | SerialAllowanceKind::AggregateRemainder => Ok(AcceptedSerialCategory::RowNative),
        SerialAllowanceKind::DerivedReservoir => Ok(AcceptedSerialCategory::RowDerived),
        SerialAllowanceKind::ModalProjection => Ok(AcceptedSerialCategory::ModalProjected),
        SerialAllowanceKind::ReferentialSubset { id } => {
            Ok(AcceptedSerialCategory::Referential { id: id.clone() })
        }
        SerialAllowanceKind::ExplicitForeignMaterial => foreign_waiver(practice, waivers)
            .map(|waiver| AcceptedSerialCategory::ForeignWithWaiver { waiver })
            .ok_or_else(|| {
                SerialCompletionError::NoLegalCandidates(
                    "explicit foreign material requires a declared foreign-material waiver"
                        .to_owned(),
                )
            }),
    }
}

fn foreign_waiver(practice: &SerialPractice, waivers: &DeclaredWaivers) -> Option<WaiverId> {
    practice
        .rule_specs()
        .into_iter()
        .find(|rule| rule.kind == PracticeRuleKind::ForeignMaterial)
        .and_then(|rule| waivers.waiver_for(&rule.id))
}

fn augment_plan(
    realization: &SerialRealization,
    candidates: &[CompletionCandidate],
    accepted_additions: &[AcceptedSerialAddition],
) -> Result<SerialPlan, SerialCompletionError> {
    let mut events = realization.plan().events().clone();
    let mut edges = realization
        .plan()
        .precedence()
        .edges()
        .map(|(before, after)| (before.clone(), after.clone()))
        .collect::<Vec<_>>();
    for accepted in accepted_additions {
        let candidate = &candidates[accepted.candidate_index];
        let simultaneous = simultaneous_group_for(accepted)?;
        for (note_index, note) in candidate.notes().enumerate() {
            let accepted_note = &accepted.notes[note_index];
            let event_id = SerialEventId::new(format!(
                "completion/addition/{}/note/{note_index}",
                accepted.candidate_index
            ))
            .map_err(|error| SerialCompletionError::Identity(error.to_string()))?;
            let parents = parent_ids(realization, candidate, note)?;
            let placement = match simultaneous.clone() {
                Some(group) => EventPlacement::simultaneous(group),
                None => EventPlacement::independent(),
            };
            let planned = crate::PlannedSerialEvent {
                id: event_id.clone(),
                ordinals: ordinals_for_match(realization, note, &accepted_note.allowance),
                role: role_for(&accepted_note.allowance.kind),
                origin: origin_for(&accepted_note.allowance.kind),
                voice: note.voice_id.clone(),
                placement,
                parents: parents.clone(),
                licenses: licenses_for(realization, &parents),
            };
            if let Some(previous) = parents.first() {
                edges.push((previous.clone(), event_id.clone()));
            }
            if let Some(next) = next_event_after(realization, candidate, note)?
                && !parents.contains(&next)
            {
                edges.push((event_id.clone(), next));
            }
            events.insert(event_id, planned);
        }
    }
    SerialPlan::try_new(realization.plan().rows().clone(), events, edges)
        .map_err(SerialCompletionError::from)
}

fn simultaneous_group_for(
    accepted: &AcceptedSerialAddition,
) -> Result<Option<SimultaneousGroupId>, SerialCompletionError> {
    if accepted.kind == AdditionKind::Chord {
        Ok(Some(
            SimultaneousGroupId::new(format!("completion/simul/{}", accepted.candidate_index))
                .map_err(|error| SerialCompletionError::Identity(error.to_string()))?,
        ))
    } else {
        Ok(None)
    }
}

fn parent_ids(
    realization: &SerialRealization,
    candidate: &CompletionCandidate,
    note: &StaffNote,
) -> Result<Vec<SerialEventId>, SerialCompletionError> {
    match candidate {
        CompletionCandidate::Ornament(ornament) => ornament_parents(realization, ornament),
        CompletionCandidate::Doubling(doubling) => {
            Ok(vec![serial_event_id(&doubling.source_event_id).map_err(
                |error| SerialCompletionError::Identity(error.to_string()),
            )?])
        }
        _ => Ok(default_parent_ids(realization, note)),
    }
}

fn ornament_parents(
    realization: &SerialRealization,
    ornament: &OrnamentAddition,
) -> Result<Vec<SerialEventId>, SerialCompletionError> {
    let anchor = serial_event_id(&ornament.anchor_event_id)
        .map_err(|error| SerialCompletionError::Identity(error.to_string()))?;
    let mut parents = vec![anchor];
    let end = ornament
        .notes
        .iter()
        .map(StaffNote::end)
        .max()
        .ok_or_else(|| {
            SerialCompletionError::NoLegalCandidates(
                "ornament additions must contain at least one note".to_owned(),
            )
        })?;
    let resolution = first_future_event(realization, end).ok_or_else(|| {
        SerialCompletionError::NoLegalCandidates(
            "ornament additions require explicit resolution evidence".to_owned(),
        )
    })?;
    if !parents.contains(&resolution) {
        parents.push(resolution);
    }
    Ok(parents)
}

fn next_event_after(
    realization: &SerialRealization,
    candidate: &CompletionCandidate,
    note: &StaffNote,
) -> Result<Option<SerialEventId>, SerialCompletionError> {
    match candidate {
        CompletionCandidate::Ornament(ornament) => {
            let end = ornament
                .notes
                .iter()
                .map(StaffNote::end)
                .max()
                .ok_or_else(|| {
                    SerialCompletionError::NoLegalCandidates(
                        "ornament additions must contain at least one note".to_owned(),
                    )
                })?;
            Ok(first_future_event(realization, end))
        }
        _ => Ok(first_future_event(
            realization,
            note.onset + note.note.duration,
        )),
    }
}

fn default_parent_ids(realization: &SerialRealization, note: &StaffNote) -> Vec<SerialEventId> {
    let mut overlapping = realization
        .events()
        .iter()
        .filter(|event| {
            let planned = realization
                .plan()
                .event(&event.event_id)
                .expect("realization event references a known plan event");
            event.onset <= note.onset
                && note.onset < event.onset + event.duration
                && planned.role == SerialRole::Structural
        })
        .map(|event| event.event_id.clone())
        .collect::<Vec<_>>();
    if overlapping.is_empty() {
        if let Some(previous) = last_prior_event(realization, note.onset) {
            overlapping.push(previous);
        }
        if let Some(next) = first_future_event(realization, note.onset)
            && !overlapping.contains(&next)
        {
            overlapping.push(next);
        }
    }
    if overlapping.is_empty() {
        realization
            .plan()
            .events()
            .keys()
            .next()
            .cloned()
            .into_iter()
            .collect()
    } else {
        overlapping
    }
}

fn last_prior_event(realization: &SerialRealization, onset: Time) -> Option<SerialEventId> {
    realization
        .events()
        .iter()
        .filter(|event| event.onset < onset)
        .map(|event| event.event_id.clone())
        .next_back()
}

fn first_future_event(realization: &SerialRealization, onset: Time) -> Option<SerialEventId> {
    realization
        .events()
        .iter()
        .find(|event| event.onset >= onset)
        .map(|event| event.event_id.clone())
}

fn licenses_for(
    realization: &SerialRealization,
    parents: &[SerialEventId],
) -> Vec<crate::StructuralLicense> {
    let mut licenses = Vec::new();
    for parent in parents {
        if let Some(event) = realization.plan().event(parent) {
            for license in &event.licenses {
                if !licenses.contains(license) {
                    licenses.push(license.clone());
                }
            }
        }
    }
    licenses
}

fn ordinals_for_match(
    realization: &SerialRealization,
    note: &StaffNote,
    matched: &SerialAllowanceMatch,
) -> Vec<crate::OrdinalRef> {
    let ordinals = if !matched.ordinals.is_empty() {
        matched.ordinals.clone()
    } else {
        pitch_class_ordinals(realization, note.note.pitch.class)
    };
    ordinals
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn pitch_class_ordinals(
    realization: &SerialRealization,
    pitch_class: sim_lib_pitch_core::PitchClass,
) -> Vec<crate::OrdinalRef> {
    realization
        .plan()
        .rows()
        .iter()
        .flat_map(|(row_id, row)| {
            row.classes()
                .iter()
                .enumerate()
                .filter(|(_, class)| **class == pitch_class)
                .map(|(ordinal, _)| crate::OrdinalRef::new(row_id.clone(), ordinal))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn role_for(kind: &SerialAllowanceKind) -> SerialRole {
    match kind {
        SerialAllowanceKind::ExplicitForeignMaterial => SerialRole::External,
        SerialAllowanceKind::DerivedReservoir => SerialRole::Derived,
        SerialAllowanceKind::CurrentPartition
        | SerialAllowanceKind::StatedPitchClasses
        | SerialAllowanceKind::AggregateRemainder
        | SerialAllowanceKind::ReferentialSubset { .. }
        | SerialAllowanceKind::ModalProjection => SerialRole::Ornamental,
    }
}

fn origin_for(kind: &SerialAllowanceKind) -> SerialOrigin {
    match kind {
        SerialAllowanceKind::ExplicitForeignMaterial => SerialOrigin::External {
            source: kind.label(),
        },
        SerialAllowanceKind::DerivedReservoir => SerialOrigin::Derived {
            technique: kind.label(),
        },
        _ => SerialOrigin::Ornamental {
            technique: kind.label(),
        },
    }
}

pub(crate) fn complete_staff(
    source: &Staff,
    request: &CompletionRequest,
    control: SearchControl,
    interrupt: &dyn SearchInterrupt,
) -> Result<CompletionResult, CompletionError> {
    for candidate in &request.candidates {
        compile_patch(
            source,
            std::slice::from_ref(candidate),
            &request.pitch_ranges,
        )?;
    }
    let run = solve(
        &GenericCompletionProblem { source, request },
        control,
        interrupt,
    );
    let receipt = run.receipt;
    let Some(output) = run.outputs.into_iter().min_by_key(|output| {
        (
            output.selected.len(),
            output.selected.clone(),
            output.added_ids.clone(),
        )
    }) else {
        return Err(CompletionError::NoCompletion {
            before: Box::new(source.clone()),
            search: Box::new(receipt),
        });
    };
    let restored = remove_additive_staff_patch(&output.completed, &output.patch)
        .map_err(CompletionError::InvalidCandidate)?;
    if restored != *source {
        return Err(CompletionError::InvalidCandidate(
            "reverse additive patch changed the source staff".to_owned(),
        ));
    }
    Ok(CompletionResult {
        patch: output.patch,
        before: source.clone(),
        after: output.completed,
        search: receipt,
        provenance: CompletionProvenance {
            selected_candidates: output.selected,
            preserved_ids: source.object_ids(),
            added_ids: output.added_ids,
            facts: vec![
                "source-material=immutable".to_owned(),
                "patch-operation=additions-only".to_owned(),
                "inverse=remove(apply(source,patch),patch)==source".to_owned(),
            ],
        },
    })
}

fn serial_event_id(value: &ObjectId) -> Result<SerialEventId, String> {
    SerialEventId::new(value.to_string()).map_err(|error| error.to_string())
}

fn pitch_label(pitch: Pitch) -> String {
    pitch.to_midi().map_or_else(
        || format!("class-{}", pitch.class.value()),
        |midi| format!("midi-{midi}"),
    )
}

fn invalid<T>(reason: impl Into<String>) -> Result<T, CompletionError> {
    Err(CompletionError::InvalidCandidate(reason.into()))
}
