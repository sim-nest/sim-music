use sim_lib_music_core::PitchClass;
use sim_lib_music_serial::{
    ReferentialClaim, ReferentialEmphasis, ReferentialEvidence, ReferentialEvidenceKind,
    ReferentialRequest, analyze_referential_subset,
};
use sim_lib_pitch_chord::ChordTemplate;
use sim_lib_pitch_ratio::{PitchRatio, RatioPolicy};
use sim_lib_pitch_scale::{Key, Mode, Scale};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};
use sim_lib_pitch_set::PitchClassMask;

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

pub fn referential_subsets() -> Result<(), Box<dyn std::error::Error>> {
    let matching = analyze_referential_subset(
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
    )?;
    assert_eq!(matching.claim, ReferentialClaim::EmbeddedSubset);
    assert_eq!(matching.context.evidence_kind, ReferentialEvidenceKind::Mask);

    let emphasis = ReferentialEmphasis {
        register_focus: Some(5),
        rhythm_profile: Some("long-short-short".to_owned()),
        dynamic_profile: Some("sforzando".to_owned()),
        timbral_profile: Some("muted-brass".to_owned()),
        harmonic_profile: Some("pedal-shadow".to_owned()),
    };
    let chord_report = analyze_referential_subset(
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
    )?;
    assert_eq!(chord_report.emphasis, emphasis);

    let key_report = analyze_referential_subset(
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
    )?;
    assert_eq!(key_report.context.harmonic_claim.as_deref(), Some("vi"));

    let ratio_report = analyze_referential_subset(
        &op25_form(),
        ReferentialRequest {
            ordinals: vec![0, 1, 2],
            evidence: ReferentialEvidence::PitchRatio {
                label: "ratio-trichord".to_owned(),
                ratios: vec![
                    PitchRatio::new(5, 4)?,
                    PitchRatio::new(3, 2)?,
                    PitchRatio::new(15, 8)?,
                ],
                policy: RatioPolicy::five_limit(),
            },
            emphasis: ReferentialEmphasis::default(),
        },
    )?;
    assert_eq!(ratio_report.claim, ReferentialClaim::EmbeddedSubset);

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
    )?;
    assert_eq!(scale_report.context.scale_degrees, vec![1, 3, 5]);
    Ok(())
}
