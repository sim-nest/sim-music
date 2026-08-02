use std::collections::BTreeMap;

use sim_lib_music_core::{Articulation, Channel, ObjectId, PitchClass, Time};
use sim_lib_music_serial::{
    EventPlacement, OrdinalRef, PlannedSerialEvent, RowInstanceId, SerialEventId, SerialOrigin,
    SerialPlan, SerialRole, SimultaneousGroupId, StrictEventSpec, StrictPitchLayout,
    StrictRealizationContext, StructuralLicense, StructuralReadingId, TiePolicy, realize_strict,
    render_serial_piano_roll,
};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

pub fn strict_row_realization() -> Result<(), Box<dyn std::error::Error>> {
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
    let chord = SimultaneousGroupId::new("simul/chord")?;
    let unison = SimultaneousGroupId::new("simul/unison")?;

    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), row);
    let license = StructuralLicense::new(
        StructuralReadingId::new("reading/recipe")?,
        "strict recipe reading",
    )?;

    let event = |id: &str,
                 ordinals: &[usize],
                 voice: &str,
                 placement: EventPlacement|
     -> Result<PlannedSerialEvent, Box<dyn std::error::Error>> {
        Ok(PlannedSerialEvent {
            id: SerialEventId::new(id)?,
            ordinals: ordinals
                .iter()
                .copied()
                .map(|ordinal| OrdinalRef::new(row_id.clone(), ordinal))
                .collect(),
            role: SerialRole::Structural,
            origin: SerialOrigin::Structural {
                rationale: "strict row statement".to_owned(),
            },
            voice: ObjectId::new(voice)?,
            placement,
            parents: vec![],
            licenses: vec![license.clone()],
        })
    };

    let events = [
        event(
            "event/chord-high",
            &[0, 1],
            "voice/high",
            EventPlacement::simultaneous(chord.clone()),
        )?,
        event(
            "event/chord-low",
            &[2],
            "voice/low",
            EventPlacement::simultaneous(chord),
        )?,
        event("event/tie-a", &[3], "voice/high", EventPlacement::independent())?,
        event("event/tie-b", &[3], "voice/high", EventPlacement::independent())?,
        event(
            "event/rest",
            &[4, 5, 6, 7, 8, 9, 10],
            "voice/low",
            EventPlacement::independent(),
        )?,
        event(
            "event/unison-high",
            &[11],
            "voice/high",
            EventPlacement::simultaneous(unison.clone()),
        )?,
        event(
            "event/unison-low",
            &[11],
            "voice/low",
            EventPlacement::simultaneous(unison),
        )?,
    ];
    let plan = SerialPlan::try_new(
        rows,
        events
            .into_iter()
            .map(|event| (event.id.clone(), event))
            .collect(),
        [
            (
                SerialEventId::new("event/chord-high")?,
                SerialEventId::new("event/tie-a")?,
            ),
            (
                SerialEventId::new("event/chord-low")?,
                SerialEventId::new("event/tie-a")?,
            ),
            (
                SerialEventId::new("event/tie-a")?,
                SerialEventId::new("event/tie-b")?,
            ),
            (
                SerialEventId::new("event/tie-b")?,
                SerialEventId::new("event/rest")?,
            ),
            (
                SerialEventId::new("event/rest")?,
                SerialEventId::new("event/unison-high")?,
            ),
            (
                SerialEventId::new("event/rest")?,
                SerialEventId::new("event/unison-low")?,
            ),
        ],
    )?;

    let channel = Channel::new(0)?;
    let quarter = Time::new(1, 4);
    let specs = [
        (
            "event/chord-high",
            StrictEventSpec {
                pitch_layout: StrictPitchLayout {
                    register: 5,
                    octave_displacements: vec![0, 1],
                },
                ..StrictEventSpec::notes(5, quarter, 92, channel, Articulation::Tenuto)
            },
        ),
        (
            "event/chord-low",
            StrictEventSpec::notes(3, quarter, 88, channel, Articulation::Marcato),
        ),
        (
            "event/tie-a",
            StrictEventSpec {
                tie: TiePolicy::IntoNext,
                ..StrictEventSpec::notes(4, quarter, 84, channel, Articulation::Legato)
            },
        ),
        (
            "event/tie-b",
            StrictEventSpec::notes(4, quarter, 84, channel, Articulation::Legato),
        ),
        ("event/rest", StrictEventSpec::rest(quarter)),
        (
            "event/unison-high",
            StrictEventSpec::notes(4, quarter, 78, channel, Articulation::Staccato),
        ),
        (
            "event/unison-low",
            StrictEventSpec::notes(4, quarter, 78, channel, Articulation::Staccato),
        ),
    ]
    .into_iter()
    .map(|(id, spec)| Ok((SerialEventId::new(id)?, spec)))
    .collect::<Result<BTreeMap<_, _>, Box<dyn std::error::Error>>>()?;

    let realization = realize_strict(&plan, &StrictRealizationContext::new(specs))?;
    assert_eq!(realization.plan(), &plan);
    assert!(
        realization
            .notes()
            .iter()
            .all(|note| plan.event(&note.event_id).is_some() && !note.origin.ordinals.is_empty())
    );
    let rest_id = SerialEventId::new("event/rest")?;
    assert!(
        realization
            .events()
            .iter()
            .any(|event| event.event_id == rest_id && event.is_rest)
    );

    let tied = realization
        .notes()
        .iter()
        .find(|note| note.event_id == SerialEventId::new("event/tie-a").expect("event id"))
        .expect("tied source note");
    assert_eq!(tied.note.duration, Time::new(1, 2));

    let roll = render_serial_piano_roll(&realization)?;
    assert!(roll.note_slices().into_iter().any(|slice| {
        slice
            .notes
            .iter()
            .filter(|note| note.timed.note.pitch.to_midi() == Some(70))
            .count()
            == 2
    }));
    Ok(())
}
