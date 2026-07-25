use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_namer::LabelContext;
use sim_lib_pitch_set::PitchClassMask;
use sim_lib_sound_core::{
    Amplitude, Frequency, Partial, PartialTag, Phase, Tone, default_envelope,
};

use crate::{
    DissonanceInputError, DissonanceModelDescriptor, DissonanceRegistry, HarmonicEntropy,
    PsychoacousticCurveFamily, analyze_chord, partial_pair_roughness, try_analyze_chord,
};

#[test]
fn all_four_models_return_finite_scores() {
    let tones = [
        Tone::sine(Frequency(440.0), Duration::from_secs(1)),
        Tone::sine(Frequency(550.0), Duration::from_secs(1)),
        Tone::sine(Frequency(660.0), Duration::from_secs(1)),
    ];
    let registry = DissonanceRegistry::new_with_builtins();
    let scores = analyze_chord(&tones, &registry);
    assert_eq!(scores.len(), 4);
    assert!(scores.iter().all(|score| score.score.is_finite()));
    assert!(
        scores
            .iter()
            .all(|score| score.sonance.evidence.model == score.model)
    );
    assert!(
        scores
            .iter()
            .any(|score| score.sonance.evidence.curve_family == "plomp-levelt")
    );
}

#[test]
fn pitch_and_sound_registries_are_separate() {
    let sound = DissonanceRegistry::new_with_builtins();
    let pitch = sim_lib_pitch_dissonance::PitchDissonanceRegistry::new_with_builtins();
    let pitch_scores = pitch.analyze_all(
        PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::E, PitchClass::G]),
        &LabelContext::default(),
    );
    let sound_scores = analyze_chord(
        &[Tone::sine(Frequency(440.0), Duration::from_secs(1))],
        &sound,
    );
    assert_ne!(pitch_scores[0].model, sound_scores[0].model);
}

#[test]
fn spectral_models_can_disagree_and_results_preserve_model_names() {
    let tones = [
        Tone::sawtooth(Frequency(220.0), Duration::from_secs(1), 8),
        Tone::square(Frequency(330.0), Duration::from_secs(1), 8),
    ];
    let registry = DissonanceRegistry::new_with_builtins();
    let scores = analyze_chord(&tones, &registry);
    let unique = scores
        .iter()
        .map(|score| format!("{:.6}", score.score))
        .collect::<BTreeSet<_>>();
    assert!(unique.len() > 1);
    assert!(scores.iter().any(|score| score.model == "sethares"));
    assert!(scores.iter().any(|score| score.model == "plomp-levelt"));
}

#[test]
fn descriptor_round_trips_to_model() {
    let model = DissonanceModelDescriptor::HarmonicEntropy { spread: 24.0 }.to_model();
    let tone = Tone::sine(Frequency(440.0), Duration::from_secs(1));
    assert!(model.dissonance_of_tone(&tone).is_finite());
}

#[test]
fn custom_model_registration_overrides_name() {
    let mut registry = DissonanceRegistry::new_with_builtins();
    registry.register(Arc::new(HarmonicEntropy { spread: 12.0 }));
    assert!(registry.get("harmonic-entropy").is_some());
}

#[test]
fn checked_analysis_rejects_non_finite_partials() {
    let tone = Tone {
        partials: vec![Partial {
            frequency: Frequency(f64::NAN),
            amplitude: Amplitude(1.0),
            phase: Phase(0.0),
            tag: PartialTag::Source,
        }],
        envelope: default_envelope(),
        duration: Duration::from_secs(1),
    };
    let registry = DissonanceRegistry::new_with_builtins();

    assert_eq!(
        try_analyze_chord(&[tone], &registry),
        Err(DissonanceInputError::NonFiniteInput)
    );
}

#[test]
fn partial_pair_policy_reports_inaudible_skips() {
    let tone = Tone::from_partials(
        vec![
            Partial::new(Frequency(440.0), Amplitude(1.0), Phase(0.0)).unwrap(),
            Partial::new(Frequency(441.0), Amplitude(0.0), Phase(0.0)).unwrap(),
            Partial::new(Frequency(660.0), Amplitude(0.5), Phase(0.0)).unwrap(),
        ],
        default_envelope(),
        Duration::from_secs(1),
    )
    .unwrap();
    let registry = DissonanceRegistry::new_with_builtins();
    let scores = try_analyze_chord(&[tone], &registry).unwrap();
    let sethares = scores
        .iter()
        .find(|score| score.model == "sethares")
        .unwrap();

    assert_eq!(sethares.sonance.evidence.partial_policy.audible_partials, 2);
    assert_eq!(
        sethares.sonance.evidence.partial_policy.inaudible_partials,
        1
    );
    assert_eq!(sethares.sonance.evidence.partial_policy.evaluated_pairs, 1);
    assert_eq!(
        sethares
            .sonance
            .evidence
            .partial_policy
            .skipped_inaudible_pairs,
        2
    );
}

#[test]
fn curve_families_expose_partial_pair_roughness() {
    let bins = [
        (Frequency(440.0), Amplitude(1.0)),
        (Frequency(445.0), Amplitude(0.5)),
    ];
    for curve in [
        PsychoacousticCurveFamily::PlompLevelt,
        PsychoacousticCurveFamily::Sethares,
        PsychoacousticCurveFamily::HelmholtzBeating,
        PsychoacousticCurveFamily::HarmonicEntropy { spread: 18.0 },
    ] {
        let pairs = partial_pair_roughness(&bins, curve).unwrap();
        assert_eq!(pairs.len(), 1);
        assert!(pairs[0].roughness.is_finite());
    }
}
