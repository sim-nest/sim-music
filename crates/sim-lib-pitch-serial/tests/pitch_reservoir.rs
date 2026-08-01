use std::collections::BTreeSet;

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{
    AffinePitchMap, BlockOrder, BlockProjectionSource, OrderKind, PitchInvariant,
    PitchTransformOutput, RowError, ToneRow, multiply_partitions, try_partition,
};
use sim_lib_serial_core::OrdinalMapError;

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

fn values(classes: &[PitchClass; 12]) -> [u8; 12] {
    classes.map(PitchClass::value)
}

#[test]
fn pitch_reservoir_rotations_and_validated_ordinal_permutations_preserve_rows() {
    let source = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");

    let rotated = source.rotate(14);
    assert_eq!(
        values(rotated.classes()),
        [7, 1, 6, 3, 8, 2, 11, 0, 9, 10, 4, 5]
    );

    let permuted = source
        .try_permute_ordinals(vec![11, 0, 10, 1, 9, 2, 8, 3, 7, 4, 6, 5])
        .expect("validated ordinal permutation");
    assert_eq!(
        values(permuted.classes()),
        [10, 4, 9, 5, 0, 7, 11, 1, 2, 6, 8, 3]
    );

    assert_eq!(
        source.try_permute_ordinals(vec![0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
        Err(RowError::OrdinalMap(OrdinalMapError::DuplicateInput {
            input: 0,
            first_output: 0,
            duplicate_output: 1,
        }))
    );
}

#[test]
fn pitch_reservoir_affine_bijections_m1_m5_m7_m11_stay_strict_rows() {
    let source = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    let inverses = [(1, 1), (5, 5), (7, 7), (11, 11)];

    for (multiplier, inverse_multiplier) in inverses {
        for addend in 0..12 {
            let transform = AffinePitchMap::new(multiplier, addend);
            assert!(
                transform.is_bijective(),
                "M{multiplier} should be bijective"
            );

            let PitchTransformOutput::Row(transformed) = transform.apply(&source) else {
                panic!("M{multiplier}T{addend} should preserve row identity");
            };
            assert_eq!(
                transformed
                    .classes()
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .len(),
                12,
                "M{multiplier}T{addend} must keep all twelve pitch classes"
            );

            let inverse_addend =
                ((12 - (u16::from(inverse_multiplier) * u16::from(addend) % 12)) % 12) as u8;
            let PitchTransformOutput::Row(restored) =
                AffinePitchMap::new(inverse_multiplier, inverse_addend).apply(&transformed)
            else {
                panic!("inverse of M{multiplier}T{addend} should preserve row identity");
            };
            assert_eq!(
                restored, source,
                "inverse law failed for M{multiplier}T{addend}"
            );
        }
    }
}

#[test]
fn pitch_reservoir_non_bijective_affine_maps_return_ordered_reservoirs() {
    let source = ToneRow::try_from_classes(CHROMATIC).expect("chromatic row");
    let PitchTransformOutput::Reservoir(reservoir) = AffinePitchMap::new(2, 1).apply(&source)
    else {
        panic!("non-bijective affine map must not return a strict row");
    };

    assert_eq!(reservoir.blocks.len(), 6);
    assert_eq!(
        reservoir
            .blocks
            .iter()
            .map(|block| block
                .pitch_classes
                .iter()
                .map(|pitch| pitch.value())
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        vec![
            vec![1, 1],
            vec![3, 3],
            vec![5, 5],
            vec![7, 7],
            vec![9, 9],
            vec![11, 11],
        ]
    );
    assert!(!reservoir.invariant_delta.retains_total_order);
    assert!(!reservoir.invariant_delta.retains_aggregate_identity);
    assert_eq!(
        reservoir.invariant_delta.relaxed_invariants,
        vec![
            PitchInvariant::TotalOrder,
            PitchInvariant::AggregateIdentity
        ]
    );
    assert!(matches!(
        &reservoir.provenance[0].source,
        BlockProjectionSource::OrdinalCollapse {
            source_ordinals,
            target_pitch_class,
        } if source_ordinals == &vec![0, 6] && *target_pitch_class == PitchClass::CS
    ));
}

#[test]
fn pitch_reservoir_block_multiplication_tracks_interval_projection_provenance() {
    let row = ToneRow::try_from_classes(CHROMATIC).expect("chromatic row");
    let anchors = try_partition(
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
    .expect("anchor dyads");
    let intervals = try_partition(
        vec![
            vec![0, 2],
            vec![1, 3],
            vec![4, 6],
            vec![5, 7],
            vec![8, 10],
            vec![9, 11],
        ],
        BlockOrder::new(OrderKind::Total, OrderKind::Partial),
    )
    .expect("interval dyads");

    let product = multiply_partitions(&row, &anchors, &intervals);
    assert_eq!(product.reservoir.blocks.len(), 36);
    assert_eq!(
        product.reservoir.blocks[0]
            .pitch_classes
            .iter()
            .map(|pitch| pitch.value())
            .collect::<Vec<_>>(),
        vec![0, 2, 1, 3]
    );
    assert_eq!(product.reservoir.blocks[0].mask.count_bits(), 4);
    assert!(!product.reservoir.invariant_delta.retains_aggregate_identity);
    assert!(matches!(
        &product.reservoir.provenance[0].source,
        BlockProjectionSource::BlockMultiplication {
            anchor_block_index,
            anchor_ordinals,
            interval_block_index,
            interval_ordinals,
            interval_content,
        } if *anchor_block_index == 0
            && anchor_ordinals == &vec![0, 1]
            && *interval_block_index == 0
            && interval_ordinals == &vec![0, 2]
            && interval_content == &vec![0, 2]
    ));
}
