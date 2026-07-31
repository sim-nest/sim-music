// conformance: bounded local and global planners share rules, ties, optima, and receipts.

use sim_lib_discrete_search::{NeverInterrupt, SearchControl, SearchStatus};
use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_pitch_set::PitchClassMask;

use crate::{
    ChordPalette, ChordTemplate, CoreHarmonyMetricResolver, CountRange, HarmonizationRequest,
    HarmonizationStrategy, HarmonyConstraint, HarmonyHeuristic, HarmonyMetric, HarmonyPredicate,
    HarmonyRuleSet, Weighted, plan_harmony,
};

fn chord(id: &str, midi: &[u8]) -> ChordTemplate {
    ChordTemplate::from_pitches(id, midi.iter().copied().map(Pitch::from_midi).collect())
}

fn note(class: PitchClass) -> PitchClassMask {
    PitchClassMask::from_pitch_classes(&[class])
}

fn fixture(length: usize) -> HarmonizationRequest {
    let palette = ChordPalette::explicit(
        "planner-palette",
        vec![
            chord("c", &[48, 52, 55, 60]),
            chord("f", &[53, 57, 60, 65]),
            chord("g7", &[55, 59, 62, 65]),
            chord("am", &[57, 60, 64, 69]),
        ],
        Vec::new(),
    )
    .unwrap();
    let phrase = [PitchClass::C, PitchClass::A, PitchClass::G, PitchClass::C];
    let melody = (0..length)
        .map(|index| note(phrase[index % phrase.len()]))
        .collect();
    HarmonizationRequest {
        melody,
        palette,
        rules: HarmonyRuleSet {
            hard: vec![
                HarmonyConstraint::new("melody-fit", HarmonyPredicate::MelodyInChord),
                HarmonyConstraint::new(
                    "common-tone",
                    HarmonyPredicate::CommonNotes {
                        count: CountRange::new(1, 3).unwrap(),
                    },
                ),
                HarmonyConstraint::new(
                    "register",
                    HarmonyPredicate::PitchRange {
                        min_midi: 48,
                        max_midi: 72,
                    },
                ),
                HarmonyConstraint::new(
                    "repetition",
                    HarmonyPredicate::MinimumChordDistance { distance: 1 },
                ),
                HarmonyConstraint::new(
                    "cadence",
                    HarmonyPredicate::ChordAt {
                        position: -1,
                        chord: PitchClassMask::from_pitch_classes(&[
                            PitchClass::C,
                            PitchClass::E,
                            PitchClass::G,
                        ]),
                    },
                ),
            ],
            soft: vec![
                Weighted::new("voice-leading", 1.0, HarmonyMetric::VoiceLeading),
                Weighted::new("common-reward", -2.0, HarmonyMetric::CommonNotes),
            ],
        },
    }
}

#[test]
fn recursive_factored_and_layered_share_the_optimal_small_problem() {
    let request = fixture(4);
    let exhaustive = plan_harmony(
        &request,
        HarmonizationStrategy::RecursiveExhaustive,
        SearchControl::default().with_max_work(100_000),
        &NeverInterrupt,
        &CoreHarmonyMetricResolver,
    )
    .unwrap();
    let factored = plan_harmony(
        &request,
        HarmonizationStrategy::FactoredBacktracking,
        SearchControl::default().with_max_work(100_000),
        &NeverInterrupt,
        &CoreHarmonyMetricResolver,
    )
    .unwrap();
    let layered = plan_harmony(
        &request,
        HarmonizationStrategy::LayeredDp,
        SearchControl::default()
            .with_max_work(1_000_000)
            .with_max_memory_nodes(10_000),
        &NeverInterrupt,
        &CoreHarmonyMetricResolver,
    )
    .unwrap();
    let beam = plan_harmony(
        &request,
        HarmonizationStrategy::Beam {
            width: 512,
            heuristic: HarmonyHeuristic {
                id: "common-tone-floor".to_owned(),
                lower_bound_per_remaining_micros: -8_000_000,
                admissible: true,
            },
        },
        SearchControl::default()
            .with_max_work(1_000_000)
            .with_max_memory_nodes(10_000)
            .with_branch_and_bound(true),
        &NeverInterrupt,
        &CoreHarmonyMetricResolver,
    )
    .unwrap();

    assert_eq!(exhaustive.receipt.status, SearchStatus::Complete);
    assert_eq!(factored.receipt.status, SearchStatus::Complete);
    assert_eq!(beam.receipt.status, SearchStatus::Complete);
    assert_eq!(
        exhaustive
            .results
            .iter()
            .map(|result| &result.palette_indices)
            .collect::<Vec<_>>(),
        factored
            .results
            .iter()
            .map(|result| &result.palette_indices)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        factored.results[0].score_micros,
        exhaustive.results[0].score_micros
    );
    assert_eq!(factored.receipt.work_used, exhaustive.receipt.work_used);
    assert_eq!(
        layered.results[0].score_micros,
        exhaustive.results[0].score_micros
    );
    assert_eq!(
        layered.results[0].palette_indices,
        exhaustive.results[0].palette_indices
    );
    assert_eq!(
        beam.results[0].score_micros,
        exhaustive.results[0].score_micros
    );
    assert_eq!(
        beam.results[0].palette_indices,
        exhaustive.results[0].palette_indices
    );
    assert!(exhaustive.receipt.work_used > 0);
    assert!(factored.receipt.work_used > 0);
    assert!(layered.receipt.work_used > 0);
    assert!(beam.receipt.work_used > 0);
    assert!(exhaustive.receipt.evaluated_candidates > 0);
    assert!(factored.receipt.evaluated_candidates > 0);
    assert!(layered.receipt.evaluated_candidates > 0);
    assert!(beam.receipt.evaluated_candidates > 0);
    assert!(layered.receipt.optimal);
    assert!(layered.receipt.optimality.is_some());
    assert!(!beam.receipt.optimal);
    assert!(matches!(
        beam.receipt.strategy,
        HarmonizationStrategy::Beam {
            heuristic: HarmonyHeuristic {
                admissible: true,
                ..
            },
            ..
        }
    ));
    assert!(
        layered
            .receipt
            .rejections
            .iter()
            .any(|rejection| rejection.rule_id == "melody-fit")
    );
}

#[test]
fn beam_is_deterministic_and_large_bounded_runs_keep_partial_receipts() {
    let request = fixture(24);
    let strategy = HarmonizationStrategy::Beam {
        width: 8,
        heuristic: HarmonyHeuristic::zero("declared-zero", false),
    };
    let control = SearchControl::default()
        .with_seed(12)
        .with_max_work(100_000)
        .with_max_results(1)
        .with_max_frontier(128)
        .with_max_memory_nodes(256);
    let first = plan_harmony(
        &request,
        strategy.clone(),
        control.clone(),
        &NeverInterrupt,
        &CoreHarmonyMetricResolver,
    )
    .unwrap();
    let second = plan_harmony(
        &request,
        strategy,
        control,
        &NeverInterrupt,
        &CoreHarmonyMetricResolver,
    )
    .unwrap();

    assert_eq!(first, second);
    assert_eq!(first.receipt.status, SearchStatus::Partial);
    assert_eq!(
        first.receipt.reason.as_deref(),
        Some("result bound reached")
    );
    assert_eq!(first.results.len(), 1);
    assert!(first.results[0].complete);
    assert!(first.receipt.work_used <= 100_000);
    assert!(!first.receipt.optimal);
}

#[test]
fn layered_budget_stop_returns_inspectable_longest_prefixes() {
    let run = plan_harmony(
        &fixture(8),
        HarmonizationStrategy::LayeredDp,
        SearchControl::default()
            .with_max_work(20)
            .with_max_results(2),
        &NeverInterrupt,
        &CoreHarmonyMetricResolver,
    )
    .unwrap();

    assert_eq!(run.receipt.status, SearchStatus::Partial);
    assert_eq!(run.receipt.reason.as_deref(), Some("work bound reached"));
    assert!(!run.results.is_empty());
    assert!(run.results.iter().all(|result| !result.complete));
    assert!(!run.receipt.rejections.is_empty());
}
