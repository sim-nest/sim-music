use sim_lib_numbers_stats::{CorpusProvenance, MarkovPolicy};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_scale::{Key, Mode};
use sim_lib_pitch_set::PitchClassMask;

use crate::harmony_rule_expr::{rule_set_from_expr, rule_set_to_expr};
use crate::{
    ChordTemplate, CoreHarmonyMetricResolver, CountRange, HarmonyConstraint, HarmonyCorpusSequence,
    HarmonyEvaluationContext, HarmonyMetric, HarmonyPredicate, HarmonyRuleSet,
    LearnedTransitionResolver, Weighted, evaluate_harmony, fit_harmony_markov,
};

// conformance: music states adapt the generic transparent Markov estimator.

const FIXTURE: &[u8] = include_bytes!("../fixtures/generated-harmony-transitions.tsv");

fn key() -> Key {
    Key {
        tonic: PitchClass::C,
        mode: Mode::Major,
    }
}

fn chord(id: &str, classes: &[PitchClass]) -> ChordTemplate {
    ChordTemplate::from_pitch_classes(id, classes.to_vec(), 4)
}

fn c() -> ChordTemplate {
    chord("c", &[PitchClass::C, PitchClass::E, PitchClass::G])
}

fn f() -> ChordTemplate {
    chord("f", &[PitchClass::F, PitchClass::A, PitchClass::C])
}

fn g() -> ChordTemplate {
    chord("g", &[PitchClass::G, PitchClass::B, PitchClass::D])
}

fn corpus() -> Vec<HarmonyCorpusSequence> {
    vec![
        HarmonyCorpusSequence::new(key(), vec![c(), f(), g(), c()]),
        HarmonyCorpusSequence::new(key(), vec![c(), g(), c(), f(), g(), c()]),
        HarmonyCorpusSequence::new(key(), vec![f(), g(), c(), g(), c()]),
    ]
}

fn report() -> sim_lib_numbers_stats::ModelReport<
    sim_lib_numbers_stats::MarkovModel<crate::HarmonyTransitionState>,
> {
    let provenance = CorpusProvenance::from_bytes(
        "generated-functional-harmony-v1",
        "deterministic synthetic I-IV-V chord-state fixture",
        "CC0-1.0",
        FIXTURE,
    )
    .unwrap();
    assert_eq!(provenance.content_hash, "fnv1a64:2a739276a9d8c13c");
    fit_harmony_markov(&corpus(), MarkovPolicy::new(0.5, 1, provenance).unwrap()).unwrap()
}

#[test]
fn chord_key_adapter_has_reproducible_held_out_evidence() {
    let report = report();
    let held_out = report.held_out_score.unwrap();
    assert_eq!(report.training_sequences, 2);
    assert_eq!(held_out.transitions, 4);
    let held_out_probability = (5.0 / 7.0) * (7.0 / 9.0) * (1.0 / 3.0) * (7.0 / 9.0);
    let expected_perplexity = f64::powf(held_out_probability, -0.25);
    assert!((held_out.perplexity - expected_perplexity).abs() < 1.0e-12);

    let first = report
        .model
        .to_stable_text(crate::HarmonyTransitionState::stable_label)
        .unwrap();
    let second = report
        .model
        .to_stable_text(crate::HarmonyTransitionState::stable_label)
        .unwrap();
    assert_eq!(first, second);
}

#[test]
fn learned_metric_descriptor_round_trips_as_declared_rule_data() {
    let rules = HarmonyRuleSet {
        hard: Vec::new(),
        soft: vec![Weighted::new(
            "learned",
            0.25,
            HarmonyMetric::LearnedTransition {
                model: "functional-v1".to_owned(),
            },
        )],
    };

    assert_eq!(
        rule_set_from_expr(&rule_set_to_expr(&rules)).unwrap(),
        rules
    );
}

#[test]
fn learned_transition_is_only_an_optional_soft_metric() {
    let report = report();
    let progression = vec![c(), g()];
    let resolver = LearnedTransitionResolver::new(
        "functional-v1",
        key(),
        &report.model,
        &CoreHarmonyMetricResolver,
    )
    .unwrap();
    let rules = HarmonyRuleSet {
        hard: vec![HarmonyConstraint::new(
            "impossible-four-notes",
            HarmonyPredicate::DistinctPitchClasses {
                count: CountRange::new(4, 4).unwrap(),
            },
        )],
        soft: vec![
            Weighted::new("declared-common-notes", -1.0, HarmonyMetric::CommonNotes),
            Weighted::new(
                "learned-functional-transition",
                0.25,
                HarmonyMetric::LearnedTransition {
                    model: "functional-v1".to_owned(),
                },
            ),
        ],
    };
    let result = evaluate_harmony(
        &rules,
        HarmonyEvaluationContext::progression(
            &[PitchClassMask::from_pitch_classes(&[PitchClass::C])],
            &progression,
        ),
        &resolver,
    )
    .unwrap();

    assert!(!result.legal);
    assert_eq!(result.hard.len(), 1);
    assert_eq!(result.soft.len(), 2);
    assert!(result.soft[1].value.is_finite());
    assert!(
        result.soft[1]
            .facts
            .iter()
            .any(|fact| fact.contains("license=CC0-1.0"))
    );
}
