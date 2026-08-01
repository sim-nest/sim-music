use std::collections::BTreeMap;

use sim_lib_music_core::ObjectId;
use sim_lib_music_core::PitchClass;
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

use crate::{
    EventPlacement, OrdinalRef, PlannedSerialEvent, RowInstanceId, SerialEventId, SerialOrigin,
    SerialPlan, SerialPlanError, SerialRole, SimultaneousGroupId,
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
