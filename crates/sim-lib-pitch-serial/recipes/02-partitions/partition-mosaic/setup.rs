use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{
    BlockOrder, OrderKind, ToneRow, analyze_interlocking_partitions, analyze_mosaic,
    analyze_partition_aggregate_coverage, analyze_partition_similarity, try_partition, verticalize,
};

pub fn partition_mosaic() -> Result<(), Box<dyn std::error::Error>> {
    let row = ToneRow::try_from_classes([
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
    ])?;

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
    )?;
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
    )?;
    let trichordal = try_partition(
        vec![vec![0, 1, 2], vec![3, 4, 5], vec![6, 7, 8], vec![9, 10, 11]],
        BlockOrder::total(),
    )?;
    let tetrachordal = try_partition(
        vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7], vec![8, 9, 10, 11]],
        BlockOrder::total(),
    )?;
    let hexachordal = try_partition(
        vec![vec![0, 1, 2, 3, 4, 5], vec![6, 7, 8, 9, 10, 11]],
        BlockOrder::unordered(),
    )?;

    let similarity = analyze_partition_similarity(&dyadic, &dyadic_reblocked);
    assert!(similarity.same_block_size_multiset);
    assert!(!similarity.same_order_contract);
    assert_eq!(similarity.overlap_matrix[0][0], 2);

    let vertical = verticalize(&row, &dyadic);
    assert_eq!(vertical.slices.len(), 6);
    assert!(vertical.aggregate_coverage.complete);

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

    let interleave_a = try_partition(
        vec![vec![0, 2, 4, 6, 8, 10], vec![1, 3, 5, 7, 9, 11]],
        BlockOrder::unordered(),
    )?;
    let interleave_b = try_partition(
        vec![vec![0, 1, 4, 5, 8, 9], vec![2, 3, 6, 7, 10, 11]],
        BlockOrder::unordered(),
    )?;
    let report = analyze_interlocking_partitions(&interleave_a, &interleave_b);
    assert!(report.is_interlocking);
    assert_eq!(report.overlap_matrix, vec![vec![3, 3], vec![3, 3]]);
    Ok(())
}
