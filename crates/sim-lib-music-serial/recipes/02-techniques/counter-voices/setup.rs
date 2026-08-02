use sim_lib_music_core::{Articulation, Channel, ObjectId, PitchClass, Time};
use sim_lib_music_serial::{
    CanonOrchestration, CanonSpec, CanonSymmetryRequirement, CanonVoiceSpec, RowInstanceId,
    StructuralLicense, StructuralReadingId, build_canon,
};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

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

pub fn counter_voices() -> Result<(), Box<dyn std::error::Error>> {
    let subject = op25_form();
    let deployment = build_canon(CanonSpec {
        event_prefix: "event/canon/retrograde".to_owned(),
        onset: Time::new(1, 4),
        rationale: "retrograde canon".to_owned(),
        license: StructuralLicense::new(
            StructuralReadingId::new("reading/canon")?,
            "retrograde canon reading",
        )?,
        requirement: CanonSymmetryRequirement::RetrogradeAnswer,
        voices: vec![
            CanonVoiceSpec {
                row_id: RowInstanceId::new("row/canon/subject")?,
                form: subject.clone(),
                voice: ObjectId::new("voice/subject")?,
                voice_offset: Time::from_integer(0),
                register: 4,
                duration: Time::new(1, 4),
                orchestration: CanonOrchestration {
                    channel: Channel::new(0)?,
                    articulation: Articulation::Normal,
                    timbre: Some("clarinet".to_owned()),
                    orchestration: Some("solo".to_owned()),
                },
            },
            CanonVoiceSpec {
                row_id: RowInstanceId::new("row/canon/answer")?,
                form: subject.row().apply(RowOperation::new(RowFamily::R, 0)),
                voice: ObjectId::new("voice/answer")?,
                voice_offset: Time::new(1, 4),
                register: 5,
                duration: Time::new(1, 4),
                orchestration: CanonOrchestration {
                    channel: Channel::new(1)?,
                    articulation: Articulation::Marcato,
                    timbre: Some("muted-trumpet".to_owned()),
                    orchestration: Some("answer".to_owned()),
                },
            },
        ],
    })?;
    assert!(deployment.symmetry.satisfied);
    assert_eq!(deployment.plan.events().len(), 24);
    assert_eq!(
        deployment.voices[1].orchestration.timbre.as_deref(),
        Some("muted-trumpet")
    );
    assert_eq!(deployment.realization[12].onset, Time::new(2, 4));
    Ok(())
}
