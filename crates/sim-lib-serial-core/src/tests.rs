use crate::{
    AggregateRule, AggregateRuleError, AlphabetError, AlphabetId, AlphabetRegistry, FiniteAlphabet,
    ProjectedClassSpec, ProjectionId, Series, SeriesError,
};
use sim_lib_rank::Nat;

fn gestures() -> FiniteAlphabet<&'static str> {
    FiniteAlphabet::try_new(
        AlphabetId::try_new("gesture/five-v1").expect("id"),
        vec!["rise", "fall", "hold", "turn", "rest"],
    )
    .expect("alphabet")
}

#[test]
fn five_symbol_non_pitch_alphabet_is_exhaustive_and_ranked_by_discrete_owner() {
    let series = Series::try_new(
        gestures(),
        AggregateRule::exhaustive_exactly_once(),
        vec!["turn", "rise", "rest", "fall", "hold"],
    )
    .expect("series");

    assert_eq!(series.order(), ["turn", "rise", "rest", "fall", "hold"]);
    assert!(series.ledger().is_exhaustive_exactly_once());
    assert_eq!(series.ledger().observed_count(&"rest"), Some(1));
    assert_eq!(series.permutation_rank().expect("rank"), Nat::from(76u64));
}

#[test]
fn exhaustive_no_repeat_and_free_order_have_distinct_contracts() {
    let alphabet = gestures();
    assert!(matches!(
        Series::try_new(
            alphabet.clone(),
            AggregateRule::exhaustive_exactly_once(),
            vec!["rise", "fall"]
        ),
        Err(SeriesError::WrongLength {
            expected: 5,
            found: 2
        })
    ));
    let no_repeat = Series::try_new(
        alphabet.clone(),
        AggregateRule::no_repeat(),
        vec!["rise", "turn"],
    )
    .expect("short no-repeat series");
    assert_eq!(no_repeat.ledger().omitted_symbols().len(), 3);
    assert!(matches!(
        Series::try_new(
            alphabet.clone(),
            AggregateRule::no_repeat(),
            vec!["rise", "rise"]
        ),
        Err(SeriesError::RepeatedSymbol {
            position: 1,
            first: 0
        })
    ));
    let free = Series::try_new(
        alphabet,
        AggregateRule::free_order(),
        vec!["rise", "rise", "rest"],
    )
    .expect("free order");
    assert_eq!(free.ledger().repeated_symbols(), ["rise"]);
    assert!(matches!(
        free.permutation_rank(),
        Err(SeriesError::NotPermutation(_))
    ));
}

#[test]
fn declared_multiplicity_and_omissions_are_symbol_based() {
    let alphabet = gestures();
    let multiplicity = AggregateRule::declared_multiplicity(
        &alphabet,
        [
            ("rise", 2),
            ("fall", 1),
            ("hold", 1),
            ("turn", 1),
            ("rest", 1),
        ],
    )
    .expect("multiplicity");
    let repeated = Series::try_new(
        alphabet.clone(),
        multiplicity,
        vec!["rise", "fall", "rise", "hold", "turn", "rest"],
    )
    .expect("declared repeat");
    assert_eq!(repeated.ledger().expected_count(&"rise"), Some(2));
    assert_eq!(repeated.ledger().observed_count(&"rise"), Some(2));

    let omissions =
        AggregateRule::declared_omissions(&alphabet, ["fall", "rest"]).expect("omissions");
    let shortened = Series::try_new(alphabet, omissions, vec!["rise", "hold", "turn"])
        .expect("omitted aggregate");
    assert_eq!(shortened.ledger().expected_count(&"fall"), Some(0));
    assert_eq!(shortened.ledger().omitted_symbols(), ["fall", "rest"]);
}

#[test]
fn projected_aggregate_counts_declared_classes() {
    let alphabet = gestures();
    let rule = AggregateRule::projected_aggregate(
        &alphabet,
        [
            ProjectedClassSpec::new(
                ProjectionId::try_new("motion").expect("projection"),
                vec!["rise", "fall", "turn"],
                2,
            ),
            ProjectedClassSpec::new(
                ProjectionId::try_new("stasis").expect("projection"),
                vec!["hold", "rest"],
                1,
            ),
        ],
    )
    .expect("projected rule");
    let series = Series::try_new(alphabet.clone(), rule.clone(), vec!["rise", "turn", "rest"])
        .expect("projected series");
    assert_eq!(series.ledger().projected_classes()[0].observed, 2);
    assert_eq!(series.ledger().projected_classes()[1].observed, 1);
    assert!(matches!(
        Series::try_new(alphabet, rule, vec!["rise", "hold", "rest"]),
        Err(SeriesError::ProjectionMismatch { .. })
    ));
}

#[test]
fn malformed_alphabets_members_and_rules_fail_closed() {
    assert!(matches!(
        AlphabetId::try_new("gesture id"),
        Err(AlphabetError::InvalidId { .. })
    ));
    assert!(matches!(
        FiniteAlphabet::<&str>::try_new(AlphabetId::try_new("empty").unwrap(), vec![]),
        Err(AlphabetError::Empty { .. })
    ));
    assert!(matches!(
        FiniteAlphabet::try_new(
            AlphabetId::try_new("duplicate").unwrap(),
            vec!["a", "b", "a"]
        ),
        Err(AlphabetError::DuplicateSymbol {
            first: 0,
            duplicate: 2,
            ..
        })
    ));
    assert!(matches!(
        Series::try_new(
            gestures(),
            AggregateRule::free_order(),
            vec!["rise", "foreign"]
        ),
        Err(SeriesError::ForeignSymbol { position: 1, .. })
    ));
    assert!(matches!(
        AggregateRule::declared_multiplicity(&gestures(), [("rise", 1)]),
        Err(AggregateRuleError::MissingDeclaration { .. })
    ));
    assert!(matches!(
        AggregateRule::declared_omissions(&gestures(), std::iter::empty()),
        Err(AggregateRuleError::NoOmissions)
    ));
    assert!(matches!(
        AggregateRule::declared_omissions(&gestures(), ["rise", "fall", "hold", "turn", "rest"]),
        Err(AggregateRuleError::OmitsEverything(_))
    ));
}

#[test]
fn registry_rejects_duplicate_stable_alphabet_ids() {
    let mut registry = AlphabetRegistry::new();
    registry.insert(gestures()).expect("first alphabet");
    assert!(matches!(
        registry.insert(gestures()),
        Err(AlphabetError::DuplicateId(_))
    ));
    assert_eq!(registry.len(), 1);
}

#[test]
fn rules_reject_foreign_duplicate_and_impossible_projection_data() {
    let alphabet = gestures();
    assert!(matches!(
        AggregateRule::declared_multiplicity(
            &alphabet,
            [
                ("rise", 1),
                ("rise", 1),
                ("hold", 1),
                ("turn", 1),
                ("rest", 1),
            ]
        ),
        Err(AggregateRuleError::DuplicateDeclaration { position: 0 })
    ));
    assert!(matches!(
        AggregateRule::projected_aggregate(
            &alphabet,
            [ProjectedClassSpec::new(
                ProjectionId::try_new("all").unwrap(),
                vec!["rise", "fall", "hold", "turn", "unknown"],
                1,
            )]
        ),
        Err(AggregateRuleError::ForeignSymbol { .. })
    ));
    assert!(matches!(
        AggregateRule::projected_aggregate(
            &alphabet,
            [ProjectedClassSpec::new(
                ProjectionId::try_new("none").unwrap(),
                vec!["rise", "fall", "hold", "turn", "rest"],
                0,
            )]
        ),
        Err(AggregateRuleError::OmitsEverything(_))
    ));
}
