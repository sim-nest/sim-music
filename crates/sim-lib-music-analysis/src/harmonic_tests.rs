use crate::{
    HarmonicDecodePlan, HarmonicDecodeStrategy, HarmonicFeatureFrame, HarmonicTemplate,
    decode_chords, decode_harmonic_sequence, decode_keys,
};

// conformance: music adapters retain generic HMM posterior and alternative evidence.

#[test]
fn declared_templates_decode_through_generic_hmm_with_alternatives() {
    let templates = vec![
        HarmonicTemplate::new("bright", vec![1.0, 0.1, 0.0]).unwrap(),
        HarmonicTemplate::new("dark", vec![0.0, 0.1, 1.0]).unwrap(),
    ];
    let frames = vec![
        HarmonicFeatureFrame {
            at_sample: 0,
            values: vec![1.0, 0.2, 0.0],
        },
        HarmonicFeatureFrame {
            at_sample: 256,
            values: vec![0.8, 0.2, 0.1],
        },
        HarmonicFeatureFrame {
            at_sample: 512,
            values: vec![0.0, 0.2, 1.0],
        },
    ];
    let plan = HarmonicDecodePlan {
        strategy: HarmonicDecodeStrategy::Viterbi,
        stay_probability: 0.7,
        max_alternatives: 2,
        ..HarmonicDecodePlan::default()
    };
    let decoded = decode_harmonic_sequence(&frames, &templates, None, &plan).unwrap();

    assert_eq!(decoded.frames[0].label, "bright");
    assert_eq!(decoded.frames[2].label, "dark");
    assert!(decoded.frames.iter().all(|frame| {
        frame.confidence > 0.0
            && frame.alternatives.len() == 2
            && frame
                .alternatives
                .iter()
                .all(|alternative| alternative.posterior > 0.0)
    }));
    assert!(decoded.evidence.path_log_probability.is_some());
    assert_eq!(decoded.evidence.normalized_steps, frames.len());
    assert!(decoded.evidence.work_used <= decoded.evidence.work_limit);
}

#[test]
fn standard_key_and_chord_templates_name_clear_profiles() {
    let c_major = HarmonicFeatureFrame {
        at_sample: 0,
        values: vec![1.0, 0.0, 0.1, 0.0, 0.9, 0.2, 0.0, 0.8, 0.0, 0.1, 0.0, 0.1],
    };
    let keys = decode_keys(
        &[c_major.clone(), c_major.clone()],
        &HarmonicDecodePlan::default(),
    )
    .unwrap();
    let chords =
        decode_chords(&[c_major.clone(), c_major], &HarmonicDecodePlan::default()).unwrap();

    assert_eq!(keys.frames[0].label, "C major");
    assert_eq!(chords.frames[0].label, "C:maj");
    assert!(keys.frames[0].alternatives.len() > 1);
    assert!(chords.frames[0].alternatives.len() > 1);
}

#[test]
fn harmonic_decode_refuses_unbounded_or_malformed_requests() {
    let frame = HarmonicFeatureFrame {
        at_sample: 0,
        values: vec![1.0, 0.0],
    };
    let template = HarmonicTemplate::new("one", vec![1.0, 0.0]).unwrap();
    let error = decode_harmonic_sequence(
        &[frame],
        &[template],
        None,
        &HarmonicDecodePlan {
            max_work: 1,
            ..HarmonicDecodePlan::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        crate::HarmonicDecodeError::WorkLimit { .. }
    ));
}
