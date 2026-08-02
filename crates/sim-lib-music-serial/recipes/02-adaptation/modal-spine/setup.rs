use std::collections::BTreeMap;

use sim_lib_music_core::{Articulation, Channel, ObjectId, PitchClass, Time};
use sim_lib_music_serial::{
    EventPlacement, OrdinalRef, PlannedSerialEvent, RowInstanceId, SerialEventId, SerialOrigin,
    SerialPlan, SerialRole, SerialSpineKind, StrictEventSpec, StrictRealizationContext,
    StructuralLicense, StructuralReadingId, default_realizer_registry,
};
use sim_lib_pitch_scale::{PlayerScale, Scale};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

pub fn modal_spine() -> Result<(), Box<dyn std::error::Error>> {
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
    ])?
    .apply(RowOperation::new(RowFamily::P, 0));
    let row_id = RowInstanceId::new("row/op25/p0")?;
    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), row);
    let license = StructuralLicense::new(
        StructuralReadingId::new("reading/modal-recipe")?,
        "modal adaptation recipe reading",
    )?;
    let event = |id: &str, ordinals: &[usize], voice: &str| -> Result<PlannedSerialEvent, Box<dyn std::error::Error>> {
        Ok(PlannedSerialEvent {
            id: SerialEventId::new(id)?,
            ordinals: ordinals
                .iter()
                .copied()
                .map(|ordinal| OrdinalRef::new(row_id.clone(), ordinal))
                .collect(),
            role: SerialRole::Structural,
            origin: SerialOrigin::Structural {
                rationale: "modal spine statement".to_owned(),
            },
            voice: ObjectId::new(voice)?,
            placement: EventPlacement::independent(),
            parents: vec![],
            licenses: vec![license.clone()],
        })
    };
    let events = [
        event("event/a", &[0, 1], "voice/high")?,
        event("event/b", &[2, 3], "voice/low")?,
        event("event/c", &[4], "voice/high")?,
        event("event/d", &[5], "voice/high")?,
        event("event/e", &[6], "voice/low")?,
        event("event/f", &[7, 8], "voice/high")?,
        event("event/g", &[9], "voice/low")?,
        event("event/h", &[10, 11], "voice/high")?,
    ];
    let plan = SerialPlan::try_new(
        rows,
        events
            .into_iter()
            .map(|event| (event.id.clone(), event))
            .collect(),
        [
            (SerialEventId::new("event/a")?, SerialEventId::new("event/b")?),
            (SerialEventId::new("event/b")?, SerialEventId::new("event/c")?),
            (SerialEventId::new("event/c")?, SerialEventId::new("event/d")?),
            (SerialEventId::new("event/d")?, SerialEventId::new("event/e")?),
            (SerialEventId::new("event/e")?, SerialEventId::new("event/f")?),
            (SerialEventId::new("event/f")?, SerialEventId::new("event/g")?),
            (SerialEventId::new("event/g")?, SerialEventId::new("event/h")?),
        ],
    )?;
    let channel = Channel::new(0)?;
    let quarter = Time::new(1, 4);
    let specs = [
        ("event/a", StrictEventSpec::notes(4, quarter, 96, channel, Articulation::Accent)),
        ("event/b", StrictEventSpec::notes(4, quarter, 92, channel, Articulation::Tenuto)),
        ("event/c", StrictEventSpec::notes(4, quarter, 88, channel, Articulation::Marcato)),
        ("event/d", StrictEventSpec::notes(4, quarter, 84, channel, Articulation::Legato)),
        ("event/e", StrictEventSpec::notes(4, quarter, 84, channel, Articulation::Legato)),
        ("event/f", StrictEventSpec::notes(5, quarter, 90, channel, Articulation::Normal)),
        ("event/g", StrictEventSpec::notes(3, quarter, 90, channel, Articulation::Normal)),
        ("event/h", StrictEventSpec::notes(4, quarter, 78, channel, Articulation::Staccato)),
    ]
    .into_iter()
    .map(|(id, spec)| Ok((SerialEventId::new(id)?, spec)))
    .collect::<Result<BTreeMap<_, _>, Box<dyn std::error::Error>>>()?;
    let strict_context = StrictRealizationContext::new(specs);
    let mut dorian_context = strict_context.clone();
    dorian_context.modal_scale = Some(PlayerScale::from_scale(Scale::dorian(PitchClass::C)));
    let mut lydian_context = strict_context.clone();
    lydian_context.modal_scale = Some(PlayerScale::from_scale(Scale::lydian(PitchClass::C)));
    let mut custom_context = strict_context;
    custom_context.modal_scale = Some(PlayerScale::custom(PitchClass::C, vec![0, 1, 4, 5, 7, 8, 10])?);

    let registry = default_realizer_registry();
    let dorian = registry.realize_named("realizer/modal-degree-cycle", &plan, &dorian_context)?;
    let lydian = registry.realize_named("realizer/modal-degree-cycle", &plan, &lydian_context)?;
    let custom = registry.realize_named(
        "realizer/modal-marked-chromatic-inflection",
        &plan,
        &custom_context,
    )?;

    assert_eq!(dorian.plan(), lydian.plan());
    assert_ne!(dorian.sounding_pitches(), lydian.sounding_pitches());
    assert!(dorian.ledger().is_preserved("serial/ordinal-order"));
    assert!(dorian.ledger().is_relaxed("serial/chromatic-aggregate"));

    let dorian_report = dorian.spine_report().expect("dorian spine report");
    assert_eq!(dorian_report.kind, SerialSpineKind::DegreeCycle);
    assert!(!dorian_report.chromatic_aggregate_identity().preserved);
    assert!(!dorian_report.sonance_context().is_empty());

    let custom_report = custom.spine_report().expect("custom spine report");
    assert_eq!(
        custom_report.kind,
        SerialSpineKind::MarkedChromaticInflection
    );
    assert!(!custom_report.pitch_changes.is_empty());
    Ok(())
}
