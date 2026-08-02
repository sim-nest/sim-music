use std::collections::BTreeMap;

use sim_lib_music_core::ObjectId;
use sim_lib_music_serial::{
    EventPlacement, OrdinalRef, PlannedSerialEvent, RowInstanceId, SerialEventId, SerialOrigin,
    SerialPlan, SerialRole, SimultaneousGroupId, StructuralLicense, StructuralReadingId,
};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

pub fn immutable_plan() -> Result<(), Box<dyn std::error::Error>> {
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
    ])?;
    let row = row.apply(RowOperation::new(RowFamily::P, 0));
    let row_id = RowInstanceId::new("row/op25/p0")?;
    let chord_group = SimultaneousGroupId::new("simul/opening")?;
    let license = StructuralLicense::new(
        StructuralReadingId::new("reading/recipe")?,
        "immutable plan recipe reading",
    )?;

    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), row);

    let source_events = [
        PlannedSerialEvent {
            id: SerialEventId::new("event/opening-upper")?,
            ordinals: vec![
                OrdinalRef::new(row_id.clone(), 0),
                OrdinalRef::new(row_id.clone(), 4),
            ],
            role: SerialRole::Structural,
            origin: SerialOrigin::Structural {
                rationale: "opening sonority".to_owned(),
            },
            voice: ObjectId::new("voice/soprano")?,
            placement: EventPlacement::simultaneous(chord_group.clone()),
            parents: vec![],
            licenses: vec![license.clone()],
        },
        PlannedSerialEvent {
            id: SerialEventId::new("event/opening-lower")?,
            ordinals: vec![OrdinalRef::new(row_id.clone(), 7)],
            role: SerialRole::Structural,
            origin: SerialOrigin::Structural {
                rationale: "opening support".to_owned(),
            },
            voice: ObjectId::new("voice/alto")?,
            placement: EventPlacement::simultaneous(chord_group),
            parents: vec![],
            licenses: vec![license.clone()],
        },
        PlannedSerialEvent {
            id: SerialEventId::new("event/middle")?,
            ordinals: vec![
                OrdinalRef::new(row_id.clone(), 1),
                OrdinalRef::new(row_id.clone(), 2),
                OrdinalRef::new(row_id.clone(), 3),
                OrdinalRef::new(row_id.clone(), 5),
                OrdinalRef::new(row_id.clone(), 6),
                OrdinalRef::new(row_id.clone(), 8),
                OrdinalRef::new(row_id.clone(), 9),
            ],
            role: SerialRole::Structural,
            origin: SerialOrigin::Structural {
                rationale: "continuation".to_owned(),
            },
            voice: ObjectId::new("voice/tenor")?,
            placement: EventPlacement::independent(),
            parents: vec![],
            licenses: vec![license.clone()],
        },
        PlannedSerialEvent {
            id: SerialEventId::new("event/close")?,
            ordinals: vec![
                OrdinalRef::new(row_id.clone(), 10),
                OrdinalRef::new(row_id, 11),
            ],
            role: SerialRole::Structural,
            origin: SerialOrigin::Structural {
                rationale: "close".to_owned(),
            },
            voice: ObjectId::new("voice/bass")?,
            placement: EventPlacement::independent(),
            parents: vec![],
            licenses: vec![license],
        },
    ];

    let plan = SerialPlan::try_new(
        rows,
        source_events
            .into_iter()
            .map(|event| (event.id.clone(), event))
            .collect(),
        [
            (
                SerialEventId::new("event/opening-upper")?,
                SerialEventId::new("event/middle")?,
            ),
            (
                SerialEventId::new("event/opening-lower")?,
                SerialEventId::new("event/middle")?,
            ),
            (
                SerialEventId::new("event/middle")?,
                SerialEventId::new("event/close")?,
            ),
        ],
    )?;

    assert_eq!(plan.rows().len(), 1);
    assert_eq!(plan.events().len(), 4);
    assert_eq!(plan.simultaneous_groups().len(), 1);
    Ok(())
}
