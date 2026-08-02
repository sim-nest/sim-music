use std::collections::BTreeSet;

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{
    BlockOrder, DerivationKind, MatrixCoordinate, OrderKind, OrderedIntervalString,
    PitchClassAlphabet, ROW_MATRIX_SIZE, RowError, RowFamily, RowFamilySet, RowLabel,
    RowLabelConvention, RowMatrix, RowOperation, RowSegmentSource, ToneRow,
    analyze_combinatoriality_partition, analyze_derivation_partition,
    analyze_interlocking_partitions, analyze_invariance, analyze_mosaic,
    analyze_partition_aggregate_coverage, analyze_partition_similarity, analyze_row_class,
    try_partition, verticalize,
};
use sim_lib_serial_core::SeriesError;

const CHROMATIC: [PitchClass; 12] = [
    PitchClass::C,
    PitchClass::CS,
    PitchClass::D,
    PitchClass::DS,
    PitchClass::E,
    PitchClass::F,
    PitchClass::FS,
    PitchClass::G,
    PitchClass::GS,
    PitchClass::A,
    PitchClass::AS,
    PitchClass::B,
];

const OP_25: [PitchClass; 12] = [
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
];

const ALL_INTERVAL_ROW: [PitchClass; 12] = [
    PitchClass::C,
    PitchClass::CS,
    PitchClass::DS,
    PitchClass::G,
    PitchClass::D,
    PitchClass::F,
    PitchClass::B,
    PitchClass::AS,
    PitchClass::GS,
    PitchClass::E,
    PitchClass::A,
    PitchClass::FS,
];

const DERIVED_COMBINATORIAL_ROW: [PitchClass; 12] = [
    PitchClass::C,
    PitchClass::CS,
    PitchClass::D,
    PitchClass::DS,
    PitchClass::E,
    PitchClass::F,
    PitchClass::FS,
    PitchClass::G,
    PitchClass::B,
    PitchClass::AS,
    PitchClass::A,
    PitchClass::GS,
];

fn values(classes: &[PitchClass; 12]) -> [u8; 12] {
    classes.map(PitchClass::value)
}

#[test]
fn gate_row_canonical_alphabet_and_malformed_aggregates() {
    let alphabet = PitchClassAlphabet::try_new().expect("canonical pitch-class alphabet");
    assert_eq!(
        alphabet
            .classes()
            .iter()
            .copied()
            .map(PitchClass::value)
            .collect::<Vec<_>>(),
        (0..12).collect::<Vec<_>>()
    );

    let row = ToneRow::try_from_classes(CHROMATIC).expect("chromatic row");
    assert_eq!(row.classes(), &CHROMATIC);

    let malformed = [PitchClass::C; 12];
    assert!(matches!(
        ToneRow::try_from_classes(malformed),
        Err(RowError::Aggregate(SeriesError::MultiplicityMismatch {
            alphabet_position: 0,
            expected: 1,
            found: 12,
        }))
    ));
}

#[test]
fn gate_row_total_p_i_r_ri_for_zero_and_nonzero_starts() {
    let zero = ToneRow::try_from_classes(CHROMATIC).expect("zero-starting row");
    assert_eq!(
        values(zero.apply(RowOperation::new(RowFamily::P, 5)).classes()),
        [5, 6, 7, 8, 9, 10, 11, 0, 1, 2, 3, 4]
    );
    assert_eq!(
        values(zero.apply(RowOperation::new(RowFamily::I, 5)).classes()),
        [5, 4, 3, 2, 1, 0, 11, 10, 9, 8, 7, 6]
    );
    assert_eq!(
        values(zero.apply(RowOperation::new(RowFamily::R, 5)).classes()),
        [4, 3, 2, 1, 0, 11, 10, 9, 8, 7, 6, 5]
    );
    assert_eq!(
        values(zero.apply(RowOperation::new(RowFamily::RI, 5)).classes()),
        [6, 7, 8, 9, 10, 11, 0, 1, 2, 3, 4, 5]
    );

    let nonzero = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    assert_eq!(
        values(nonzero.classes()),
        [4, 5, 7, 1, 6, 3, 8, 2, 11, 0, 9, 10]
    );
    assert_eq!(
        nonzero
            .apply(RowOperation {
                family: RowFamily::P,
                addend: 255,
            })
            .operation(),
        RowOperation::new(RowFamily::P, 3)
    );
}

#[test]
fn gate_row_operations_obey_inverse_laws() {
    let source = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    for family in [RowFamily::P, RowFamily::I, RowFamily::R, RowFamily::RI] {
        for addend in 0..12 {
            let operation = RowOperation::new(family, addend);
            let transformed = source.apply(operation);
            let restored = transformed.row().apply(operation.inverse());
            assert_eq!(
                restored.classes(),
                source.classes(),
                "inverse law failed for {operation}"
            );
        }
    }
}

#[test]
fn gate_row_operation_identity_and_labels_remain_distinct() {
    let source = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    for family in [RowFamily::P, RowFamily::I, RowFamily::R, RowFamily::RI] {
        let form = source.apply(RowOperation::new(family, 0));
        assert_eq!(form.operation(), RowOperation::new(family, 0));
        assert_eq!(
            form.label(RowLabelConvention::OperationIndex),
            RowLabel::new(family, 0)
        );
        let expected_pitch_label = match family {
            RowFamily::P | RowFamily::R => 4,
            RowFamily::I | RowFamily::RI => 8,
        };
        assert_eq!(
            form.label(RowLabelConvention::FirstLastPitch),
            RowLabel::new(family, expected_pitch_label)
        );
    }

    let retrograde = source.apply(RowOperation::new(RowFamily::R, 7));
    assert_eq!(retrograde.classes()[0], PitchClass::F);
    assert_eq!(retrograde.classes()[11], PitchClass::B);
    assert_eq!(
        retrograde
            .label(RowLabelConvention::FirstLastPitch)
            .to_string(),
        "R11"
    );
    assert_eq!(
        retrograde
            .label(RowLabelConvention::OperationIndex)
            .to_string(),
        "R7"
    );
}

#[test]
fn gate_ordered_intervals_obey_p_i_r_ri_laws() {
    let source = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    let intervals = source.ordered_intervals();
    assert_eq!(intervals.intervals(), &[1, 2, 6, 5, 9, 5, 6, 9, 1, 9, 1]);

    for family in [RowFamily::P, RowFamily::I, RowFamily::R, RowFamily::RI] {
        let form = source.apply(RowOperation::new(family, 7));
        assert_eq!(
            OrderedIntervalString::of_row(form.row()),
            intervals.under_family(family)
        );
    }
}

#[test]
fn gate_segments_retain_order_and_unordered_set_facts() {
    let source = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");

    let contiguous = source.segment(0, 4).expect("contiguous segment");
    assert_eq!(
        contiguous.source(),
        &RowSegmentSource::Contiguous { start: 0, len: 4 }
    );
    assert_eq!(contiguous.ordinals(), &[0, 1, 2, 3]);
    assert_eq!(
        contiguous
            .classes()
            .iter()
            .map(|pitch_class| pitch_class.value())
            .collect::<Vec<_>>(),
        vec![4, 5, 7, 1]
    );
    assert_eq!(contiguous.mask().count_bits(), 4);
    assert_eq!(contiguous.ordered_intervals(), vec![1, 2, 6]);

    let wrapped = source.wrapped_segment(10, 4).expect("wrapped segment");
    assert_eq!(
        wrapped.source(),
        &RowSegmentSource::Wrapped { start: 10, len: 4 }
    );
    assert_eq!(wrapped.ordinals(), &[10, 11, 0, 1]);
    assert_eq!(
        wrapped
            .classes()
            .iter()
            .map(|pitch_class| pitch_class.value())
            .collect::<Vec<_>>(),
        vec![9, 10, 4, 5]
    );

    let indexed = source
        .indexed_segment(&[0, 3, 6, 9])
        .expect("indexed segment");
    assert_eq!(indexed.source(), &RowSegmentSource::Indexed);
    assert_eq!(indexed.ordinals(), &[0, 3, 6, 9]);
    assert_eq!(
        indexed
            .classes()
            .iter()
            .map(|pitch_class| pitch_class.value())
            .collect::<Vec<_>>(),
        vec![4, 1, 8, 0]
    );

    let labeled = ToneRow::try_from_classes(CHROMATIC)
        .expect("chromatic row")
        .segment(0, 3)
        .expect("named contiguous segment");
    assert_eq!(labeled.forte_label(), Some("3-1"));

    assert_eq!(
        source.segment(10, 3),
        Err(RowError::SegmentOutOfBounds { start: 10, len: 3 })
    );
    assert_eq!(
        source.wrapped_segment(0, 13),
        Err(RowError::WrappedSegmentTooLong { len: 13 })
    );
    assert_eq!(
        source.indexed_segment(&[12]),
        Err(RowError::InvalidOrdinal { ordinal: 12 })
    );
}

#[test]
fn gate_invariance_distinguishes_ordered_and_unordered_evidence() {
    let source = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    let left = source.segment(0, 4).expect("left segment");
    let transposed_row = source.apply(RowOperation::new(RowFamily::P, 10)).into_row();
    let transposed = transposed_row.segment(0, 4).expect("transposed segment");
    let reordered = source
        .indexed_segment(&[3, 2, 1, 0])
        .expect("reordered segment");

    let transposed_invariant = analyze_invariance(&left, &transposed);
    assert!(transposed_invariant.ordinal_identity);
    assert!(!transposed_invariant.pitch_identity);
    assert_eq!(transposed_invariant.transposition, Some(10));
    assert_eq!(transposed_invariant.inversion, None);
    assert!(transposed_invariant.interval_order_identity);
    assert!(transposed_invariant.set_class_identity);

    let reordered_invariant = analyze_invariance(&left, &reordered);
    assert!(!reordered_invariant.ordinal_identity);
    assert!(!reordered_invariant.pitch_identity);
    assert_eq!(reordered_invariant.transposition, None);
    assert_eq!(reordered_invariant.inversion, None);
    assert!(!reordered_invariant.interval_order_identity);
    assert!(reordered_invariant.set_class_identity);
}

#[test]
fn gate_row_class_reports_stabilizers_and_form_equivalence() {
    let row = ToneRow::try_from_classes(CHROMATIC).expect("chromatic row");
    let report = analyze_row_class(&row);

    assert_eq!(report.ordered_intervals.intervals(), &[1; 11]);
    assert_eq!(report.aliases.len(), 48);
    assert_eq!(report.distinct_forms.len(), 24);
    assert_eq!(report.stabilizers.len(), 2);
    assert_eq!(
        report.stabilizers,
        vec![
            RowOperation::new(RowFamily::P, 0),
            RowOperation::new(RowFamily::RI, 11)
        ]
    );
    assert_eq!(report.form_equivalences.len(), report.distinct_forms.len());
    assert!(
        report
            .form_equivalences
            .iter()
            .all(|equivalence| !equivalence.operations.is_empty())
    );
    assert!(report.form_equivalences.iter().any(
        |equivalence| equivalence.operations.len() == 2 && equivalence.invariant.pitch_identity
    ));
}

#[test]
fn gate_row_class_reports_derivation_all_interval_and_combinatoriality() {
    let derived = ToneRow::try_from_classes(DERIVED_COMBINATORIAL_ROW).expect("derived row");
    let derived_report = analyze_row_class(&derived);

    assert_eq!(derived_report.derivation.generator_size, Some(4));
    assert!(
        derived_report
            .derivation
            .matches
            .iter()
            .any(|entry| entry.kind == DerivationKind::Tetrachordal && entry.generator_size == 4)
    );
    assert!(
        derived_report
            .combinatoriality
            .iter()
            .all(|partner| { partner.source.union(partner.complement).count_bits() == 12 })
    );
    let families = derived_report
        .combinatoriality
        .iter()
        .map(|partner| partner.operation.family)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        families,
        [RowFamily::P, RowFamily::I, RowFamily::R, RowFamily::RI,]
            .into_iter()
            .collect()
    );
    let same_row_partner = derived_report
        .combinatoriality
        .iter()
        .find(|partner| partner.operation == RowOperation::new(RowFamily::P, 0))
        .expect("P0 combinatorial witness");
    assert_eq!(same_row_partner.partition.block_size, 6);
    assert_eq!(same_row_partner.partition.partner_block_order, vec![1, 0]);

    let all_interval = ToneRow::try_from_classes(ALL_INTERVAL_ROW).expect("all-interval row");
    let all_interval_report = analyze_row_class(&all_interval);
    assert!(all_interval_report.all_interval.is_all_interval);
    assert!(all_interval_report.all_interval.duplicates.is_empty());
    assert!(all_interval_report.all_interval.missing.is_empty());

    let non_all_interval = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    let non_all_interval_report = analyze_row_class(&non_all_interval);
    assert!(!non_all_interval_report.all_interval.is_all_interval);
    assert_eq!(
        non_all_interval_report.all_interval.missing,
        vec![3, 4, 7, 8, 10, 11]
    );
}

#[test]
fn gate_row_partition_analyses_reject_invalid_requests() {
    let row = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");

    assert_eq!(
        analyze_derivation_partition(&row, 5),
        Err(RowError::InvalidPartitionSize { size: 5 })
    );
    assert_eq!(
        analyze_combinatoriality_partition(&row, RowOperation::new(RowFamily::P, 0), 5),
        Err(RowError::InvalidPartitionSize { size: 5 })
    );
    assert_eq!(
        try_partition(vec![vec![0, 1], vec![2, 12]], BlockOrder::total()),
        Err(RowError::InvalidOrdinal { ordinal: 12 })
    );
    assert_eq!(
        try_partition(vec![vec![0, 1], vec![]], BlockOrder::total()),
        Err(RowError::EmptyPartitionBlock { block_index: 1 })
    );
    assert_eq!(
        try_partition(vec![vec![0, 1], vec![1, 2]], BlockOrder::total()),
        Err(RowError::DuplicatePartitionOrdinal {
            ordinal: 1,
            first_block_index: 0,
            second_block_index: 1,
        })
    );
    assert_eq!(
        try_partition(vec![vec![0, 1], vec![2, 3]], BlockOrder::total()),
        Err(RowError::PartitionCoverageMismatch {
            missing: vec![4, 5, 6, 7, 8, 9, 10, 11],
        })
    );
}

#[test]
fn gate_row_partitions_validate_order_similarity_and_verticalization() {
    let row = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    let dyadic = try_partition(
        vec![
            vec![0, 1],
            vec![2, 3],
            vec![4, 5],
            vec![6, 7],
            vec![8, 9],
            vec![10, 11],
        ],
        BlockOrder::new(OrderKind::Total, OrderKind::Partial),
    )
    .expect("dyadic partition");
    let dyadic_reblocked = try_partition(
        vec![
            vec![1, 0],
            vec![3, 2],
            vec![5, 4],
            vec![7, 6],
            vec![9, 8],
            vec![11, 10],
        ],
        BlockOrder::new(OrderKind::Partial, OrderKind::Absent),
    )
    .expect("reordered dyadic partition");

    assert_eq!(
        dyadic
            .blocks()
            .iter()
            .flat_map(|block| block.ordinals().iter().copied())
            .collect::<BTreeSet<_>>(),
        (0_u8..12).collect()
    );
    assert_eq!(dyadic.order().within_blocks, OrderKind::Total);
    assert_eq!(dyadic.order().between_blocks, OrderKind::Partial);

    let similarity = analyze_partition_similarity(&dyadic, &dyadic_reblocked);
    assert!(similarity.same_block_size_multiset);
    assert!(!similarity.same_order_contract);
    assert!(similarity.exact_block_matches.is_empty());
    assert_eq!(similarity.overlap_matrix[0][0], 2);

    let vertical = verticalize(&row, &dyadic);
    assert_eq!(vertical.order, dyadic.order());
    assert_eq!(vertical.slices.len(), 6);
    assert_eq!(vertical.slices[0].ordinals, vec![0, 1]);
    assert_eq!(
        vertical.slices[0]
            .pitch_classes
            .iter()
            .map(|pitch_class| pitch_class.value())
            .collect::<Vec<_>>(),
        vec![4, 5]
    );
    assert!(vertical.aggregate_coverage.complete);
    assert_eq!(vertical.aggregate_coverage.missing.bits(), 0);
}

#[test]
fn gate_row_partitions_support_dyadic_through_hexachordal_mosaics() {
    let row = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    let dyadic = try_partition(
        vec![
            vec![0, 1],
            vec![2, 3],
            vec![4, 5],
            vec![6, 7],
            vec![8, 9],
            vec![10, 11],
        ],
        BlockOrder::total(),
    )
    .expect("dyadic partition");
    let trichordal = try_partition(
        vec![vec![0, 1, 2], vec![3, 4, 5], vec![6, 7, 8], vec![9, 10, 11]],
        BlockOrder::total(),
    )
    .expect("trichordal partition");
    let tetrachordal = try_partition(
        vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7], vec![8, 9, 10, 11]],
        BlockOrder::total(),
    )
    .expect("tetrachordal partition");
    let hexachordal = try_partition(
        vec![vec![0, 1, 2, 3, 4, 5], vec![6, 7, 8, 9, 10, 11]],
        BlockOrder::unordered(),
    )
    .expect("hexachordal partition");

    assert_eq!(dyadic.block_sizes(), vec![2, 2, 2, 2, 2, 2]);
    assert_eq!(trichordal.block_sizes(), vec![3, 3, 3, 3]);
    assert_eq!(tetrachordal.block_sizes(), vec![4, 4, 4]);
    assert_eq!(hexachordal.block_sizes(), vec![6, 6]);

    let mosaic = analyze_mosaic(
        &row,
        &[
            dyadic.clone(),
            trichordal.clone(),
            tetrachordal.clone(),
            hexachordal.clone(),
        ],
    );
    assert_eq!(mosaic.blocks.len(), 15);
    assert!(mosaic.aggregate_coverage.complete);

    let hexachordal_coverage = analyze_partition_aggregate_coverage(&row, &hexachordal);
    assert!(hexachordal_coverage.complete);
    assert_eq!(hexachordal_coverage.covered.bits(), 0x0fff);
}

#[test]
fn gate_row_partitions_report_interlocking_evidence() {
    let interleave_a = try_partition(
        vec![vec![0, 2, 4, 6, 8, 10], vec![1, 3, 5, 7, 9, 11]],
        BlockOrder::unordered(),
    )
    .expect("even-odd partition");
    let interleave_b = try_partition(
        vec![vec![0, 1, 4, 5, 8, 9], vec![2, 3, 6, 7, 10, 11]],
        BlockOrder::unordered(),
    )
    .expect("paired partition");

    let report = analyze_interlocking_partitions(&interleave_a, &interleave_b);
    assert!(report.is_interlocking);
    assert_eq!(report.overlap_matrix, vec![vec![3, 3], vec![3, 3]]);
    assert_eq!(report.left_to_right_links, vec![vec![0, 1], vec![0, 1]]);
    assert_eq!(report.right_to_left_links, vec![vec![0, 1], vec![0, 1]]);
}

#[test]
fn gate_row_family_retains_every_alias_and_deduplicates_values() {
    let row = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    let family = RowFamilySet::of(&row);

    assert_eq!(family.source(), &row);
    assert_eq!(family.aliases().len(), 48);
    for alias in family.aliases() {
        assert_eq!(alias.form, row.apply(alias.operation));
        assert_eq!(
            alias.form.row(),
            &family.distinct_forms()[alias.distinct_form_index()]
        );
    }
    assert!(family.distinct_forms().len() <= 48);

    for (index, distinct) in family.distinct_forms().iter().enumerate() {
        let aliases = family.aliases_for_distinct_form(index).collect::<Vec<_>>();
        assert!(!aliases.is_empty());
        assert!(aliases.iter().all(|alias| alias.form.row() == distinct));
    }
}

#[test]
fn gate_symmetric_row_collapses_forms_without_losing_aliases() {
    let row = ToneRow::try_from_classes(CHROMATIC).expect("chromatic row");
    let family = RowFamilySet::of(&row);

    assert_eq!(family.aliases().len(), 48);
    assert_eq!(family.distinct_forms().len(), 24);
    assert!(
        family
            .distinct_forms()
            .iter()
            .enumerate()
            .all(|(index, _)| family.aliases_for_distinct_form(index).count() == 2)
    );
    for alias in family.aliases() {
        assert_eq!(alias.form, row.apply(alias.operation));
    }
}

#[test]
fn gate_matrix_rows_columns_and_reverse_edges_match_operations() {
    let source = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    for convention in [
        RowLabelConvention::FirstLastPitch,
        RowLabelConvention::OperationIndex,
    ] {
        let matrix = RowMatrix::new(&source, convention);
        assert_eq!(matrix.source(), &source);
        assert_eq!(matrix.convention(), convention);

        for row in 0..ROW_MATRIX_SIZE {
            let operation = matrix.row_operation(row).expect("matrix row");
            let form = source.apply(operation);
            assert_eq!(matrix.row(row), Some(form.classes()));
            assert_eq!(matrix.row_form(row), Some(form.clone()));
            assert_eq!(matrix.edge_labels().left()[row], form.label(convention));

            let reverse = source.apply(RowOperation::new(RowFamily::R, operation.addend));
            let matrix_row = matrix.row(row).expect("matrix row");
            assert_eq!(
                std::array::from_fn(|index| matrix_row[ROW_MATRIX_SIZE - 1 - index]),
                *reverse.classes()
            );
            assert_eq!(matrix.edge_labels().right()[row], reverse.label(convention));
        }

        for column in 0..ROW_MATRIX_SIZE {
            let operation = matrix.column_operation(column).expect("matrix column");
            let form = source.apply(operation);
            assert_eq!(matrix.column(column), Some(*form.classes()));
            assert_eq!(matrix.column_form(column), Some(form.clone()));
            assert_eq!(matrix.edge_labels().top()[column], form.label(convention));

            let reverse = source.apply(RowOperation::new(RowFamily::RI, operation.addend));
            let matrix_column = matrix.column(column).expect("matrix column");
            assert_eq!(
                std::array::from_fn(|index| matrix_column[ROW_MATRIX_SIZE - 1 - index]),
                *reverse.classes()
            );
            assert_eq!(
                matrix.edge_labels().bottom()[column],
                reverse.label(convention)
            );
        }
    }
}

#[test]
fn gate_matrix_ascii_and_data_share_coordinates_and_semantics() {
    let source = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    let matrix = RowMatrix::new(&source, RowLabelConvention::FirstLastPitch);
    let data = matrix.render_data();

    assert_eq!(data.source(), &source);
    assert_eq!(data.convention(), matrix.convention());
    assert_eq!(data.edge_labels(), matrix.edge_labels());
    assert_eq!(data.cells().len(), ROW_MATRIX_SIZE * ROW_MATRIX_SIZE);
    for row in 0..ROW_MATRIX_SIZE {
        for column in 0..ROW_MATRIX_SIZE {
            let coordinate = MatrixCoordinate::new(row, column).expect("matrix coordinate");
            let cell = data.cell(coordinate);
            assert_eq!(cell.coordinate(), coordinate);
            assert_eq!(cell, &matrix.cell(coordinate));
        }
    }

    assert!(MatrixCoordinate::new(ROW_MATRIX_SIZE, 0).is_none());
    assert!(MatrixCoordinate::new(0, ROW_MATRIX_SIZE).is_none());
    let ascii = matrix.render_ascii();
    assert!(ascii.starts_with("label-convention: first-last-pitch\nsource:  4  5"));
    assert!(ascii.contains("P4 |"));
    assert!(ascii.lines().last().is_some_and(|line| line.contains("RI")));
}
