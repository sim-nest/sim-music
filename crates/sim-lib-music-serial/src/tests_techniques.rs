use sim_lib_music_core::{Articulation, Channel, ObjectId, PitchClass, Time};
use sim_lib_pitch_chord::ChordTemplate;
use sim_lib_pitch_ratio::{PitchRatio, RatioPolicy};
use sim_lib_pitch_scale::{Key, Mode, Scale};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};
use sim_lib_pitch_set::PitchClassMask;

use crate::{
    CanonOrchestration, CanonSpec, CanonSymmetryRequirement, CanonVoiceSpec, CyclicOrder,
    CyclicProjectionSpec, InvariantRequirement, ParameterTrackKind, ReferentialClaim,
    ReferentialEmphasis, ReferentialEvidence, ReferentialEvidenceKind, ReferentialRequest,
    RowInstanceId, StructuralLicense, StructuralReadingId, SymmetryRequirement,
    analyze_referential_subset, build_canon, deploy_derived_cells, forms_with_invariant,
    project_cyclic_order,
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

#[test]
fn referential_subset_binds_mask_without_whole_passage_tonality() {
    let report = analyze_referential_subset(
        &op25_form(),
        ReferentialRequest {
            ordinals: vec![0, 1, 2],
            evidence: ReferentialEvidence::Mask {
                label: "op25-opening-trichord".to_owned(),
                mask: PitchClassMask::from_pitch_classes(&[
                    PitchClass::E,
                    PitchClass::F,
                    PitchClass::G,
                ]),
            },
            emphasis: ReferentialEmphasis::default(),
        },
    )
    .expect("referential report");

    assert_eq!(report.claim, ReferentialClaim::EmbeddedSubset);
    assert_eq!(report.ordinals, vec![0, 1, 2]);
    assert_eq!(report.context.evidence_kind, ReferentialEvidenceKind::Mask);
    assert!(!report.claims_whole_passage_tonality);
}

#[test]
fn referential_report_retains_emphasis_without_changing_identity() {
    let emphasis = ReferentialEmphasis {
        register_focus: Some(5),
        rhythm_profile: Some("long-short-short".to_owned()),
        dynamic_profile: Some("sforzando".to_owned()),
        timbral_profile: Some("muted-brass".to_owned()),
        harmonic_profile: Some("pedal-shadow".to_owned()),
    };
    let report = analyze_referential_subset(
        &op25_form(),
        ReferentialRequest {
            ordinals: vec![10, 9, 0],
            evidence: ReferentialEvidence::Chord {
                label: "a-minor".to_owned(),
                chord: ChordTemplate::from_pitch_classes(
                    "a-minor",
                    vec![PitchClass::A, PitchClass::C, PitchClass::E],
                    4,
                ),
            },
            emphasis: emphasis.clone(),
        },
    )
    .expect("referential report");

    assert_eq!(report.claim, ReferentialClaim::EmbeddedSubset);
    assert_eq!(report.ordinals, vec![10, 9, 0]);
    assert_eq!(report.emphasis, emphasis);
    assert!(!report.claims_whole_passage_tonality);
}

#[test]
fn referential_report_tracks_nonmatching_rows_and_label_context_changes() {
    let matching = analyze_referential_subset(
        &op25_form(),
        ReferentialRequest {
            ordinals: vec![10, 9, 0],
            evidence: ReferentialEvidence::KeyRegion {
                label: "c-major/vi".to_owned(),
                key: Key {
                    tonic: PitchClass::C,
                    mode: Mode::Major,
                },
                preferred_root: Some(PitchClass::A),
            },
            emphasis: ReferentialEmphasis::default(),
        },
    )
    .expect("matching report");
    let relabeled = analyze_referential_subset(
        &op25_form(),
        ReferentialRequest {
            ordinals: vec![10, 9, 0],
            evidence: ReferentialEvidence::KeyRegion {
                label: "a-minor/i".to_owned(),
                key: Key {
                    tonic: PitchClass::A,
                    mode: Mode::MinorNatural,
                },
                preferred_root: Some(PitchClass::A),
            },
            emphasis: ReferentialEmphasis::default(),
        },
    )
    .expect("relabeled report");
    let nonmatching = analyze_referential_subset(
        &webern_op24_row().apply(RowOperation::new(RowFamily::P, 0)),
        ReferentialRequest {
            ordinals: vec![10, 9, 0],
            evidence: ReferentialEvidence::Mask {
                label: "a-minor-subset".to_owned(),
                mask: matching.context.subset_mask,
            },
            emphasis: ReferentialEmphasis::default(),
        },
    )
    .expect("nonmatching report");

    assert_eq!(matching.claim, ReferentialClaim::EmbeddedSubset);
    assert_eq!(matching.context.harmonic_claim.as_deref(), Some("vi"));
    assert_eq!(relabeled.context.harmonic_claim.as_deref(), Some("i"));
    assert_eq!(
        matching.context.subset_classes,
        vec![PitchClass::A, PitchClass::C, PitchClass::E]
    );
    assert_eq!(nonmatching.claim, ReferentialClaim::NoReferentialSubset);
}

#[test]
fn cyclic_projection_rotates_orchestration_and_ratio_tracks() {
    let orchestration = project_cyclic_order(
        &CyclicOrder {
            track: ParameterTrackKind::Orchestration,
            values: vec![
                "flute".to_owned(),
                "clarinet".to_owned(),
                "violin".to_owned(),
                "horn".to_owned(),
            ],
        },
        &CyclicProjectionSpec {
            order: vec![0, 2, 3, 1],
            rotation: 1,
        },
    )
    .expect("orchestration projection");
    let ratio_projection = project_cyclic_order(
        &CyclicOrder {
            track: ParameterTrackKind::Rhythm,
            values: vec![
                PitchRatio::new(1, 1).unwrap(),
                PitchRatio::new(3, 2).unwrap(),
                PitchRatio::new(5, 4).unwrap(),
            ],
        },
        &CyclicProjectionSpec {
            order: vec![0, 1, 2],
            rotation: 2,
        },
    )
    .expect("ratio projection");
    let ratio_report = analyze_referential_subset(
        &op25_form(),
        ReferentialRequest {
            ordinals: vec![0, 1, 2],
            evidence: ReferentialEvidence::PitchRatio {
                label: "ratio-trichord".to_owned(),
                ratios: ratio_projection.values.clone(),
                policy: RatioPolicy::five_limit(),
            },
            emphasis: ReferentialEmphasis::default(),
        },
    )
    .expect("ratio referential report");
    let scale_report = analyze_referential_subset(
        &op25_form(),
        ReferentialRequest {
            ordinals: vec![10, 9, 0],
            evidence: ReferentialEvidence::Scale {
                label: "a-natural-minor".to_owned(),
                scale: Scale::aeolian(PitchClass::A),
            },
            emphasis: ReferentialEmphasis::default(),
        },
    )
    .expect("scale report");

    assert_eq!(
        orchestration.values,
        vec!["violin", "horn", "clarinet", "flute"]
    );
    assert_eq!(ratio_projection.values[0], PitchRatio::new(5, 4).unwrap());
    assert_eq!(ratio_report.claim, ReferentialClaim::EmbeddedSubset);
    assert_eq!(
        ratio_report
            .context
            .ratio_summary
            .as_ref()
            .map(|summary| summary.admitted_tones),
        Some(3)
    );
    assert_eq!(scale_report.context.scale_degrees, vec![1, 3, 5]);
}
