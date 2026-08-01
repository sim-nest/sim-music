use std::collections::BTreeMap;

use sim_lib_discrete_search::SearchControl;
use sim_lib_music_core::{Articulation, Channel, ObjectId, PitchClass, Score, Time};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

use crate::{
    EventPlacement, ExtractionOutcome, OrdinalRef, PlannedSerialEvent, RowInstanceId,
    SerialEventId, SerialExtractionRequest, SerialExtractionServices, SerialOrigin, SerialPlan,
    SerialRenderOptions, SerialRole, SimultaneousGroupId, StrictEventSpec,
    StrictRealizationContext, extract_serial_hypotheses, realize_strict, render_serial_score,
};

fn op25_form() -> sim_lib_pitch_serial::RowForm {
    let row = ToneRow::try_from_classes([
        PitchClass::E,
        PitchClass::F,
        PitchClass::G,
        PitchClass::CS,
        PitchClass::FS,
        PitchClass::DS,
        PitchClass::GS,
        PitchClass::D,
        PitchClass::B,
        PitchClass::C,
        PitchClass::A,
        PitchClass::AS,
    ])
    .expect("row");
    row.apply(RowOperation::new(RowFamily::P, 0))
}

fn voice(name: &str) -> ObjectId {
    ObjectId::new(name).expect("voice id")
}

fn quarter() -> Time {
    Time::new(1, 4)
}

fn event(id: &str, ordinals: &[usize], voice_name: &str) -> PlannedSerialEvent {
    let row_id = RowInstanceId::new("row/op25/p0").expect("row id");
    PlannedSerialEvent {
        id: SerialEventId::new(id).expect("event id"),
        ordinals: ordinals
            .iter()
            .copied()
            .map(|ordinal| OrdinalRef::new(row_id.clone(), ordinal))
            .collect(),
        role: SerialRole::Structural,
        origin: SerialOrigin::Structural {
            rationale: "row statement".to_owned(),
        },
        voice: voice(voice_name),
        placement: EventPlacement::independent(),
        parents: Vec::new(),
    }
}

fn linear_serial_score() -> Score {
    let row_id = RowInstanceId::new("row/op25/p0").expect("row id");
    let mut rows = BTreeMap::new();
    rows.insert(row_id, op25_form());
    let event_ids = (0..12)
        .map(|ordinal| format!("event/linear-{ordinal}"))
        .collect::<Vec<_>>();
    let events = event_ids
        .iter()
        .enumerate()
        .map(|(ordinal, id)| {
            let event = event(
                id,
                &[ordinal],
                if ordinal % 2 == 0 {
                    "voice/high"
                } else {
                    "voice/low"
                },
            );
            (event.id.clone(), event)
        })
        .collect();
    let precedence = event_ids
        .windows(2)
        .map(|pair| {
            (
                SerialEventId::new(&pair[0]).expect("event id"),
                SerialEventId::new(&pair[1]).expect("event id"),
            )
        })
        .collect::<Vec<_>>();
    let plan = SerialPlan::try_new(rows, events, precedence).expect("plan");
    let channel = Channel::new(0).expect("channel");
    let specs = event_ids
        .iter()
        .enumerate()
        .map(|(ordinal, id)| {
            (
                SerialEventId::new(id).expect("event id"),
                StrictEventSpec::notes(
                    if ordinal % 2 == 0 { 5 } else { 3 },
                    quarter(),
                    96,
                    channel,
                    Articulation::Normal,
                ),
            )
        })
        .collect();
    let realization =
        realize_strict(&plan, &StrictRealizationContext::new(specs)).expect("realization");
    render_serial_score(&realization, &SerialRenderOptions::default()).expect("score")
}

fn ambiguous_partition_score() -> Score {
    let row_id = RowInstanceId::new("row/op25/p0").expect("row id");
    let mut rows = BTreeMap::new();
    rows.insert(row_id, op25_form());
    let group = SimultaneousGroupId::new("simul/opening").expect("group");
    let mut events = vec![
        PlannedSerialEvent {
            placement: EventPlacement::simultaneous(group.clone()),
            ..event("event/open-a", &[0], "voice/high")
        },
        PlannedSerialEvent {
            placement: EventPlacement::simultaneous(group),
            ..event("event/open-b", &[1], "voice/low")
        },
    ];
    events.extend(
        (2..12).map(|ordinal| event(&format!("event/after-{ordinal}"), &[ordinal], "voice/high")),
    );
    let event_map = events
        .into_iter()
        .map(|event| (event.id.clone(), event))
        .collect();
    let precedence = (2..12)
        .scan(
            SerialEventId::new("event/open-a").expect("event id"),
            |prev, ordinal| {
                let next = SerialEventId::new(format!("event/after-{ordinal}")).expect("event id");
                let edge = (prev.clone(), next.clone());
                *prev = next;
                Some(edge)
            },
        )
        .collect::<Vec<_>>();
    let plan = SerialPlan::try_new(rows, event_map, precedence).expect("plan");
    let channel = Channel::new(0).expect("channel");
    let mut specs = BTreeMap::new();
    specs.insert(
        SerialEventId::new("event/open-a").expect("event id"),
        StrictEventSpec::notes(5, quarter(), 96, channel, Articulation::Accent),
    );
    specs.insert(
        SerialEventId::new("event/open-b").expect("event id"),
        StrictEventSpec::notes(3, quarter(), 96, channel, Articulation::Accent),
    );
    for ordinal in 2..12 {
        specs.insert(
            SerialEventId::new(format!("event/after-{ordinal}")).expect("event id"),
            StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Normal),
        );
    }
    let realization =
        realize_strict(&plan, &StrictRealizationContext::new(specs)).expect("realization");
    render_serial_score(&realization, &SerialRenderOptions::default()).expect("score")
}

fn repeated_pitch_score() -> Score {
    let row_id = RowInstanceId::new("row/op25/p0").expect("row id");
    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), op25_form());
    let mut items = Vec::new();
    for ordinal in 0..12 {
        items.push(event(
            &format!("event/struct-{ordinal}"),
            &[ordinal],
            "voice/high",
        ));
    }
    items.insert(
        2,
        PlannedSerialEvent {
            id: SerialEventId::new("event/repeat-0").expect("event id"),
            ordinals: vec![OrdinalRef::new(row_id, 0)],
            role: SerialRole::Ornamental,
            origin: SerialOrigin::Ornamental {
                technique: "repeat".to_owned(),
            },
            voice: voice("voice/high"),
            placement: EventPlacement::independent(),
            parents: vec![SerialEventId::new("event/struct-0").expect("event id")],
        },
    );
    let events = items
        .iter()
        .cloned()
        .map(|event| (event.id.clone(), event))
        .collect();
    let precedence = items
        .windows(2)
        .map(|pair| (pair[0].id.clone(), pair[1].id.clone()))
        .collect::<Vec<_>>();
    let plan = SerialPlan::try_new(rows, events, precedence).expect("plan");
    let channel = Channel::new(0).expect("channel");
    let specs = items
        .iter()
        .map(|event| {
            (
                event.id.clone(),
                StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Normal),
            )
        })
        .collect();
    let realization =
        realize_strict(&plan, &StrictRealizationContext::new(specs)).expect("realization");
    render_serial_score(&realization, &SerialRenderOptions::default()).expect("score")
}

#[test]
fn extraction_round_trips_linear_serial_scores() {
    let outcome = extract_serial_hypotheses(
        &linear_serial_score(),
        &SerialExtractionRequest::default(),
        &SerialExtractionServices::default(),
    )
    .expect("extract");
    let ExtractionOutcome::Complete {
        hypothesis, ranked, ..
    } = outcome
    else {
        panic!("linear fixture should extract one complete hypothesis");
    };
    assert_eq!(ranked.len(), 1);
    assert_eq!(hypothesis.omissions, 0);
    assert_eq!(hypothesis.duplicates_before_completion, 0);
    assert_eq!(hypothesis.row.classes(), op25_form().classes());
}

#[test]
fn extraction_tracks_crossed_voices_without_losing_coverage() {
    let outcome = extract_serial_hypotheses(
        &linear_serial_score(),
        &SerialExtractionRequest::default(),
        &SerialExtractionServices::default(),
    )
    .expect("extract");
    let ExtractionOutcome::Complete { hypothesis, .. } = outcome else {
        panic!("crossed-voice fixture should complete");
    };
    let observed_voices = hypothesis
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .observations
                .iter()
                .map(|observation| observation.voice_id.clone())
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert!(observed_voices.len() >= 2);
    assert_eq!(hypothesis.aliases.len(), 48);
}

#[test]
fn extraction_reports_simultaneous_partitions_and_ambiguity() {
    let outcome = extract_serial_hypotheses(
        &ambiguous_partition_score(),
        &SerialExtractionRequest::default(),
        &SerialExtractionServices::default(),
    )
    .expect("extract");
    let ExtractionOutcome::Ambiguous { ranked, .. } = outcome else {
        panic!("opening partition fixture is deliberately ambiguous");
    };
    assert!(ranked.len() >= 2);
    assert!(
        ranked
            .windows(2)
            .all(|pair| pair[0].stable_rank <= pair[1].stable_rank)
    );
    assert!(
        ranked[0]
            .blocks
            .iter()
            .any(|block| block.observations.len() == 2)
    );
}

#[test]
fn extraction_counts_repetition_before_aggregate_completion() {
    let outcome = extract_serial_hypotheses(
        &repeated_pitch_score(),
        &SerialExtractionRequest::default(),
        &SerialExtractionServices::default(),
    )
    .expect("extract");
    let ExtractionOutcome::Complete { hypothesis, .. } = outcome else {
        panic!("repetition fixture should still complete");
    };
    assert_eq!(hypothesis.omissions, 0);
    assert!(hypothesis.duplicates_before_completion >= 1);
}

#[test]
fn extraction_surfaces_budget_exhaustion_from_generic_search() {
    let outcome = extract_serial_hypotheses(
        &ambiguous_partition_score(),
        &SerialExtractionRequest {
            search: SearchControl::default().with_max_results(1),
            ..SerialExtractionRequest::default()
        },
        &SerialExtractionServices::default(),
    )
    .expect("extract");
    let ExtractionOutcome::BudgetExhausted { ranked, evidence } = outcome else {
        panic!("max-results bound should stop the search early");
    };
    assert_eq!(
        evidence.search.reason.as_deref(),
        Some("result bound reached")
    );
    assert!(!ranked.is_empty());
}
