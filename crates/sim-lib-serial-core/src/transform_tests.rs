use crate::{
    AggregateRule, AlphabetId, BlockPartition, BlockPartitionError, FiniteAlphabet, OrdinalMap,
    OrdinalMapError, ProjectedClassSpec, ProjectionId, RelaxedInvariant, SerialAlphabet, Series,
    SeriesTransform, SeriesTransformError, SymbolBijection, SymbolBijectionError,
};
use sim_lib_discrete_rank::PermutationSpace;
use sim_lib_rank::Nat;

fn gestures() -> FiniteAlphabet<&'static str> {
    FiniteAlphabet::try_new(
        AlphabetId::try_new("gesture/five-v1").expect("id"),
        vec!["rise", "fall", "hold", "turn", "rest"],
    )
    .expect("alphabet")
}

fn letters() -> FiniteAlphabet<&'static str> {
    FiniteAlphabet::try_new(
        AlphabetId::try_new("letter/five-v1").expect("id"),
        vec!["a", "b", "c", "d", "e"],
    )
    .expect("alphabet")
}

fn source() -> Series<FiniteAlphabet<&'static str>> {
    Series::try_new(
        gestures(),
        AggregateRule::exhaustive_exactly_once(),
        vec!["turn", "rise", "rest", "fall", "hold"],
    )
    .expect("source series")
}

fn small_source(cardinality: usize) -> Series<FiniteAlphabet<usize>> {
    let alphabet = FiniteAlphabet::try_new(
        AlphabetId::try_new(format!("test/small-{cardinality}")).expect("id"),
        (0..cardinality).collect(),
    )
    .expect("small alphabet");
    Series::try_new(
        alphabet,
        AggregateRule::exhaustive_exactly_once(),
        (0..cardinality).collect(),
    )
    .expect("small series")
}

#[test]
fn positional_transforms_and_certificates_are_exact() {
    let source = source();

    let retrograde = source
        .apply(&SeriesTransform::retrograde(5))
        .expect("retrograde");
    assert_eq!(
        retrograde.series.order(),
        ["hold", "fall", "rest", "rise", "turn"]
    );
    assert_eq!(
        retrograde.certificate.order_map.output_to_input(),
        [4, 3, 2, 1, 0]
    );
    assert_eq!(
        retrograde.certificate.relaxed_invariants,
        [RelaxedInvariant::SourceOrder]
    );

    let rotation = source
        .apply(&SeriesTransform::rotation(5, 7))
        .expect("rotation modulo five");
    assert_eq!(
        rotation.series.order(),
        ["rest", "fall", "hold", "turn", "rise"]
    );

    let partition =
        BlockPartition::try_new(5, vec![vec![2, 3], vec![0, 1], vec![4]]).expect("block partition");
    let partitioned = source
        .apply(&SeriesTransform::block_partition(partition))
        .expect("partition transform");
    assert_eq!(
        partitioned.series.order(),
        ["rest", "fall", "turn", "rise", "hold"]
    );

    let ordinal = OrdinalMap::try_new(vec![4, 0, 3, 1, 2]).expect("ordinal map");
    let permuted = source
        .apply(&SeriesTransform::ordinal_permutation(ordinal))
        .expect("ordinal permutation");
    assert_eq!(
        permuted.series.order(),
        ["hold", "turn", "fall", "rise", "rest"]
    );

    for transformed in [retrograde, rotation, partitioned, permuted] {
        assert!(transformed.certificate.aggregate_preserved);
        assert_eq!(
            transformed.certificate.source_alphabet,
            *source.alphabet().id()
        );
        assert_eq!(
            transformed.certificate.target_alphabet,
            *source.alphabet().id()
        );
        let inverse = transformed
            .certificate
            .inverse
            .as_ref()
            .expect("specified transforms are invertible");
        let restored = transformed.series.apply(inverse).expect("inverse applies");
        assert_eq!(restored.series, source);
    }
}

#[test]
fn cyclic_and_caller_bijections_rebind_aggregate_evidence() {
    let source = source();
    let cyclic = source
        .apply(&SeriesTransform::cyclic_relabeling(gestures(), 2).expect("cyclic map"))
        .expect("cyclic relabeling");
    assert_eq!(
        cyclic.series.order(),
        ["rise", "hold", "fall", "turn", "rest"]
    );
    assert!(cyclic.certificate.aggregate_preserved);
    assert_eq!(
        cyclic.certificate.relaxed_invariants,
        [RelaxedInvariant::SymbolIdentity]
    );

    let mapping = SymbolBijection::try_new(
        gestures(),
        letters(),
        [
            ("rise", "c"),
            ("fall", "a"),
            ("hold", "e"),
            ("turn", "b"),
            ("rest", "d"),
        ],
    )
    .expect("caller bijection");
    let transformed = source
        .apply(&SeriesTransform::bijection(mapping))
        .expect("cross-alphabet relabeling");
    assert_eq!(transformed.series.order(), ["b", "c", "d", "a", "e"]);
    assert_eq!(transformed.series.alphabet(), &letters());
    assert_eq!(
        transformed.certificate.relaxed_invariants,
        [
            RelaxedInvariant::SymbolIdentity,
            RelaxedInvariant::AlphabetIdentity
        ]
    );
    let inverse = transformed
        .certificate
        .inverse
        .as_ref()
        .expect("bijection has inverse");
    assert_eq!(
        transformed.series.apply(inverse).expect("inverse").series,
        source
    );
}

#[test]
fn declared_and_projected_rules_survive_cross_alphabet_bijections() {
    let alphabet = gestures();
    let mapping = SymbolBijection::try_new(
        alphabet.clone(),
        letters(),
        [
            ("rise", "c"),
            ("fall", "a"),
            ("hold", "e"),
            ("turn", "b"),
            ("rest", "d"),
        ],
    )
    .expect("mapping");
    let operation = SeriesTransform::bijection(mapping);

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
    .expect("rule");
    let repeated = Series::try_new(
        alphabet.clone(),
        multiplicity,
        vec!["rise", "fall", "rise", "hold", "turn", "rest"],
    )
    .expect("series");
    let mapped = repeated.apply(&operation).expect("mapped multiplicity");
    assert_eq!(mapped.series.ledger().expected_count(&"c"), Some(2));
    assert_eq!(mapped.series.ledger().observed_count(&"c"), Some(2));

    let projected = AggregateRule::projected_aggregate(
        &alphabet,
        [
            ProjectedClassSpec::new(
                ProjectionId::try_new("motion").expect("id"),
                vec!["rise", "fall", "turn"],
                2,
            ),
            ProjectedClassSpec::new(
                ProjectionId::try_new("stasis").expect("id"),
                vec!["hold", "rest"],
                1,
            ),
        ],
    )
    .expect("projected rule");
    let projected = Series::try_new(alphabet, projected, vec!["rise", "turn", "rest"])
        .expect("projected series")
        .apply(&operation)
        .expect("mapped projection");
    assert_eq!(projected.series.order(), ["c", "b", "d"]);
    assert_eq!(projected.series.ledger().projected_classes()[0].observed, 2);
    assert_eq!(projected.series.ledger().projected_classes()[1].observed, 1);
}

#[test]
fn malformed_ordinal_partition_and_symbol_maps_fail_before_application() {
    assert!(matches!(
        OrdinalMap::try_new(vec![0, 0]),
        Err(OrdinalMapError::DuplicateInput { .. })
    ));
    assert!(matches!(
        OrdinalMap::try_new(vec![0, 2]),
        Err(OrdinalMapError::OutOfRange { .. })
    ));
    assert!(matches!(
        BlockPartition::try_new(3, vec![vec![0, 1]]),
        Err(BlockPartitionError::CardinalityMismatch { .. })
    ));
    assert!(matches!(
        BlockPartition::try_new(3, vec![vec![0], vec![], vec![1, 2]]),
        Err(BlockPartitionError::EmptyBlock { block: 1 })
    ));
    assert!(matches!(
        BlockPartition::try_new(3, vec![vec![0, 1], vec![1]]),
        Err(BlockPartitionError::OrdinalMap(
            OrdinalMapError::DuplicateInput { .. }
        ))
    ));

    assert!(matches!(
        SymbolBijection::try_new(gestures(), letters(), [("rise", "a"), ("fall", "b")]),
        Err(SymbolBijectionError::MissingSource { .. })
    ));
    assert!(matches!(
        SymbolBijection::try_new(
            gestures(),
            letters(),
            [
                ("rise", "a"),
                ("fall", "a"),
                ("hold", "c"),
                ("turn", "d"),
                ("rest", "e")
            ]
        ),
        Err(SymbolBijectionError::DuplicateTarget { position: 0 })
    ));
    assert!(matches!(
        source().apply(&SeriesTransform::identity(4)),
        Err(SeriesTransformError::OrdinalMap(
            OrdinalMapError::CardinalityMismatch {
                expected: 4,
                found: 5
            }
        ))
    ));
}

#[test]
fn small_alphabets_exhaustively_obey_identity_inverse_closure_and_composition() {
    for cardinality in 1..=5 {
        let source = small_source(cardinality);
        let space = PermutationSpace::try_new(cardinality).expect("permutation space");
        let count = (1..=cardinality).product::<usize>();
        for rank in 0..count {
            let permutation = space
                .unrank(&Nat::from(rank as u64))
                .expect("shared unrank");
            let order_map = OrdinalMap::try_new(permutation).expect("unrank is a bijection");
            let operation = SeriesTransform::ordinal_permutation(order_map.clone());
            let transformed = source.apply(&operation).expect("closed transform");
            assert!(transformed.series.ledger().is_exhaustive_exactly_once());
            assert_eq!(
                operation.canonical_form(),
                SeriesTransform::<FiniteAlphabet<usize>>::ordinal_permutation(order_map.clone())
                    .canonical_form()
            );

            let inverse = transformed
                .certificate
                .inverse
                .as_ref()
                .expect("permutation inverse");
            let restored = transformed.series.apply(inverse).expect("inverse applies");
            assert_eq!(restored.series, source);

            let identity = operation.compose(inverse).expect("compose inverse");
            assert_eq!(
                identity.canonical_form(),
                SeriesTransform::<FiniteAlphabet<usize>>::identity(cardinality).canonical_form()
            );
            assert_eq!(
                source.apply(&identity).expect("identity applies").series,
                source
            );

            let rotation = SeriesTransform::rotation(cardinality, rank + 1);
            let sequential = transformed
                .series
                .apply(&rotation)
                .expect("sequential rotation");
            let composed = operation.compose(&rotation).expect("composition");
            assert_eq!(
                source.apply(&composed).expect("composed apply").series,
                sequential.series
            );
        }
    }
}

#[test]
fn block_and_ordinal_spellings_have_one_deterministic_canonical_form() {
    let partition =
        BlockPartition::try_new(5, vec![vec![2, 3], vec![0, 1], vec![4]]).expect("partition");
    let block = SeriesTransform::<FiniteAlphabet<&str>>::block_partition(partition);
    let ordinal = SeriesTransform::<FiniteAlphabet<&str>>::ordinal_permutation(
        OrdinalMap::try_new(vec![2, 3, 0, 1, 4]).expect("map"),
    );
    assert_eq!(block, ordinal);
    assert_eq!(block.canonical_form(), ordinal.canonical_form());
}

#[test]
fn library_transform_paths_contain_no_panicking_extractors() {
    for source in [
        include_str!("permutation.rs"),
        include_str!("transform.rs"),
        include_str!("certificate.rs"),
    ] {
        assert!(!source.contains(".expect("));
        assert!(!source.contains(".unwrap("));
    }
}
