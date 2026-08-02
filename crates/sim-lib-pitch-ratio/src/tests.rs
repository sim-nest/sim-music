use super::*;
use sim_lib_discrete_search::{SearchControl, SearchOrder, SearchStatus};

#[test]
fn ratio_identity_reduces_and_octave_folds() {
    let ratio = PitchRatio::new(12, 8).unwrap();
    assert_eq!(ratio.numerator(), 3);
    assert_eq!(ratio.denominator(), 2);
    assert_eq!(
        PitchRatio::new(9, 1).unwrap().octave_reduced().unwrap(),
        PitchRatio::new(9, 8).unwrap()
    );
}

#[test]
fn invalid_ratios_and_unbounded_factoring_are_rejected() {
    assert_eq!(
        PitchRatio::new(0, 1),
        Err(PitchRatioError::NonPositiveRatio)
    );
    assert_eq!(
        PitchRatio::new(3, 2).unwrap().factor_vector(RatioPolicy {
            octave_reduce: true,
            prime_limit: None
        }),
        Err(PitchRatioError::UnboundedFactorization)
    );
    assert!(matches!(
        PitchRatio::new(7, 4)
            .unwrap()
            .factor_vector(RatioPolicy::five_limit()),
        Err(PitchRatioError::PrimeLimitExceeded { .. })
    ));
}

#[test]
fn factor_vector_uses_signed_prime_exponents() {
    let vector = PitchRatio::new(45, 32)
        .unwrap()
        .factor_vector(RatioPolicy::five_limit())
        .unwrap();
    assert_eq!(vector.primes, vec![2, 3, 5]);
    assert_eq!(vector.exponents, vec![-5, 2, 1]);
    assert_eq!(vector.to_ratio().unwrap(), PitchRatio::new(45, 32).unwrap());
}

#[test]
fn rank_unrank_is_bijective_over_declared_domain() {
    let policy = RatioPolicy::five_limit();
    let ratios = [
        PitchRatio::new(1, 1).unwrap(),
        PitchRatio::new(3, 2).unwrap(),
        PitchRatio::new(5, 4).unwrap(),
        PitchRatio::new(45, 32).unwrap(),
    ];
    for ratio in ratios {
        let rank = rank_ratio(ratio, policy).unwrap();
        assert_eq!(
            unrank_ratio(&rank, policy).unwrap(),
            ratio.canonical(policy).unwrap()
        );
    }
}

#[test]
fn cents_and_tuning_error_are_exact_ratio_derived() {
    let octave = PitchRatio::new(2, 1).unwrap();
    assert!((octave.cents() - 1200.0).abs() < 1e-9);
    let fifth = PitchRatio::new(3, 2).unwrap();
    assert!(fifth.tuning_error_cents(700.0) < 2.0);
}

#[test]
fn approximation_search_respects_receipts_and_error() {
    let run = approximate_ratio_with_strategy(
        701.955_000_865,
        RatioPolicy::five_limit(),
        SearchControl::default()
            .with_order(SearchOrder::BestFirst)
            .with_max_work(30_000)
            .with_max_results(4)
            .with_branch_and_bound(true),
        ApproximationStrategy::Nearest,
    );
    assert_eq!(run.receipt.status, SearchStatus::Partial);
    assert_eq!(run.outputs[0].ratio, PitchRatio::new(3, 2).unwrap());
    assert!(run.outputs[0].error_cents.abs() < 0.001);
    assert!(run.receipt.work_used <= 30_000);
}

#[test]
fn approximation_strategies_are_available() {
    for strategy in [
        ApproximationStrategy::Nearest,
        ApproximationStrategy::First,
        ApproximationStrategy::Balanced,
    ] {
        let run = approximate_ratio_with_strategy(
            386.313_713_865,
            RatioPolicy::five_limit(),
            SearchControl::default()
                .with_order(SearchOrder::BestFirst)
                .with_max_work(20_000)
                .with_max_results(2),
            strategy,
        );
        assert!(!run.outputs.is_empty());
        assert!(
            run.outputs
                .iter()
                .all(|output| output.error_cents.is_finite())
        );
    }
}

#[test]
fn ratio_chord_matrix_declares_root_and_exact_directed_intervals() {
    let chord = [
        PitchRatio::new(1, 1).unwrap(),
        PitchRatio::new(5, 4).unwrap(),
        PitchRatio::new(3, 2).unwrap(),
    ];
    let report = analyze_ratio_chord(&chord, RatioPolicy::five_limit()).unwrap();

    assert_eq!(report.matrix[0][0], PitchRatio::new(1, 1).unwrap());
    assert_eq!(report.matrix[0][1], PitchRatio::new(5, 4).unwrap());
    assert_eq!(report.matrix[0][2], PitchRatio::new(3, 2).unwrap());
    assert_eq!(report.matrix[1][2], PitchRatio::new(6, 5).unwrap());
    assert_eq!(report.matrix[2][1], PitchRatio::new(5, 3).unwrap());
    assert_eq!(report.covered.admitted_tones, 3);
    assert_eq!(report.covered.matrix_entries, 9);
    assert_eq!(report.covered.rejected_intervals, 0);
    assert!(report.cost > 0.0);
}

#[test]
fn chord_cost_standard_is_default_and_legacy_deviation_is_opt_in() {
    let matrix = ratio_interval_matrix(
        &[
            PitchRatio::new(1, 1).unwrap(),
            PitchRatio::new(5, 4).unwrap(),
            PitchRatio::new(3, 2).unwrap(),
        ],
        RatioPolicy::five_limit(),
    )
    .unwrap();
    let standard = generalized_mean_chord_cost(
        &matrix,
        RatioPolicy::five_limit(),
        2.0,
        MeanDialect::Standard,
    )
    .unwrap();
    let legacy = generalized_mean_chord_cost(
        &matrix,
        RatioPolicy::five_limit(),
        2.0,
        MeanDialect::LegacyTunedNoDivision,
    )
    .unwrap();

    assert_eq!(
        analyze_ratio_chord(
            &[
                PitchRatio::new(1, 1).unwrap(),
                PitchRatio::new(5, 4).unwrap(),
                PitchRatio::new(3, 2).unwrap(),
            ],
            RatioPolicy::five_limit()
        )
        .unwrap()
        .cost,
        standard
    );
    assert!(legacy > standard);
}

#[test]
fn chord_invariants_hold_under_permutation_octave_and_scaling() {
    let policy = RatioPolicy::five_limit();
    let chord = [
        PitchRatio::new(1, 1).unwrap(),
        PitchRatio::new(5, 4).unwrap(),
        PitchRatio::new(3, 2).unwrap(),
    ];
    let permuted = [chord[2], chord[0], chord[1]];
    let octave_shifted = [
        PitchRatio::new(2, 1).unwrap(),
        PitchRatio::new(5, 2).unwrap(),
        PitchRatio::new(3, 1).unwrap(),
    ];
    let scaled = [
        PitchRatio::new(7, 4).unwrap(),
        PitchRatio::new(35, 16).unwrap(),
        PitchRatio::new(21, 8).unwrap(),
    ];

    let base = analyze_ratio_chord(&chord, policy).unwrap();
    let permutation =
        analyze_ratio_chord_with_root(&permuted, 1, policy, 2.0, MeanDialect::Standard).unwrap();
    let octave = analyze_ratio_chord(&octave_shifted, policy).unwrap();
    let scaled = root_normalized_tones(&scaled, 0, policy).unwrap();

    assert_eq!(
        base.covered.distinct_intervals,
        permutation.covered.distinct_intervals
    );
    assert_eq!(base.covered.octave_classes, octave.covered.octave_classes);
    assert_eq!(scaled, chord);
}

#[test]
fn relation_tree_is_lazy_bounded_and_cycle_safe() {
    let run = expand_ratio_relation_tree(
        PitchRatio::unison(),
        &[
            RatioRelation::new("fifth", PitchRatio::new(3, 2).unwrap()),
            RatioRelation::new("fourth", PitchRatio::new(4, 3).unwrap()),
        ],
        RatioPolicy::three_limit(),
        SearchControl::default()
            .with_order(SearchOrder::BreadthFirst)
            .with_max_results(2)
            .with_max_work(100),
    );

    assert_eq!(run.receipt.status, SearchStatus::Partial);
    assert!(
        run.outputs
            .iter()
            .any(|path| path.nodes == vec![PitchRatio::unison(), PitchRatio::new(3, 2).unwrap()])
    );
    assert!(run.outputs.iter().all(|path| {
        path.nodes
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == path.nodes.len()
    }));
}
