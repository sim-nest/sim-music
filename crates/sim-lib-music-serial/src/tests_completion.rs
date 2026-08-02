use std::collections::BTreeMap;
use std::sync::Arc;

use sim_lib_discrete_search::{NeverInterrupt, SearchControl, SearchStatus};
use sim_lib_music_core::{Articulation, Channel, Note, ObjectId, Pitch, Staff, StaffNote, Time};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_dissonance::{PitchDissonanceRegistry, PitchDissonanceScore};
use sim_lib_pitch_namer::LabelContext;
use sim_lib_pitch_scale::{PlayerScale, Scale};
use sim_lib_pitch_set::PitchClassMask;

use crate::additive::remove_additive_staff_patch;
use crate::candidate_filter::{SerialCandidateContext, classify_note};
use crate::tests::{practice_plan, quarter, strict_context, strict_plan};
use crate::{
    AcceptedSerialCategory, BuiltInPracticeRule, ChordAddition, CompletionCandidate,
    CompletionRequest, DeclaredWaivers, DoublingAddition, NoteAddition, OrnamentAddition,
    PedalAddition, PitchRangeConstraint, PracticeId, PracticeRuleId, ReferentialSubsetAllowance,
    SerialAllowanceKind, SerialCompletionAllowances, SerialCompletionRequest, SerialPractice,
    StrictEventSpec, StrictRealizationContext, WaiverId, complete_serial,
    default_realizer_registry, realize_strict,
};

fn note(voice_id: &str, event: &str, pitch: u8, onset: Time, duration: Time) -> StaffNote {
    StaffNote {
        voice_id: ObjectId::new(voice_id).expect("voice id"),
        note_id: ObjectId::new(format!("note/{event}")).expect("note id"),
        event_id: ObjectId::new(format!("event/{event}")).expect("event id"),
        onset,
        note: Note::new(
            duration,
            Pitch::from_midi(pitch),
            96,
            Channel::new(0).expect("channel"),
            Articulation::Normal,
        )
        .expect("note"),
    }
}

fn practice() -> SerialPractice {
    SerialPractice::new(
        PracticeId::new("practice/serial-completion").expect("practice id"),
        vec![
            Arc::new(BuiltInPracticeRule::aggregate(
                PracticeRuleId::new("rule/aggregate").expect("rule id"),
            )),
            Arc::new(BuiltInPracticeRule::order(
                PracticeRuleId::new("rule/order").expect("rule id"),
            )),
            Arc::new(BuiltInPracticeRule::repeats(
                PracticeRuleId::new("rule/repeats").expect("rule id"),
            )),
            Arc::new(BuiltInPracticeRule::foreign_material(
                PracticeRuleId::new("rule/foreign").expect("rule id"),
                false,
            )),
        ],
    )
}

fn practice_context() -> StrictRealizationContext {
    let channel = Channel::new(0).expect("channel");
    let specs = [
        (
            "event/struct-a",
            StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Normal),
        ),
        (
            "event/struct-b",
            StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Normal),
        ),
        (
            "event/struct-c",
            StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Normal),
        ),
        (
            "event/struct-d",
            StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Normal),
        ),
        (
            "event/derived-repeat",
            StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Normal),
        ),
        (
            "event/external-citation",
            StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Normal),
        ),
    ]
    .into_iter()
    .map(|(id, spec)| (crate::SerialEventId::new(id).expect("event id"), spec))
    .collect::<BTreeMap<_, _>>();
    StrictRealizationContext::new(specs)
}

fn note_candidate(
    voice_id: &str,
    event: &str,
    pitch: u8,
    onset: Time,
    duration: Time,
) -> CompletionCandidate {
    CompletionCandidate::Note(NoteAddition {
        note: note(voice_id, event, pitch, onset, duration),
    })
}

fn tritone_density_metric(staff: &Staff, onset: Time) -> PitchDissonanceScore {
    let classes = staff
        .notes()
        .filter(|note| note.onset <= onset && onset < note.end())
        .map(|note| note.note.pitch.class)
        .collect::<Vec<_>>();
    let mask = PitchClassMask::from_pitch_classes(&classes);
    PitchDissonanceRegistry::new_with_builtins()
        .analyze_all(mask, &LabelContext::default())
        .into_iter()
        .find(|score| score.model == "tritone-density")
        .expect("tritone-density score")
}

#[test]
fn serial_completion_preserves_structural_plan_and_marks_sounding_repeats() {
    let realization = realize_strict(&strict_plan(), &strict_context()).expect("realization");
    let request = SerialCompletionRequest {
        completion: CompletionRequest {
            candidates: vec![note_candidate(
                "voice/high",
                "added-e",
                64,
                Time::from_integer(0),
                quarter(),
            )],
            min_candidates: 1,
            max_candidates: Some(1),
            pitch_ranges: Vec::new(),
        },
        allowances: SerialCompletionAllowances::default(),
    };

    let result = complete_serial(
        &realization,
        &practice(),
        &DeclaredWaivers::default(),
        &request,
        SearchControl::default(),
        &NeverInterrupt,
    )
    .expect("serial completion");

    assert_eq!(result.structural_plan, *realization.plan());
    assert_eq!(result.structural_before, result.structural_after);
    assert_eq!(result.generic.search.status, SearchStatus::Complete);
    assert_eq!(result.generic.provenance.selected_candidates, vec![0]);
    assert!(
        result
            .sounding_after
            .entries()
            .iter()
            .any(|entry| entry.rule_id.as_str() == "rule/repeats"
                && matches!(entry.status, crate::InvariantStatus::Violated))
    );
    let restored = remove_additive_staff_patch(&result.generic.after, &result.generic.patch)
        .expect("reverse patch");
    assert_eq!(restored, result.generic.before);
}

#[test]
fn serial_completion_rejects_future_remainder_without_allowance_and_accepts_with_it() {
    let realization = realize_strict(&strict_plan(), &strict_context()).expect("realization");
    let future_candidate = note_candidate(
        "voice/high",
        "future-b",
        71,
        Time::from_integer(0),
        quarter(),
    );
    let denied = complete_serial(
        &realization,
        &practice(),
        &DeclaredWaivers::default(),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![future_candidate.clone()],
                min_candidates: 1,
                max_candidates: Some(1),
                pitch_ranges: Vec::new(),
            },
            allowances: SerialCompletionAllowances {
                current_partition: false,
                stated_pitch_classes: false,
                ..SerialCompletionAllowances::default()
            },
        },
        SearchControl::default(),
        &NeverInterrupt,
    )
    .expect_err("future remainder should be rejected");
    assert!(matches!(
        denied,
        crate::SerialCompletionError::NoLegalCandidates(_)
    ));

    let admitted = complete_serial(
        &realization,
        &practice(),
        &DeclaredWaivers::default(),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![future_candidate],
                min_candidates: 1,
                max_candidates: Some(1),
                pitch_ranges: Vec::new(),
            },
            allowances: SerialCompletionAllowances {
                current_partition: false,
                stated_pitch_classes: false,
                aggregate_remainder: true,
                ..SerialCompletionAllowances::default()
            },
        },
        SearchControl::default(),
        &NeverInterrupt,
    )
    .expect("future remainder completion");
    assert_eq!(admitted.generic.provenance.selected_candidates, vec![0]);
}

#[test]
fn serial_completion_retains_partial_generic_receipt_under_result_bounds() {
    let realization = realize_strict(&strict_plan(), &strict_context()).expect("realization");
    let result = complete_serial(
        &realization,
        &practice(),
        &DeclaredWaivers::default(),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![
                    note_candidate(
                        "voice/high",
                        "bounded-e",
                        64,
                        Time::from_integer(0),
                        quarter(),
                    ),
                    note_candidate("voice/high", "bounded-f", 65, Time::new(1, 4), quarter()),
                ],
                min_candidates: 1,
                max_candidates: Some(1),
                pitch_ranges: Vec::new(),
            },
            allowances: SerialCompletionAllowances::default(),
        },
        SearchControl::default().with_max_results(1),
        &NeverInterrupt,
    )
    .expect("bounded serial completion");
    assert_eq!(result.generic.search.status, SearchStatus::Partial);
    assert_eq!(
        result.generic.search.reason.as_deref(),
        Some("result bound reached")
    );
}

#[test]
fn completion_allowance_classifier_covers_referential_modal_derived_and_foreign_paths() {
    let strict_realization =
        realize_strict(&strict_plan(), &strict_context()).expect("realization");
    let referential_note = note(
        "voice/high",
        "referential",
        65,
        Time::from_integer(0),
        quarter(),
    );
    let referential = classify_note(
        &SerialCandidateContext {
            realization: &strict_realization,
            allowances: &SerialCompletionAllowances {
                current_partition: false,
                stated_pitch_classes: false,
                referential_subsets: vec![
                    ReferentialSubsetAllowance::new("hexachord/a", [PitchClass::F])
                        .expect("subset"),
                ],
                ..SerialCompletionAllowances::default()
            },
        },
        &referential_note,
    );
    assert!(matches!(
        referential.selected.as_ref().map(|matched| &matched.kind),
        Some(SerialAllowanceKind::ReferentialSubset { .. })
    ));

    let registry = default_realizer_registry();
    let mut modal_context = strict_context();
    modal_context.modal_scale = Some(PlayerScale::from_scale(Scale::dorian(PitchClass::C)));
    let modal_realization = registry
        .realize_named(
            "realizer/modal-degree-cycle",
            &strict_plan(),
            &modal_context,
        )
        .expect("modal realization");
    let modal_pitch = modal_realization
        .spine_report()
        .expect("spine report")
        .entries[0]
        .landed_pitch
        .to_midi()
        .expect("midi");
    let modal = classify_note(
        &SerialCandidateContext {
            realization: &modal_realization,
            allowances: &SerialCompletionAllowances {
                current_partition: false,
                stated_pitch_classes: false,
                aggregate_remainder: false,
                modal_projection: true,
                ..SerialCompletionAllowances::default()
            },
        },
        &note(
            "voice/high",
            "modal",
            modal_pitch,
            Time::from_integer(0),
            quarter(),
        ),
    );
    assert!(matches!(
        modal.selected.as_ref().map(|matched| &matched.kind),
        Some(SerialAllowanceKind::ModalProjection)
    ));

    let realized_practice =
        realize_strict(&practice_plan(), &practice_context()).expect("practice realization");
    let derived = classify_note(
        &SerialCandidateContext {
            realization: &realized_practice,
            allowances: &SerialCompletionAllowances {
                current_partition: false,
                stated_pitch_classes: false,
                aggregate_remainder: false,
                derived_reservoir: true,
                ..SerialCompletionAllowances::default()
            },
        },
        &note("voice/middle", "derived", 64, quarter(), quarter()),
    );
    assert!(matches!(
        derived.selected.as_ref().map(|matched| &matched.kind),
        Some(SerialAllowanceKind::DerivedReservoir)
    ));

    let foreign = classify_note(
        &SerialCandidateContext {
            realization: &realized_practice,
            allowances: &SerialCompletionAllowances {
                current_partition: false,
                stated_pitch_classes: false,
                aggregate_remainder: false,
                explicitly_foreign_material: true,
                ..SerialCompletionAllowances::default()
            },
        },
        &note("voice/guest", "foreign", 70, quarter(), quarter()),
    );
    assert!(matches!(
        foreign.selected.as_ref().map(|matched| &matched.kind),
        Some(SerialAllowanceKind::ExplicitForeignMaterial)
    ));
}

#[test]
fn typed_additions_map_into_serial_allowances_and_categories() {
    let realization = realize_strict(&strict_plan(), &strict_context()).expect("realization");
    let cases = vec![
        CompletionCandidate::Chord(ChordAddition {
            label: Some("support".to_owned()),
            notes: vec![
                note(
                    "voice/high",
                    "support-root",
                    64,
                    Time::from_integer(0),
                    quarter(),
                ),
                note(
                    "voice/low",
                    "support-fifth",
                    67,
                    Time::from_integer(0),
                    quarter(),
                ),
            ],
        }),
        CompletionCandidate::Pedal(PedalAddition {
            label: Some("pedal".to_owned()),
            note: note("voice/low", "pedal", 52, Time::from_integer(0), quarter()),
        }),
        CompletionCandidate::Doubling(DoublingAddition {
            source_event_id: ObjectId::new("event/lead-a").expect("event id"),
            note: note(
                "voice/high",
                "double-lead",
                76,
                Time::from_integer(0),
                quarter(),
            ),
        }),
        CompletionCandidate::Ornament(OrnamentAddition {
            anchor_event_id: ObjectId::new("event/lead-a").expect("event id"),
            notes: vec![note(
                "voice/high",
                "neighbor",
                65,
                Time::new(1, 8),
                Time::new(1, 8),
            )],
        }),
        CompletionCandidate::Ornament(OrnamentAddition {
            anchor_event_id: ObjectId::new("event/chord-upper").expect("event id"),
            notes: vec![note(
                "voice/high",
                "passing",
                66,
                Time::new(3, 8),
                Time::new(1, 8),
            )],
        }),
        CompletionCandidate::Ornament(OrnamentAddition {
            anchor_event_id: ObjectId::new("event/tie-a").expect("event id"),
            notes: vec![note(
                "voice/inner",
                "suspension",
                61,
                Time::new(5, 8),
                Time::new(1, 8),
            )],
        }),
        CompletionCandidate::Ornament(OrnamentAddition {
            anchor_event_id: ObjectId::new("event/rest").expect("event id"),
            notes: vec![note(
                "voice/high",
                "anticipation",
                71,
                Time::new(7, 8),
                Time::new(1, 8),
            )],
        }),
    ];
    let expected_kinds = vec![
        crate::AdditionKind::Chord,
        crate::AdditionKind::Pedal,
        crate::AdditionKind::Doubling,
        crate::AdditionKind::Ornament,
        crate::AdditionKind::Ornament,
        crate::AdditionKind::Ornament,
        crate::AdditionKind::Ornament,
    ];
    for (candidate, expected_kind) in cases.into_iter().zip(expected_kinds) {
        let result = complete_serial(
            &realization,
            &practice(),
            &DeclaredWaivers::default(),
            &SerialCompletionRequest {
                completion: CompletionRequest {
                    candidates: vec![candidate],
                    min_candidates: 1,
                    max_candidates: Some(1),
                    pitch_ranges: Vec::new(),
                },
                allowances: SerialCompletionAllowances {
                    aggregate_remainder: true,
                    ..SerialCompletionAllowances::default()
                },
            },
            SearchControl::default(),
            &NeverInterrupt,
        )
        .expect("typed completion");
        assert_eq!(result.generic.provenance.selected_candidates, vec![0]);
        assert_eq!(result.accepted_additions[0].kind, expected_kind);
        assert!(
            result.accepted_additions[0]
                .notes
                .iter()
                .all(|note| matches!(note.category, AcceptedSerialCategory::RowNative))
        );
    }
}

#[test]
fn accepted_additions_classify_row_native_derived_modal_referential_and_foreign() {
    let row_native_realization =
        realize_strict(&strict_plan(), &strict_context()).expect("strict realization");
    let row_native = complete_serial(
        &row_native_realization,
        &practice(),
        &DeclaredWaivers::default(),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![note_candidate(
                    "voice/high",
                    "row-native",
                    64,
                    Time::from_integer(0),
                    quarter(),
                )],
                min_candidates: 1,
                max_candidates: Some(1),
                pitch_ranges: Vec::new(),
            },
            allowances: SerialCompletionAllowances::default(),
        },
        SearchControl::default(),
        &NeverInterrupt,
    )
    .expect("row-native completion");
    assert!(matches!(
        row_native.accepted_additions[0].notes[0].category,
        AcceptedSerialCategory::RowNative
    ));

    let realized_practice =
        realize_strict(&practice_plan(), &practice_context()).expect("practice realization");
    let derived = complete_serial(
        &realized_practice,
        &practice(),
        &DeclaredWaivers::default(),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![note_candidate(
                    "voice/middle",
                    "derived",
                    64,
                    quarter(),
                    quarter(),
                )],
                min_candidates: 1,
                max_candidates: Some(1),
                pitch_ranges: Vec::new(),
            },
            allowances: SerialCompletionAllowances {
                current_partition: false,
                stated_pitch_classes: false,
                aggregate_remainder: false,
                derived_reservoir: true,
                ..SerialCompletionAllowances::default()
            },
        },
        SearchControl::default(),
        &NeverInterrupt,
    )
    .expect("derived completion");
    assert!(matches!(
        derived.accepted_additions[0].notes[0].category,
        AcceptedSerialCategory::RowDerived
    ));

    let registry = default_realizer_registry();
    let mut modal_context = strict_context();
    modal_context.modal_scale = Some(PlayerScale::from_scale(Scale::dorian(PitchClass::C)));
    let modal_realization = registry
        .realize_named(
            "realizer/modal-degree-cycle",
            &strict_plan(),
            &modal_context,
        )
        .expect("modal realization");
    let modal_pitch = modal_realization
        .spine_report()
        .expect("spine report")
        .entries[0]
        .landed_pitch
        .to_midi()
        .expect("midi");
    let modal = complete_serial(
        &modal_realization,
        &practice(),
        &DeclaredWaivers::default(),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![note_candidate(
                    "voice/high",
                    "modal",
                    modal_pitch,
                    Time::from_integer(0),
                    quarter(),
                )],
                min_candidates: 1,
                max_candidates: Some(1),
                pitch_ranges: Vec::new(),
            },
            allowances: SerialCompletionAllowances {
                current_partition: false,
                stated_pitch_classes: false,
                aggregate_remainder: false,
                modal_projection: true,
                ..SerialCompletionAllowances::default()
            },
        },
        SearchControl::default(),
        &NeverInterrupt,
    )
    .expect("modal completion");
    assert!(matches!(
        modal.accepted_additions[0].notes[0].category,
        AcceptedSerialCategory::ModalProjected
    ));

    let referential = complete_serial(
        &row_native_realization,
        &practice(),
        &DeclaredWaivers::default(),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![note_candidate(
                    "voice/high",
                    "referential",
                    65,
                    Time::from_integer(0),
                    quarter(),
                )],
                min_candidates: 1,
                max_candidates: Some(1),
                pitch_ranges: Vec::new(),
            },
            allowances: SerialCompletionAllowances {
                current_partition: false,
                stated_pitch_classes: false,
                referential_subsets: vec![
                    ReferentialSubsetAllowance::new("hexachord/a", [PitchClass::F])
                        .expect("subset"),
                ],
                ..SerialCompletionAllowances::default()
            },
        },
        SearchControl::default(),
        &NeverInterrupt,
    )
    .expect("referential completion");
    assert!(matches!(
        &referential.accepted_additions[0].notes[0].category,
        AcceptedSerialCategory::Referential { id } if id == "hexachord/a"
    ));

    let foreign_waiver = WaiverId::new("waiver/foreign-material").expect("waiver id");
    let foreign = complete_serial(
        &realized_practice,
        &practice(),
        &DeclaredWaivers::new([(
            PracticeRuleId::new("rule/foreign").expect("rule id"),
            foreign_waiver.clone(),
        )]),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![note_candidate(
                    "voice/guest",
                    "foreign",
                    70,
                    quarter(),
                    quarter(),
                )],
                min_candidates: 1,
                max_candidates: Some(1),
                pitch_ranges: Vec::new(),
            },
            allowances: SerialCompletionAllowances {
                current_partition: false,
                stated_pitch_classes: false,
                aggregate_remainder: false,
                explicitly_foreign_material: true,
                ..SerialCompletionAllowances::default()
            },
        },
        SearchControl::default(),
        &NeverInterrupt,
    )
    .expect("foreign completion");
    assert!(matches!(
        &foreign.accepted_additions[0].notes[0].category,
        AcceptedSerialCategory::ForeignWithWaiver { waiver } if waiver == &foreign_waiver
    ));
}

#[path = "tests_completion_more.rs"]
mod tests_completion_more;
