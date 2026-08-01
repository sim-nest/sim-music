# sim-lib-music-serial

Immutable serial plans with stable row and event identity, explicit role and
origin provenance, and validated partial temporal order.

`sim-lib-music-serial` keeps the structural source honest before realization.
Rows remain immutable `RowForm` values from `sim-lib-pitch-serial`; events cite
them through stable `OrdinalRef` values rather than flattening them into a fake
score order. Equal-onset chords are expressed through simultaneous groups, while
independent temporal requirements stay in a validated precedence DAG. Roles and
origins stay explicit, and every non-structural event carries parent evidence.

```rust
use std::collections::BTreeMap;

use sim_lib_music_core::ObjectId;
use sim_lib_music_serial::{
    EventPlacement, OrdinalRef, PlannedSerialEvent, RowInstanceId, SerialEventId,
    SerialOrigin, SerialPlan, SerialRole, SimultaneousGroupId,
};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

let row = ToneRow::try_from_classes([
    PitchClass::E, PitchClass::F, PitchClass::G, PitchClass::CS,
    PitchClass::FS, PitchClass::DS, PitchClass::GS, PitchClass::D,
    PitchClass::B, PitchClass::C, PitchClass::A, PitchClass::AS,
])?;
let row_form = row.apply(RowOperation::new(RowFamily::P, 0));
let row_id = RowInstanceId::new("row/op25/p0")?;
let group = SimultaneousGroupId::new("simul/opening")?;

let mut rows = BTreeMap::new();
rows.insert(row_id.clone(), row_form);

let opening = PlannedSerialEvent {
    id: SerialEventId::new("event/opening")?,
    ordinals: vec![
        OrdinalRef::new(row_id.clone(), 0),
        OrdinalRef::new(row_id.clone(), 4),
    ],
    role: SerialRole::Structural,
    origin: SerialOrigin::Structural {
        rationale: "opening statement".to_owned(),
    },
    voice: ObjectId::new("voice/soprano")?,
    placement: EventPlacement::simultaneous(group),
    parents: vec![],
};

let answer = PlannedSerialEvent {
    id: SerialEventId::new("event/answer")?,
    ordinals: vec![
        OrdinalRef::new(row_id.clone(), 1),
        OrdinalRef::new(row_id.clone(), 2),
        OrdinalRef::new(row_id, 3),
    ],
    role: SerialRole::Structural,
    origin: SerialOrigin::Structural {
        rationale: "continuation".to_owned(),
    },
    voice: ObjectId::new("voice/alto")?,
    placement: EventPlacement::independent(),
    parents: vec![],
};

let plan = SerialPlan::try_new(
    rows,
    [opening, answer]
        .into_iter()
        .map(|event| (event.id.clone(), event))
        .collect(),
    [(
        SerialEventId::new("event/opening")?,
        SerialEventId::new("event/answer")?,
    )],
)?;

assert_eq!(plan.simultaneous_groups().len(), 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Construction fails closed when row or event ids are malformed, ordinal
references leave the row, structural coverage is incomplete, parent evidence is
missing or cyclic, or precedence fabricates an order inside one simultaneous
group.
