use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_namer::LabelContext;
use sim_lib_pitch_set::PitchClassMask;
use sim_lib_sound_core::{Frequency, Tone};

use crate::{DissonanceModelDescriptor, DissonanceRegistry, HarmonicEntropy, analyze_chord};

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
