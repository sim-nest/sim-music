use super::*;

#[test]
fn ornament_resolution_and_doubling_preserve_parent_evidence_and_multiplicity() {
    let realization = realize_strict(&strict_plan(), &strict_context()).expect("realization");
    let result = complete_serial(
        &realization,
        &practice(),
        &DeclaredWaivers::default(),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![
                    CompletionCandidate::Ornament(OrnamentAddition {
                        anchor_event_id: ObjectId::new("event/lead-a").expect("event id"),
                        notes: vec![note(
                            "voice/high",
                            "turn",
                            65,
                            Time::new(1, 8),
                            Time::new(1, 8),
                        )],
                    }),
                    CompletionCandidate::Doubling(DoublingAddition {
                        source_event_id: ObjectId::new("event/lead-a").expect("event id"),
                        note: note("voice/high", "double", 76, Time::from_integer(0), quarter()),
                    }),
                ],
                min_candidates: 2,
                max_candidates: Some(2),
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
    .expect("ornament and doubling");

    let first_added = result
        .generic
        .after
        .notes()
        .filter(|note| note.event_id == ObjectId::new("event/double").expect("event id"))
        .count();
    assert_eq!(first_added, 1);
    assert_eq!(
        result
            .generic
            .after
            .notes()
            .filter(|note| note.onset == Time::from_integer(0)
                && note.note.pitch.class == PitchClass::E)
            .count(),
        2
    );
}

#[test]
fn low_register_and_unresolved_ornament_requests_are_rejected() {
    let realization = realize_strict(&strict_plan(), &strict_context()).expect("realization");
    let low_register = complete_serial(
        &realization,
        &practice(),
        &DeclaredWaivers::default(),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![note_candidate(
                    "voice/high",
                    "too-low",
                    40,
                    Time::from_integer(0),
                    quarter(),
                )],
                min_candidates: 1,
                max_candidates: Some(1),
                pitch_ranges: vec![PitchRangeConstraint {
                    voice_id: Some(ObjectId::new("voice/high").expect("voice id")),
                    lowest: Pitch::from_midi(60),
                    highest: Pitch::from_midi(84),
                }],
            },
            allowances: SerialCompletionAllowances::default(),
        },
        SearchControl::default(),
        &NeverInterrupt,
    )
    .expect_err("low register should fail");
    assert!(matches!(
        low_register,
        crate::SerialCompletionError::Completion(crate::CompletionError::InvalidCandidate(_))
    ));

    let unresolved = complete_serial(
        &realization,
        &practice(),
        &DeclaredWaivers::default(),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![CompletionCandidate::Ornament(OrnamentAddition {
                    anchor_event_id: ObjectId::new("event/unison-a").expect("event id"),
                    notes: vec![note(
                        "voice/high",
                        "unresolved",
                        70,
                        Time::new(9, 8),
                        Time::new(1, 8),
                    )],
                })],
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
    .expect_err("unresolved ornament should fail");
    assert!(matches!(
        unresolved,
        crate::SerialCompletionError::NoLegalCandidates(_)
    ));
}

#[test]
fn impossible_request_and_density_only_improvement_stay_honest() {
    let realization = realize_strict(&strict_plan(), &strict_context()).expect("realization");
    let impossible = complete_serial(
        &realization,
        &practice(),
        &DeclaredWaivers::default(),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![note_candidate(
                    "voice/high",
                    "single",
                    64,
                    Time::from_integer(0),
                    quarter(),
                )],
                min_candidates: 2,
                max_candidates: Some(2),
                pitch_ranges: Vec::new(),
            },
            allowances: SerialCompletionAllowances::default(),
        },
        SearchControl::default(),
        &NeverInterrupt,
    )
    .expect_err("impossible request should fail");
    assert!(matches!(
        impossible,
        crate::SerialCompletionError::Completion(crate::CompletionError::NoCompletion { .. })
    ));

    let density = complete_serial(
        &realization,
        &practice(),
        &DeclaredWaivers::default(),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![
                    note_candidate(
                        "voice/high",
                        "density-e",
                        64,
                        Time::from_integer(0),
                        quarter(),
                    ),
                    note_candidate(
                        "voice/low",
                        "density-g",
                        67,
                        Time::from_integer(0),
                        quarter(),
                    ),
                ],
                min_candidates: 2,
                max_candidates: Some(2),
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
    .expect("density improvement");
    let before = tritone_density_metric(&density.generic.before, Time::from_integer(0));
    let after = tritone_density_metric(&density.generic.after, Time::from_integer(0));
    assert!(after.sonance.normalized_density <= before.sonance.normalized_density);
    assert!(after.sonance.roughness_mass >= before.sonance.roughness_mass);
    assert!(
        density
            .generic
            .provenance
            .facts
            .iter()
            .all(|fact| !fact.contains("LessRough"))
    );
}
