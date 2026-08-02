use std::collections::BTreeMap;

use sim_lib_music_core::{Articulation, Channel, Time};
use sim_lib_music_serial::{
    EventPlacement, OrdinalRef, RowInstanceId, SerialEventId, SerialOrigin, SerialPlan, SerialRole,
    StrictEventSpec, StrictRealizationContext, StructuralLicense, StructuralReadingId,
    realize_strict,
};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

use crate::{
    RetrogradeMode, SerialProvenanceStatus, apply_serial_row_operation, quantize_serial,
    remap_serial_voices, retrograde_serial, scale_serial_time, transpose_serial,
};

fn quarter() -> Time {
    Time::new(1, 4)
}

fn serial_fixture() -> sim_lib_music_serial::SerialRealization {
    let row_id = RowInstanceId::new("row/fixture/p0").expect("row id");
    let row = ToneRow::try_from_classes([
        PitchClass::C,
        PitchClass::CS,
        PitchClass::D,
        PitchClass::DS,
        PitchClass::E,
        PitchClass::F,
        PitchClass::FS,
        PitchClass::G,
        PitchClass::GS,
        PitchClass::A,
        PitchClass::AS,
        PitchClass::B,
    ])
    .expect("row")
    .apply(RowOperation::new(RowFamily::P, 0));
    let reading_id = StructuralReadingId::new("reading/fixture").expect("reading");
    let license = StructuralLicense::new(reading_id, "fixture statement").expect("license");
    let group = sim_lib_music_serial::SimultaneousGroupId::new("group/opening").expect("group");
    let event_specs = [
        (
            "event/opening-high",
            0usize,
            "voice/high",
            EventPlacement::simultaneous(group.clone()),
        ),
        (
            "event/opening-low",
            1usize,
            "voice/low",
            EventPlacement::simultaneous(group),
        ),
        (
            "event/closing",
            2usize,
            "voice/high",
            EventPlacement::independent(),
        ),
        (
            "event/fill-3",
            3usize,
            "voice/low",
            EventPlacement::independent(),
        ),
        (
            "event/fill-4",
            4usize,
            "voice/high",
            EventPlacement::independent(),
        ),
        (
            "event/fill-5",
            5usize,
            "voice/low",
            EventPlacement::independent(),
        ),
        (
            "event/fill-6",
            6usize,
            "voice/high",
            EventPlacement::independent(),
        ),
        (
            "event/fill-7",
            7usize,
            "voice/low",
            EventPlacement::independent(),
        ),
        (
            "event/fill-8",
            8usize,
            "voice/high",
            EventPlacement::independent(),
        ),
        (
            "event/fill-9",
            9usize,
            "voice/low",
            EventPlacement::independent(),
        ),
        (
            "event/fill-10",
            10usize,
            "voice/high",
            EventPlacement::independent(),
        ),
        (
            "event/fill-11",
            11usize,
            "voice/low",
            EventPlacement::independent(),
        ),
    ];
    let events = event_specs
        .into_iter()
        .map(|(id, ordinal, voice, placement)| {
            let event_id = SerialEventId::new(id).expect("event id");
            (
                event_id.clone(),
                sim_lib_music_serial::PlannedSerialEvent {
                    id: event_id,
                    ordinals: vec![OrdinalRef::new(row_id.clone(), ordinal)],
                    role: SerialRole::Structural,
                    origin: SerialOrigin::Structural {
                        rationale: "fixture".to_owned(),
                    },
                    voice: sim_lib_music_core::ObjectId::new(voice).expect("voice id"),
                    placement,
                    parents: Vec::new(),
                    licenses: vec![license.clone()],
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let precedence = [
        ("event/opening-high", "event/closing"),
        ("event/closing", "event/fill-3"),
        ("event/fill-3", "event/fill-4"),
        ("event/fill-4", "event/fill-5"),
        ("event/fill-5", "event/fill-6"),
        ("event/fill-6", "event/fill-7"),
        ("event/fill-7", "event/fill-8"),
        ("event/fill-8", "event/fill-9"),
        ("event/fill-9", "event/fill-10"),
        ("event/fill-10", "event/fill-11"),
    ]
    .into_iter()
    .map(|(from, to)| {
        (
            SerialEventId::new(from).expect("from"),
            SerialEventId::new(to).expect("to"),
        )
    });
    let plan = SerialPlan::try_new([(row_id, row)].into_iter().collect(), events, precedence)
        .expect("plan");
    let channel = Channel::new(0).expect("channel");
    let context = StrictRealizationContext::new(
        [
            ("event/opening-high", 4i8),
            ("event/opening-low", 3i8),
            ("event/closing", 4i8),
            ("event/fill-3", 3i8),
            ("event/fill-4", 4i8),
            ("event/fill-5", 3i8),
            ("event/fill-6", 4i8),
            ("event/fill-7", 3i8),
            ("event/fill-8", 4i8),
            ("event/fill-9", 3i8),
            ("event/fill-10", 4i8),
            ("event/fill-11", 3i8),
        ]
        .into_iter()
        .map(|(id, register)| {
            (
                SerialEventId::new(id).expect("event id"),
                StrictEventSpec::notes(register, quarter(), 96, channel, Articulation::Normal),
            )
        })
        .collect(),
    );
    realize_strict(&plan, &context).expect("realization")
}

#[test]
fn serial_transpose_and_time_scale_preserve_provenance() {
    let realization = serial_fixture();
    let transposed = transpose_serial(&realization, 3).expect("transpose");
    assert!(matches!(
        transposed.provenance.ordinal_order,
        SerialProvenanceStatus::Preserved
    ));
    assert!(matches!(
        transposed.provenance.row_forms,
        SerialProvenanceStatus::Preserved
    ));

    let scaled = scale_serial_time(&realization, Time::new(3, 2)).expect("scale");
    assert_eq!(scaled.staff.duration(), Time::new(33, 8));
    assert!(matches!(
        scaled.provenance.ordinal_order,
        SerialProvenanceStatus::Preserved
    ));
}

#[test]
fn serial_row_operation_and_retrograde_report_explicit_status() {
    let realization = serial_fixture();
    let row = apply_serial_row_operation(&realization, RowOperation::new(RowFamily::RI, 5))
        .expect("row op");
    assert!(matches!(
        row.provenance.ordinal_order,
        SerialProvenanceStatus::Invalidated { .. }
    ));
    assert!(matches!(
        row.provenance.row_forms,
        SerialProvenanceStatus::Preserved
    ));

    let retrograde = retrograde_serial(&realization, RetrogradeMode::Cutout).expect("retrograde");
    assert!(matches!(
        retrograde.provenance.ordinal_order,
        SerialProvenanceStatus::Invalidated { .. }
    ));
    assert!(matches!(
        retrograde.provenance.row_forms,
        SerialProvenanceStatus::Invalidated { .. }
    ));
}

#[test]
fn serial_quantize_fails_closed_and_voice_remap_invalidates_voice_provenance() {
    let realization = serial_fixture();
    let err = quantize_serial(&realization, Time::new(1, 3)).expect_err("fail closed");
    assert!(err.to_string().contains("exact serial timing"));

    let mapping = [(
        sim_lib_music_core::ObjectId::new("voice/high").unwrap(),
        sim_lib_music_core::ObjectId::new("voice/lead").unwrap(),
    )]
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    let remapped = remap_serial_voices(&realization, &mapping).expect("remap");
    assert!(matches!(
        remapped.provenance.voices,
        SerialProvenanceStatus::Invalidated { .. }
    ));
}
