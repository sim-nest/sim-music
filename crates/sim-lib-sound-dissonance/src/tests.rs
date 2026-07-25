use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use sim_lib_discrete_search::{SearchControl, SearchStatus};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_namer::LabelContext;
use sim_lib_pitch_set::PitchClassMask;
use sim_lib_sound_core::{
    Amplitude, Frequency, Partial, PartialTag, Phase, Tone, default_envelope,
};

use crate::{
    DissonanceInputError, DissonanceModelDescriptor, DissonanceRegistry, HarmonicEntropy,
    ParameterRange, PartialRoughnessGrid, PsychoacousticCurveFamily, SonanceFitObjective,
    SonanceFitStrategy, analyze_chord, fit_partial_roughness_catalog, fit_sonance_model,
    locked_partial_roughness_corpus, partial_pair_roughness, try_analyze_chord,
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

#[test]
fn partial_roughness_catalog_fit_is_bounded_and_reproducible() {
    let first = fit_partial_roughness_catalog().unwrap();
    let second = fit_partial_roughness_catalog().unwrap();

    assert_eq!(first.model, "partial-roughness");
    assert_eq!(first.strategy, "brute-force");
    assert_eq!(first.objective, "rank-correlation");
    assert_eq!(first.receipt.seed, 7);
    assert_eq!(first.receipt.status, SearchStatus::Complete);
    assert_eq!(first.receipt.reason, None);
    assert_eq!(first.candidates.len(), 8);
    assert_eq!(first.digest, second.digest);
    assert_eq!(first.receipt.digest, second.receipt.digest);
    assert!(first.candidates.iter().all(|candidate| {
        candidate.parameters.a >= 2.0
            && candidate.parameters.a <= 5.0
            && candidate.parameters.b >= 4.0
            && candidate.parameters.b <= 12.0
            && candidate.parameters.b > candidate.parameters.a
    }));
    assert!(first.candidates.iter().all(
        |candidate| candidate.training.rank_correlation.is_finite()
            && candidate.validation.residual_variance.is_finite()
            && candidate.locked_conformance.rank_correlation.is_finite()
    ));
}

#[test]
fn fitting_report_keeps_corpus_hashes_and_licenses() {
    let report = fit_partial_roughness_catalog().unwrap();

    assert_eq!(report.corpora.len(), 3);
    assert!(report.corpora.iter().all(|meta| meta.license == "MPL-2.0"));
    assert!(
        report
            .corpora
            .iter()
            .all(|meta| meta.corpus_hash.starts_with("fnv1a64:"))
    );
    assert_eq!(
        report
            .corpora
            .iter()
            .map(|meta| meta.id)
            .collect::<Vec<_>>(),
        vec![
            "partial-roughness-training-v1",
            "partial-roughness-validation-v1",
            "partial-roughness-locked-conformance-v1",
        ]
    );
}

#[test]
fn every_fit_strategy_runs_through_search_control() {
    let grid = PartialRoughnessGrid {
        a: ParameterRange::new(2.0, 2.2, 0.1).unwrap(),
        b: ParameterRange::new(4.0, 4.2, 0.1).unwrap(),
    };
    let control = SearchControl::default()
        .with_max_work(500)
        .with_max_results(3)
        .with_seed(11);

    let reports = [
        SonanceFitStrategy::BruteForce,
        SonanceFitStrategy::Coordinate,
        SonanceFitStrategy::BoundedStochastic,
    ]
    .into_iter()
    .map(|strategy| {
        fit_sonance_model(
            strategy,
            SonanceFitObjective::RankCorrelation,
            grid,
            control.clone(),
            locked_partial_roughness_corpus(),
        )
        .unwrap()
    })
    .collect::<Vec<_>>();

    assert_eq!(reports.len(), 3);
    assert!(reports.iter().all(|report| report.receipt.seed == 11));
    assert!(reports.iter().all(|report| report.candidates.len() <= 3));
    assert!(reports.iter().all(|report| report.receipt.work_used > 0));
    assert_eq!(reports[0].strategy, "brute-force");
    assert_eq!(reports[1].strategy, "coordinate");
    assert_eq!(reports[2].strategy, "bounded-stochastic");
}
