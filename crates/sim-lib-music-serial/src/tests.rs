use std::collections::BTreeMap;
use std::sync::Arc;

use sim_lib_music_core::{Articulation, Channel, ObjectId, PitchClass, Score, Time};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

use crate::{
    BuiltInPracticeRule, DeclaredWaivers, EventPlacement, InvariantStatus, OrdinalRef,
    PlannedSerialEvent, PracticeId, PracticeRuleId, RowInstanceId, SerialEventId, SerialOrigin,
    SerialPlan, SerialPlanError, SerialPractice, SerialReading, SerialRenderOptions, SerialRole,
    SimultaneousGroupId, StrictEventSpec, StrictPitchLayout, StrictRealizationContext, TiePolicy,
    WaiverId, realize_strict, render_serial_piano_roll, render_serial_score, render_serial_staff,
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
    event_for_row(row_id, id, ordinals, voice_name)
}

fn event_for_row(
    row_id: RowInstanceId,
    id: &str,
    ordinals: &[usize],
    voice_name: &str,
) -> PlannedSerialEvent {
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

#[test]
fn serial_plan_accepts_chords_and_multi_voice_row_identity() {
    let row_id = RowInstanceId::new("row/op25/p0").expect("row id");
    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), op25_form());

    let group = SimultaneousGroupId::new("simul/chord-a").expect("group");
    let events = vec![
        PlannedSerialEvent {
            placement: EventPlacement::simultaneous(group.clone()),
            ..event("event/chord-upper", &[0, 4], "voice/soprano")
        },
        PlannedSerialEvent {
            placement: EventPlacement::simultaneous(group),
            ..event("event/chord-lower", &[7], "voice/alto")
        },
        event("event/bass-1", &[1, 2, 3], "voice/bass"),
        event("event/tenor-1", &[5], "voice/tenor"),
        event("event/tenor-2", &[6], "voice/tenor"),
        event("event/alto-2", &[8], "voice/alto"),
        event("event/soprano-2", &[9], "voice/soprano"),
        event("event/bass-2", &[10, 11], "voice/bass"),
    ]
    .into_iter()
    .map(|event| (event.id.clone(), event))
    .collect();

    let plan = SerialPlan::try_new(
        rows,
        events,
        [
            (
                SerialEventId::new("event/chord-upper").unwrap(),
                SerialEventId::new("event/bass-1").unwrap(),
            ),
            (
                SerialEventId::new("event/chord-lower").unwrap(),
                SerialEventId::new("event/bass-1").unwrap(),
            ),
        ],
    )
    .expect("plan");

    let simultaneous = plan.simultaneous_groups();
    assert_eq!(simultaneous.len(), 1);
    assert_eq!(simultaneous.values().next().expect("group").len(), 2);
    assert!(plan.events().values().any(|event| {
        event.voice == voice("voice/alto")
            && event
                .ordinals
                .iter()
                .any(|ordinal| ordinal.row_id == row_id)
    }));
}

#[test]
fn serial_plan_rejects_parentless_ornament() {
    let row_id = RowInstanceId::new("row/op25/p0").expect("row id");
    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), op25_form());
    let event_id = SerialEventId::new("event/turn").expect("event id");
    let event = PlannedSerialEvent {
        id: event_id.clone(),
        ordinals: vec![OrdinalRef::new(row_id, 0)],
        role: SerialRole::Ornamental,
        origin: SerialOrigin::Ornamental {
            technique: "turn".to_owned(),
        },
        voice: voice("voice/soprano"),
        placement: EventPlacement::independent(),
        parents: Vec::new(),
    };

    let error = SerialPlan::try_new(
        rows,
        [(event_id.clone(), event)].into_iter().collect(),
        std::iter::empty(),
    )
    .expect_err("ornament without parents should fail");
    assert_eq!(
        error,
        SerialPlanError::MissingParents {
            event_id,
            role: "ornamental",
        }
    );
}

#[test]
fn serial_plan_rejects_precedence_inside_one_simultaneous_group() {
    let row_id = RowInstanceId::new("row/op25/p0").expect("row id");
    let mut rows = BTreeMap::new();
    rows.insert(row_id, op25_form());
    let group = SimultaneousGroupId::new("simul/a").expect("group");
    let left = PlannedSerialEvent {
        placement: EventPlacement::simultaneous(group.clone()),
        ..event("event/a", &[0, 1, 2, 3, 4, 5], "voice/high")
    };
    let right = PlannedSerialEvent {
        placement: EventPlacement::simultaneous(group.clone()),
        ..event("event/b", &[6, 7, 8, 9, 10, 11], "voice/low")
    };
    let events = [(left.id.clone(), left), (right.id.clone(), right)]
        .into_iter()
        .collect();

    let error = SerialPlan::try_new(
        rows,
        events,
        [(
            SerialEventId::new("event/a").unwrap(),
            SerialEventId::new("event/b").unwrap(),
        )],
    )
    .expect_err("simultaneous precedence should fail");
    assert_eq!(
        error,
        SerialPlanError::SimultaneousPrecedenceConflict {
            group_id: group,
            before: SerialEventId::new("event/a").unwrap(),
            after: SerialEventId::new("event/b").unwrap(),
        }
    );
}

#[test]
fn serial_plan_rejects_missing_structural_coverage() {
    let row_id = RowInstanceId::new("row/op25/p0").expect("row id");
    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), op25_form());
    let events = [event("event/source", &[0, 1, 2], "voice/soprano")]
        .into_iter()
        .map(|event| (event.id.clone(), event))
        .collect();

    let error = SerialPlan::try_new(rows, events, std::iter::empty())
        .expect_err("incomplete structural coverage should fail");
    assert_eq!(
        error,
        SerialPlanError::MissingStructuralCoverage {
            row_id,
            ordinals: vec![3, 4, 5, 6, 7, 8, 9, 10, 11],
        }
    );
}

fn strict_plan() -> SerialPlan {
    let row_id = RowInstanceId::new("row/op25/p0").expect("row id");
    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), op25_form());
    let chord_group = SimultaneousGroupId::new("simul/chord").expect("group");
    let unison_group = SimultaneousGroupId::new("simul/unison").expect("group");
    let events = vec![
        event("event/lead-a", &[0], "voice/high"),
        PlannedSerialEvent {
            placement: EventPlacement::simultaneous(chord_group.clone()),
            ..event("event/chord-upper", &[1, 2], "voice/high")
        },
        PlannedSerialEvent {
            placement: EventPlacement::simultaneous(chord_group),
            ..event("event/chord-lower", &[3, 4], "voice/low")
        },
        event("event/tie-a", &[5], "voice/inner"),
        event("event/tie-b", &[5], "voice/inner"),
        event("event/cross-low", &[9], "voice/high"),
        event("event/cross-high", &[6], "voice/low"),
        event("event/rest", &[7, 10, 11], "voice/high"),
        PlannedSerialEvent {
            placement: EventPlacement::simultaneous(unison_group.clone()),
            ..event("event/unison-a", &[8], "voice/high")
        },
        PlannedSerialEvent {
            placement: EventPlacement::simultaneous(unison_group),
            ..event("event/unison-b", &[8], "voice/low")
        },
    ]
    .into_iter()
    .map(|event| (event.id.clone(), event))
    .collect();
    SerialPlan::try_new(
        rows,
        events,
        [
            (
                SerialEventId::new("event/lead-a").unwrap(),
                SerialEventId::new("event/chord-upper").unwrap(),
            ),
            (
                SerialEventId::new("event/lead-a").unwrap(),
                SerialEventId::new("event/chord-lower").unwrap(),
            ),
            (
                SerialEventId::new("event/chord-upper").unwrap(),
                SerialEventId::new("event/tie-a").unwrap(),
            ),
            (
                SerialEventId::new("event/chord-lower").unwrap(),
                SerialEventId::new("event/tie-a").unwrap(),
            ),
            (
                SerialEventId::new("event/tie-a").unwrap(),
                SerialEventId::new("event/tie-b").unwrap(),
            ),
            (
                SerialEventId::new("event/tie-b").unwrap(),
                SerialEventId::new("event/cross-low").unwrap(),
            ),
            (
                SerialEventId::new("event/tie-b").unwrap(),
                SerialEventId::new("event/cross-high").unwrap(),
            ),
            (
                SerialEventId::new("event/cross-low").unwrap(),
                SerialEventId::new("event/rest").unwrap(),
            ),
            (
                SerialEventId::new("event/cross-high").unwrap(),
                SerialEventId::new("event/rest").unwrap(),
            ),
            (
                SerialEventId::new("event/rest").unwrap(),
                SerialEventId::new("event/unison-a").unwrap(),
            ),
            (
                SerialEventId::new("event/rest").unwrap(),
                SerialEventId::new("event/unison-b").unwrap(),
            ),
        ],
    )
    .expect("plan")
}

fn practice_plan() -> SerialPlan {
    let row_id = RowInstanceId::new("row/practice/p0").expect("row id");
    let alt_row_id = RowInstanceId::new("row/practice/alt").expect("row id");
    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), op25_form());
    rows.insert(alt_row_id.clone(), op25_form());
    let simult = SimultaneousGroupId::new("simul/practice").expect("group");
    let structural_a = event_for_row(
        row_id.clone(),
        "event/struct-a",
        &[0, 1, 2, 3, 4, 5],
        "voice/high",
    );
    let structural_b = event_for_row(
        row_id.clone(),
        "event/struct-b",
        &[6, 7, 8, 9, 10, 11],
        "voice/low",
    );
    let structural_c = event_for_row(
        alt_row_id.clone(),
        "event/struct-c",
        &[0, 1, 2, 3, 4, 5],
        "voice/alto",
    );
    let structural_d = event_for_row(
        alt_row_id.clone(),
        "event/struct-d",
        &[6, 7, 8, 9, 10, 11],
        "voice/tenor",
    );
    let derived = PlannedSerialEvent {
        id: SerialEventId::new("event/derived-repeat").expect("event id"),
        ordinals: vec![
            OrdinalRef::new(row_id.clone(), 0),
            OrdinalRef::new(alt_row_id.clone(), 0),
        ],
        role: SerialRole::Derived,
        origin: SerialOrigin::Derived {
            technique: "partition-exchange".to_owned(),
        },
        voice: voice("voice/middle"),
        placement: EventPlacement::simultaneous(simult),
        parents: vec![
            SerialEventId::new("event/struct-a").unwrap(),
            SerialEventId::new("event/struct-c").unwrap(),
        ],
    };
    let external = PlannedSerialEvent {
        id: SerialEventId::new("event/external-citation").expect("event id"),
        ordinals: vec![OrdinalRef::new(row_id, 11), OrdinalRef::new(alt_row_id, 0)],
        role: SerialRole::External,
        origin: SerialOrigin::External {
            source: "quote".to_owned(),
        },
        voice: voice("voice/guest"),
        placement: EventPlacement::independent(),
        parents: vec![SerialEventId::new("event/derived-repeat").unwrap()],
    };
    let events = [
        (structural_a.id.clone(), structural_a),
        (structural_b.id.clone(), structural_b),
        (structural_c.id.clone(), structural_c),
        (structural_d.id.clone(), structural_d),
        (derived.id.clone(), derived),
        (external.id.clone(), external),
    ]
    .into_iter()
    .collect();
    SerialPlan::try_new(
        rows,
        events,
        [
            (
                SerialEventId::new("event/struct-a").unwrap(),
                SerialEventId::new("event/derived-repeat").unwrap(),
            ),
            (
                SerialEventId::new("event/struct-b").unwrap(),
                SerialEventId::new("event/derived-repeat").unwrap(),
            ),
            (
                SerialEventId::new("event/struct-c").unwrap(),
                SerialEventId::new("event/derived-repeat").unwrap(),
            ),
            (
                SerialEventId::new("event/struct-d").unwrap(),
                SerialEventId::new("event/external-citation").unwrap(),
            ),
            (
                SerialEventId::new("event/derived-repeat").unwrap(),
                SerialEventId::new("event/external-citation").unwrap(),
            ),
        ],
    )
    .expect("practice plan")
}

fn practice_rules() -> Vec<Arc<dyn crate::PracticeRule>> {
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
        Arc::new(BuiltInPracticeRule::doublings(
            PracticeRuleId::new("rule/doublings").expect("rule id"),
        )),
        Arc::new(BuiltInPracticeRule::simultaneity(
            PracticeRuleId::new("rule/simultaneity").expect("rule id"),
            false,
        )),
        Arc::new(BuiltInPracticeRule::row_mixing(
            PracticeRuleId::new("rule/row-mixing").expect("rule id"),
        )),
        Arc::new(BuiltInPracticeRule::foreign_material(
            PracticeRuleId::new("rule/foreign").expect("rule id"),
            false,
        )),
        Arc::new(BuiltInPracticeRule::parameter_exhaustion(
            PracticeRuleId::new("rule/parameter-exhaustion").expect("rule id"),
        )),
    ]
}

fn strict_context() -> StrictRealizationContext {
    let channel = Channel::new(0).expect("channel");
    let specs = [
        (
            "event/lead-a",
            StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Accent),
        ),
        (
            "event/chord-upper",
            StrictEventSpec {
                pitch_layout: StrictPitchLayout {
                    register: 5,
                    octave_displacements: vec![0, 1],
                },
                ..StrictEventSpec::notes(5, quarter(), 92, channel, Articulation::Tenuto)
            },
        ),
        (
            "event/chord-lower",
            StrictEventSpec {
                pitch_layout: StrictPitchLayout {
                    register: 3,
                    octave_displacements: vec![0, -1],
                },
                ..StrictEventSpec::notes(3, quarter(), 88, channel, Articulation::Marcato)
            },
        ),
        (
            "event/tie-a",
            StrictEventSpec {
                tie: TiePolicy::IntoNext,
                ..StrictEventSpec::notes(4, quarter(), 84, channel, Articulation::Legato)
            },
        ),
        (
            "event/tie-b",
            StrictEventSpec::notes(4, quarter(), 84, channel, Articulation::Legato),
        ),
        (
            "event/cross-low",
            StrictEventSpec::notes(3, quarter(), 90, channel, Articulation::Normal),
        ),
        (
            "event/cross-high",
            StrictEventSpec::notes(5, quarter(), 90, channel, Articulation::Normal),
        ),
        ("event/rest", StrictEventSpec::rest(quarter())),
        (
            "event/unison-a",
            StrictEventSpec::notes(4, quarter(), 78, channel, Articulation::Staccato),
        ),
        (
            "event/unison-b",
            StrictEventSpec::notes(4, quarter(), 78, channel, Articulation::Staccato),
        ),
    ]
    .into_iter()
    .map(|(id, spec)| (SerialEventId::new(id).expect("event id"), spec))
    .collect();
    StrictRealizationContext::new(specs)
}

#[test]
fn strict_realization_preserves_plan_and_serial_origin() {
    let plan = strict_plan();
    let realization = realize_strict(&plan, &strict_context()).expect("realization");
    assert_eq!(realization.plan(), &plan);
    assert!(
        realization.notes().iter().all(|note| {
            plan.event(&note.event_id).is_some() && !note.origin.ordinals.is_empty()
        })
    );
    assert!(realization.events().iter().any(|event| event.is_rest));
}

#[test]
fn strict_realization_renders_chords_crossings_and_equal_pitch_multiplicity() {
    let realization = realize_strict(&strict_plan(), &strict_context()).expect("realization");
    let staff = render_serial_staff(&realization).expect("staff");
    let roll = render_serial_piano_roll(&realization).expect("roll");

    assert_eq!(staff.voices.len(), 3);
    assert!(
        staff
            .voices
            .iter()
            .find(|staff_voice| staff_voice.id == voice("voice/high"))
            .expect("high voice")
            .notes
            .iter()
            .any(|note| note.note.pitch.to_midi() == Some(48))
    );
    assert!(
        staff
            .voices
            .iter()
            .find(|staff_voice| staff_voice.id == voice("voice/low"))
            .expect("low voice")
            .notes
            .iter()
            .any(|note| note.note.pitch.to_midi() == Some(80))
    );

    let tied = realization
        .notes()
        .iter()
        .find(|note| note.event_id == SerialEventId::new("event/tie-a").unwrap())
        .expect("tied note");
    assert_eq!(tied.note.duration, Time::new(1, 2));
    assert!(
        realization
            .notes()
            .iter()
            .all(|note| note.event_id != SerialEventId::new("event/tie-b").unwrap())
    );

    assert!(roll.note_slices().into_iter().any(|slice| {
        slice
            .notes
            .iter()
            .filter(|note| note.timed.note.pitch.to_midi() == Some(71))
            .count()
            == 2
    }));
}

#[test]
fn strict_render_keeps_trailing_rest_duration_for_existing_voice() {
    let row_id = RowInstanceId::new("row/op25/p0").expect("row id");
    let note_id = SerialEventId::new("event/opening").expect("event id");
    let rest_id = SerialEventId::new("event/trailing-rest").expect("event id");
    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), op25_form());
    let events = [
        event(
            "event/opening",
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            "voice/high",
        ),
        event("event/trailing-rest", &[11], "voice/high"),
    ]
    .into_iter()
    .map(|event| (event.id.clone(), event))
    .collect();
    let plan =
        SerialPlan::try_new(rows, events, [(note_id.clone(), rest_id.clone())]).expect("plan");
    let channel = Channel::new(0).expect("channel");
    let context = StrictRealizationContext::new(
        [
            (
                note_id,
                StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Normal),
            ),
            (rest_id, StrictEventSpec::rest(Time::new(3, 4))),
        ]
        .into_iter()
        .collect(),
    );
    let realization = realize_strict(&plan, &context).expect("realization");
    let staff = render_serial_staff(&realization).expect("staff");
    let high_voice = staff
        .voices
        .iter()
        .find(|staff_voice| staff_voice.id == voice("voice/high"))
        .expect("high voice");
    assert_eq!(high_voice.duration, Time::from_integer(1));
}

#[test]
fn strict_realization_wraps_the_canonical_piano_roll_in_a_score() {
    let realization = realize_strict(&strict_plan(), &strict_context()).expect("realization");
    let score = render_serial_score(&realization, &SerialRenderOptions::default()).expect("score");
    assert_eq!(score.tempo_bpm, 60);
    let Score { body, .. } = score;
    let sim_lib_music_core::Music::PianoRoll(roll) = body else {
        panic!("score should render to piano roll");
    };
    assert_eq!(roll.lanes.len(), 3);
    assert!(roll.items.len() >= realization.notes().len());
}

#[test]
fn serial_practice_rules_are_inspectable() {
    let practice = SerialPractice::new(
        PracticeId::new("practice/op25/strict").expect("practice id"),
        practice_rules(),
    );
    let specs = practice.rule_specs();
    assert_eq!(specs.len(), 8);
    assert_eq!(
        specs[0].expected_fact,
        "each structural ordinal appears exactly once"
    );
    assert_eq!(specs[4].parameters[0].name, "allow");
    assert_eq!(specs[4].parameters[0].value, "false");
}

#[test]
fn serial_practice_structural_reading_preserves_strict_statement() {
    let practice = SerialPractice::new(
        PracticeId::new("practice/op25/strict").expect("practice id"),
        practice_rules(),
    );
    let report = practice.evaluate(
        &practice_plan(),
        SerialReading::StructuralPlan,
        &DeclaredWaivers::default(),
    );
    assert!(!report.has_unwaived_violations());
    assert!(report.ledger.entries().iter().all(|entry| {
        matches!(
            entry.status,
            InvariantStatus::Preserved | InvariantStatus::NotApplicable
        )
    }));
}

#[test]
fn serial_practice_all_sounding_makes_relaxations_explicit() {
    let practice = SerialPractice::new(
        PracticeId::new("practice/op25/strict").expect("practice id"),
        practice_rules(),
    );
    let waivers = DeclaredWaivers::new([
        (
            PracticeRuleId::new("rule/simultaneity").expect("rule id"),
            WaiverId::new("waiver/chords").expect("waiver id"),
        ),
        (
            PracticeRuleId::new("rule/parameter-exhaustion").expect("rule id"),
            WaiverId::new("waiver/post-aggregate").expect("waiver id"),
        ),
    ]);
    let report = practice.evaluate(&practice_plan(), SerialReading::AllSounding, &waivers);
    let entries = report.ledger.entries();
    assert!(report.has_unwaived_violations());
    assert!(
        entries
            .iter()
            .any(|entry| matches!(entry.status, InvariantStatus::Relaxed { .. }))
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.rule_id.as_str() == "rule/foreign"
                && matches!(entry.status, InvariantStatus::Violated))
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.rule_id.as_str() == "rule/row-mixing"
                && matches!(entry.status, InvariantStatus::Violated))
    );
}
