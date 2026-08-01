use std::collections::BTreeMap;

use sim_lib_music_core::{ObjectId, PitchClass};
use sim_lib_music_serial::{
    EventPlacement, OrdinalRef, PlannedSerialEvent, RowInstanceId, SerialEventId, SerialOrigin,
    SerialPlan, SerialRole, StructuralLicense, StructuralReadingId,
};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

use crate::{decode_serial_plan, encode_serial_plan};

#[test]
fn serial_plan_round_trips_via_canonical_text() {
    let row_id = RowInstanceId::new("row/op25/p0").expect("row");
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
    .expect("row")
    .apply(RowOperation::new(RowFamily::P, 0));
    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), row);
    let license = StructuralLicense::new(
        StructuralReadingId::new("reading/shape").expect("reading"),
        "shape round-trip reading",
    )
    .expect("license");
    let events = [
        PlannedSerialEvent {
            id: SerialEventId::new("event/a").expect("event"),
            ordinals: vec![
                OrdinalRef::new(row_id.clone(), 0),
                OrdinalRef::new(row_id.clone(), 1),
                OrdinalRef::new(row_id.clone(), 2),
                OrdinalRef::new(row_id.clone(), 3),
                OrdinalRef::new(row_id.clone(), 4),
                OrdinalRef::new(row_id.clone(), 5),
            ],
            role: SerialRole::Structural,
            origin: SerialOrigin::Structural {
                rationale: "a".to_owned(),
            },
            voice: ObjectId::new("voice/a").expect("voice"),
            placement: EventPlacement::independent(),
            parents: vec![],
            licenses: vec![license.clone()],
        },
        PlannedSerialEvent {
            id: SerialEventId::new("event/b").expect("event"),
            ordinals: vec![
                OrdinalRef::new(row_id.clone(), 6),
                OrdinalRef::new(row_id.clone(), 7),
                OrdinalRef::new(row_id.clone(), 8),
                OrdinalRef::new(row_id.clone(), 9),
                OrdinalRef::new(row_id.clone(), 10),
                OrdinalRef::new(row_id, 11),
            ],
            role: SerialRole::Structural,
            origin: SerialOrigin::Structural {
                rationale: "b".to_owned(),
            },
            voice: ObjectId::new("voice/b").expect("voice"),
            placement: EventPlacement::independent(),
            parents: vec![],
            licenses: vec![license],
        },
    ]
    .into_iter()
    .map(|event| (event.id.clone(), event))
    .collect();
    let plan = SerialPlan::try_new(
        rows,
        events,
        [(
            SerialEventId::new("event/a").expect("a"),
            SerialEventId::new("event/b").expect("b"),
        )],
    )
    .expect("plan");

    let encoded = encode_serial_plan(&plan).expect("encode");
    let decoded = decode_serial_plan(&encoded).expect("decode");
    assert_eq!(decoded, plan);
}
