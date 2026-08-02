use sim_lib_discrete_graph::{AlgorithmControl, AlignmentWindow};
use sim_lib_discrete_search::{SearchControl, SearchStatus};
use sim_lib_music_core::{
    Articulation, Channel, Note, ObjectId, Pitch, Staff, StaffNote, StaffVoice, Time,
};

use crate::{
    AnalysisError, AnalysisEvent, AnalysisTransform, Meter, MetricalLattice,
    PatternDiscoveryPolicy, PatternOverlapPolicy, QuantizationPlan, SimilarityFeature,
    SimilarityInvariances, SimilarityPlan, SwingPolicy, analysis_events, compare_sequences,
    discover_patterns, quantize_staff,
};

// conformance: exact quantization, similarity, and bounded pattern evidence retain policy and identity.

fn id(value: impl Into<String>) -> ObjectId {
    ObjectId::new(value).expect("identity")
}

fn note(event: &str, onset: Time, duration: Time, semitone: i32) -> StaffNote {
    StaffNote {
        voice_id: id("voice/main"),
        note_id: id(format!("note/{event}")),
        event_id: id(format!("event/{event}")),
        onset,
        note: Note::new(
            duration,
            Pitch::from_semitone(semitone),
            100,
            Channel::new(0).expect("channel"),
            Articulation::Normal,
        )
        .expect("note"),
    }
}

fn staff(notes: Vec<StaffNote>) -> Staff {
    let duration = notes
        .iter()
        .map(StaffNote::end)
        .max()
        .unwrap_or_else(|| Time::from_integer(0));
    Staff::new(vec![StaffVoice {
        id: id("voice/main"),
        name: "main".to_owned(),
        duration,
        notes,
    }])
    .expect("staff")
}

fn event(index: usize, onset: Time, duration: Time, pitch: i32) -> AnalysisEvent {
    AnalysisEvent {
        voice_id: id("voice/sequence"),
        note_id: id(format!("note/sequence/{index}")),
        event_id: id(format!("event/sequence/{index}")),
        onset,
        duration,
        pitch: Pitch::from_semitone(pitch),
    }
}

#[test]
fn metrical_quantization_uses_swing_tuplets_and_lists_every_exact_edit() {
    let source = staff(vec![
        note("zero", Time::new(0, 1), Time::new(1, 32), 60),
        note("triplet", Time::new(2, 25), Time::new(1, 32), 62),
        note("swing", Time::new(17, 100), Time::new(1, 32), 64),
    ]);
    let plan = QuantizationPlan {
        lattice: MetricalLattice {
            tempo_bpm: Time::from_integer(120),
            meter: Meter {
                beats_per_bar: 4,
                beat_unit: 4,
            },
            subdivision: 2,
            swing: SwingPolicy::Ratio { long: 2, short: 1 },
            tuplets: vec![3],
        },
        tolerance: Time::new(1, 100),
        max_alternatives: 3,
        max_lattice_points: 1_000,
        control: AlgorithmControl::default()
            .with_max_work(100_000)
            .with_max_memory_cells(10_000),
    };
    let report = quantize_staff(&source, &plan).expect("quantized");
    let times = report
        .output
        .notes()
        .map(|note| note.onset)
        .collect::<Vec<_>>();
    assert_eq!(
        times,
        vec![Time::new(0, 1), Time::new(1, 12), Time::new(1, 6)]
    );
    assert_eq!(report.transform.edits.len(), 2);
    assert_eq!(report.plan, plan);
    assert_eq!(report.decisions.len(), 3);
    assert!(
        report
            .decisions
            .iter()
            .all(|decision| !decision.alternatives.is_empty())
    );
    assert!(report.cost > 0.0);
    assert!(report.alignment.receipt.work_used > 0);
    assert_eq!(
        source.notes().map(|note| note.onset).collect::<Vec<_>>(),
        vec![Time::new(0, 1), Time::new(2, 25), Time::new(17, 100),]
    );

    let strict = QuantizationPlan {
        tolerance: Time::new(1, 1000),
        ..plan
    };
    let strict = quantize_staff(&source, &strict).expect("strict quantization");
    assert!(strict.transform.edits.is_empty());
    assert_eq!(strict.output, source);
    assert!(
        strict
            .decisions
            .iter()
            .skip(1)
            .all(|decision| decision.selected.is_none())
    );
}

#[test]
fn melody_and_rhythm_similarity_state_invariances_and_keep_both_engines() {
    let left = vec![
        event(0, Time::new(0, 1), Time::new(1, 8), 60),
        event(1, Time::new(1, 4), Time::new(1, 8), 62),
        event(2, Time::new(1, 2), Time::new(1, 8), 65),
    ];
    let right = vec![
        event(10, Time::new(1, 1), Time::new(1, 16), 65),
        event(11, Time::new(9, 8), Time::new(1, 16), 67),
        event(12, Time::new(5, 4), Time::new(1, 16), 70),
    ];
    let base = SimilarityPlan {
        feature: SimilarityFeature::AbsolutePitch,
        invariances: SimilarityInvariances {
            transposition: true,
            time_scale: true,
        },
        gap_cost: 12.0,
        alignment_window: AlignmentWindow::Unbounded,
        control: AlgorithmControl::default()
            .with_max_work(100_000)
            .with_max_memory_cells(10_000),
        max_alternatives: 4,
    };
    let melody = compare_sequences(&left, &right, &base).expect("melody similarity");
    assert_eq!(melody.invariances, base.invariances);
    assert_eq!(melody.plan, base);
    assert_eq!(
        melody.selected().transform,
        AnalysisTransform {
            transposition: -5,
            time_scale: Time::from_integer(2),
            time_shift: Time::from_integer(-2),
        }
    );
    assert!(melody.alternatives.len() >= 2);
    assert_eq!(melody.selected().alignment.score, 0.0);
    assert!(melody.selected().correlation.coefficient > 0.99);
    assert!(melody.selected().alignment.receipt.work_used > 0);
    assert!(!melody.selected().correlation.result.lags.is_empty());

    let rhythm = compare_sequences(
        &left,
        &right,
        &SimilarityPlan {
            feature: SimilarityFeature::InterOnsetRhythm,
            invariances: SimilarityInvariances {
                transposition: false,
                time_scale: true,
            },
            ..base
        },
    )
    .expect("rhythm similarity");
    assert_eq!(
        rhythm.selected().transform.time_scale,
        Time::from_integer(2)
    );
    assert_eq!(rhythm.selected().alignment.score, 0.0);
}

#[test]
fn pattern_discovery_hash_filters_then_exact_verifies_bounded_occurrences() {
    let events = vec![
        event(0, Time::new(0, 1), Time::new(1, 8), 60),
        event(1, Time::new(1, 4), Time::new(1, 8), 62),
        event(2, Time::new(1, 2), Time::new(1, 8), 64),
        event(3, Time::new(1, 1), Time::new(1, 4), 65),
        event(4, Time::new(3, 2), Time::new(1, 4), 67),
        event(5, Time::new(2, 1), Time::new(1, 4), 69),
    ];
    let policy = PatternDiscoveryPolicy {
        min_events: 3,
        max_events: 3,
        min_support: 2,
        invariances: SimilarityInvariances {
            transposition: true,
            time_scale: true,
        },
        overlap: PatternOverlapPolicy::DisallowSharedEvents,
        max_windows: 100,
        max_candidate_pairs: 100,
        max_hash_bytes: 100_000,
        search: SearchControl::default()
            .with_max_work(10_000)
            .with_max_memory_nodes(1_000),
    };
    let report = discover_patterns(&events, &policy).expect("patterns");
    assert_eq!(report.search.status, SearchStatus::Complete);
    assert_eq!(report.policy, policy);
    assert!(report.resources.windows > 0);
    assert!(report.resources.candidate_pairs > 0);
    let pattern = report
        .patterns
        .iter()
        .find(|pattern| pattern.prototype_event_ids[0].as_str() == "event/sequence/0")
        .expect("three-note motif");
    assert_eq!(pattern.event_count, 3);
    assert_eq!(pattern.occurrences.len(), 2);
    assert_eq!(pattern.occurrences[0].cost, 0.0);
    assert_eq!(pattern.occurrences[1].transform.transposition, 5);
    assert_eq!(
        pattern.occurrences[1].transform.time_scale,
        Time::from_integer(2)
    );
    assert_eq!(
        pattern.occurrences[1].event_ids[0].as_str(),
        "event/sequence/3"
    );
    assert_ne!(
        pattern.occurrences[0].occurrence_id,
        pattern.occurrences[1].occurrence_id
    );

    let bounded = PatternDiscoveryPolicy {
        max_windows: 1,
        ..policy.clone()
    };
    assert!(matches!(
        discover_patterns(&events, &bounded),
        Err(AnalysisError::ResourceLimit {
            resource: "pattern windows",
            ..
        })
    ));

    let unbounded_search = PatternDiscoveryPolicy {
        search: SearchControl::default(),
        ..policy
    };
    assert!(matches!(
        discover_patterns(&events, &unbounded_search),
        Err(AnalysisError::InvalidPolicy {
            field: "pattern search",
            ..
        })
    ));
}

#[test]
fn staff_projection_retains_all_occurrence_identities() {
    let staff = staff(vec![
        note("a", Time::new(0, 1), Time::new(1, 4), 60),
        note("b", Time::new(1, 4), Time::new(1, 4), 62),
    ]);
    let projected = analysis_events(&staff);
    assert_eq!(projected[0].voice_id.as_str(), "voice/main");
    assert_eq!(projected[0].note_id.as_str(), "note/a");
    assert_eq!(projected[1].event_id.as_str(), "event/b");
}
