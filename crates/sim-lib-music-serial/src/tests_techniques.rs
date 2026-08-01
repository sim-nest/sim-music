use sim_lib_music_core::{Articulation, Channel, ObjectId, PitchClass, Time};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

use crate::{
    CanonOrchestration, CanonSpec, CanonSymmetryRequirement, CanonVoiceSpec, InvariantRequirement,
    RowInstanceId, StructuralLicense, StructuralReadingId, SymmetryRequirement, build_canon,
    deploy_derived_cells, forms_with_invariant,
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

fn webern_op24_row() -> ToneRow {
    ToneRow::try_from_classes([
        PitchClass::C,
        PitchClass::B,
        PitchClass::DS,
        PitchClass::E,
        PitchClass::GS,
        PitchClass::G,
        PitchClass::A,
        PitchClass::F,
        PitchClass::FS,
        PitchClass::CS,
        PitchClass::D,
        PitchClass::AS,
    ])
    .expect("Webern op. 24 row")
}

fn voice(name: &str) -> ObjectId {
    ObjectId::new(name).expect("voice id")
}

fn quarter() -> Time {
    Time::new(1, 4)
}

fn named_license(id: &str, rationale: &str) -> StructuralLicense {
    StructuralLicense::new(StructuralReadingId::new(id).expect("reading id"), rationale)
        .expect("license")
}

#[test]
fn derived_cell_deployment_retains_webern_generator_and_occurrence_evidence() {
    let row = webern_op24_row().apply(RowOperation::new(RowFamily::P, 0));
    let row_id = RowInstanceId::new("row/webern/op24").expect("row id");
    let deployment = deploy_derived_cells(
        row_id,
        row,
        3,
        vec![
            voice("voice/clarinet"),
            voice("voice/violin"),
            voice("voice/trumpet"),
            voice("voice/piano"),
        ],
        "event/webern/derived",
        "Op. 24 trichordal derivation",
        named_license("reading/webern-derived", "Webern Op. 24 derived cells"),
    )
    .expect("derived deployment");

    assert_eq!(deployment.generator_size, 3);
    assert_eq!(deployment.occurrences.len(), 4);
    assert_eq!(
        deployment.occurrences[0]
            .generator_classes
            .iter()
            .map(|class| class.value())
            .collect::<Vec<_>>(),
        vec![0, 11, 3]
    );
    assert_eq!(deployment.occurrences[1].source_ordinals, vec![3, 4, 5]);
    assert_eq!(
        deployment.occurrences[1].operation,
        RowOperation::new(RowFamily::RI, 7)
    );
    assert_eq!(
        deployment.occurrences[2].operation,
        RowOperation::new(RowFamily::R, 6)
    );
    assert_eq!(
        deployment.occurrences[3].operation,
        RowOperation::new(RowFamily::I, 1)
    );
}

#[test]
fn invariant_form_selection_returns_only_certified_candidates() {
    let row = op25_form().into_row();
    let segment = row.segment(0, 3).expect("segment");
    let requirement = InvariantRequirement {
        preserve_source_ordinals: true,
        preserve_pitch_identity: false,
        require_transposition: true,
        require_inversion: false,
        preserve_interval_order: true,
        preserve_set_class: true,
        symmetry: SymmetryRequirement::None,
    };
    let candidates = forms_with_invariant(&row, &segment, requirement).expect("candidates");

    assert!(candidates.unsatisfied().is_none());
    assert!(!candidates.as_slice().is_empty());
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.certificate.satisfies(&requirement))
    );
}

#[test]
fn retrograde_canon_uses_offsets_and_keeps_timbre_as_realization_metadata() {
    let subject = op25_form();
    let answer = webern_op24_row().apply(RowOperation::new(RowFamily::R, 10));
    let deployment = build_canon(CanonSpec {
        event_prefix: "event/canon/retrograde".to_owned(),
        onset: quarter(),
        rationale: "retrograde canon".to_owned(),
        license: named_license("reading/canon", "retrograde canon reading"),
        requirement: CanonSymmetryRequirement::RetrogradeAnswer,
        voices: vec![
            CanonVoiceSpec {
                row_id: RowInstanceId::new("row/canon/subject").expect("row id"),
                form: subject.clone(),
                voice: voice("voice/subject"),
                voice_offset: Time::from_integer(0),
                register: 4,
                duration: quarter(),
                orchestration: CanonOrchestration {
                    channel: Channel::new(0).expect("channel"),
                    articulation: Articulation::Normal,
                    timbre: Some("clarinet".to_owned()),
                    orchestration: Some("solo".to_owned()),
                },
            },
            CanonVoiceSpec {
                row_id: RowInstanceId::new("row/canon/answer").expect("row id"),
                form: subject.row().apply(RowOperation::new(RowFamily::R, 0)),
                voice: voice("voice/answer"),
                voice_offset: quarter(),
                register: 5,
                duration: quarter(),
                orchestration: CanonOrchestration {
                    channel: Channel::new(1).expect("channel"),
                    articulation: Articulation::Marcato,
                    timbre: Some("muted-trumpet".to_owned()),
                    orchestration: Some("answer".to_owned()),
                },
            },
        ],
    })
    .expect("canon");

    assert!(deployment.symmetry.satisfied);
    assert_eq!(deployment.plan.events().len(), 24);
    assert_eq!(deployment.voices.len(), 2);
    assert_eq!(
        deployment.voices[1].orchestration.timbre.as_deref(),
        Some("muted-trumpet")
    );
    assert_eq!(deployment.realization[0].onset, quarter());
    assert_eq!(deployment.realization[12].onset, Time::new(2, 4));
    assert_eq!(answer.classes().len(), 12);
}

#[test]
fn impossible_invariant_request_returns_explicit_unsatisfied_evidence() {
    let row = op25_form().into_row();
    let segment = row.segment(0, 4).expect("segment");
    let candidates = forms_with_invariant(
        &row,
        &segment,
        InvariantRequirement {
            preserve_source_ordinals: true,
            preserve_pitch_identity: true,
            require_transposition: false,
            require_inversion: false,
            preserve_interval_order: true,
            preserve_set_class: true,
            symmetry: SymmetryRequirement::PitchPalindrome,
        },
    )
    .expect("candidates");

    assert!(candidates.as_slice().is_empty());
    let unsatisfied = candidates.unsatisfied().expect("unsatisfied");
    assert_eq!(unsatisfied.forms_checked, 48);
    assert!(unsatisfied.segments_checked >= 48);
}
