use std::num::NonZeroU16;

use sim_lib_music_core::{PitchClass, Time};
use sim_lib_serial_core::{AggregateRule, Series, SeriesError};

use crate::{
    ColumnPartition, NestedSerialValue, NestingError, NestingLimits, SerialArray, SerialArrayError,
    SerialArrayRow, TimePointRow, TimePointSystem, VerticalAggregateRequirement, expand_nested,
    rotate_sequence_left,
};

fn pitch_series(values: &[PitchClass]) -> Series<sim_lib_pitch_serial::PitchClassAlphabet> {
    Series::try_new(
        sim_lib_pitch_serial::PitchClassAlphabet::try_new().expect("pitch alphabet"),
        AggregateRule::exhaustive_exactly_once(),
        values.to_vec(),
    )
    .expect("pitch series")
}

fn pitch_series_with_rule(
    values: &[PitchClass],
    rule: AggregateRule,
) -> Series<sim_lib_pitch_serial::PitchClassAlphabet> {
    Series::try_new(
        sim_lib_pitch_serial::PitchClassAlphabet::try_new().expect("pitch alphabet"),
        rule,
        values.to_vec(),
    )
    .expect("pitch series with rule")
}

#[test]
fn serial_array_validates_horizontal_rows_and_vertical_aggregate_requirements_independently() {
    let pitch_alphabet =
        sim_lib_pitch_serial::PitchClassAlphabet::try_new().expect("pitch alphabet");
    let array = SerialArray::try_new(
        vec![
            SerialArrayRow {
                label: "row/a".to_owned(),
                order: pitch_series_with_rule(
                    &[
                        PitchClass::C,
                        PitchClass::CS,
                        PitchClass::D,
                        PitchClass::DS,
                        PitchClass::E,
                        PitchClass::F,
                    ],
                    AggregateRule::declared_omissions(
                        &pitch_alphabet,
                        [
                            PitchClass::FS,
                            PitchClass::G,
                            PitchClass::GS,
                            PitchClass::A,
                            PitchClass::AS,
                            PitchClass::B,
                        ],
                    )
                    .expect("hexachord rule"),
                ),
            },
            SerialArrayRow {
                label: "row/b".to_owned(),
                order: pitch_series_with_rule(
                    &[
                        PitchClass::FS,
                        PitchClass::G,
                        PitchClass::GS,
                        PitchClass::A,
                        PitchClass::AS,
                        PitchClass::B,
                    ],
                    AggregateRule::declared_omissions(
                        &pitch_alphabet,
                        [
                            PitchClass::C,
                            PitchClass::CS,
                            PitchClass::D,
                            PitchClass::DS,
                            PitchClass::E,
                            PitchClass::F,
                        ],
                    )
                    .expect("hexachord rule"),
                ),
            },
        ],
        vec![VerticalAggregateRequirement {
            id: "aggregate/two-row".to_owned(),
            partitions: vec![ColumnPartition {
                id: "hexachord-pair".to_owned(),
                columns: vec![0, 1, 2, 3, 4, 5],
            }],
            rule: AggregateRule::exhaustive_exactly_once(),
        }],
    )
    .expect("two-row combinatorial array");

    let report = &array.aggregate_reports()[0];
    assert_eq!(array.row_count(), 2);
    assert_eq!(array.column_count(), 6);
    assert!(report.coverage.complete);
    assert!(report.satisfied);
    assert!(report.partitions[0].duplicates.is_empty());
    assert!(report.partitions[0].omissions.is_empty());
}

#[test]
fn all_partition_report_tracks_partition_and_coverage_evidence() {
    let array = SerialArray::try_new(
        vec![
            SerialArrayRow {
                label: "row/1".to_owned(),
                order: pitch_series(&[
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
                ]),
            },
            SerialArrayRow {
                label: "row/2".to_owned(),
                order: pitch_series(&[
                    PitchClass::DS,
                    PitchClass::E,
                    PitchClass::F,
                    PitchClass::FS,
                    PitchClass::G,
                    PitchClass::GS,
                    PitchClass::A,
                    PitchClass::AS,
                    PitchClass::B,
                    PitchClass::C,
                    PitchClass::CS,
                    PitchClass::D,
                ]),
            },
            SerialArrayRow {
                label: "row/3".to_owned(),
                order: pitch_series(&[
                    PitchClass::FS,
                    PitchClass::G,
                    PitchClass::GS,
                    PitchClass::A,
                    PitchClass::AS,
                    PitchClass::B,
                    PitchClass::C,
                    PitchClass::CS,
                    PitchClass::D,
                    PitchClass::DS,
                    PitchClass::E,
                    PitchClass::F,
                ]),
            },
            SerialArrayRow {
                label: "row/4".to_owned(),
                order: pitch_series(&[
                    PitchClass::A,
                    PitchClass::AS,
                    PitchClass::B,
                    PitchClass::C,
                    PitchClass::CS,
                    PitchClass::D,
                    PitchClass::DS,
                    PitchClass::E,
                    PitchClass::F,
                    PitchClass::FS,
                    PitchClass::G,
                    PitchClass::GS,
                ]),
            },
        ],
        vec![],
    )
    .expect("all-partition array");

    let report = array.all_partition_report(3).expect("all-partition report");
    assert_eq!(report.id, "all-partition/3");
    assert!(report.coverage.complete);
    assert!(report.satisfied);
    assert_eq!(report.partitions.len(), 4);
    assert!(
        report
            .partitions
            .iter()
            .all(|partition| partition.satisfied)
    );
    assert!(
        report
            .partitions
            .iter()
            .all(|partition| partition.duplicates.is_empty() && partition.omissions.is_empty())
    );
}

#[test]
fn time_point_rows_keep_onset_order_separate_from_duration_series() {
    let system = TimePointSystem {
        modulus: NonZeroU16::new(12).expect("non-zero modulus"),
        unit: Time::new(1, 12),
    };
    let row = TimePointRow::try_new(&system, (0..12).collect()).expect("time-point row");

    assert_eq!(row.order(), &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    assert_eq!(
        row.onsets(&system).expect("onsets"),
        (0..12)
            .map(|index| Time::new(i64::from(index), 12))
            .collect::<Vec<_>>()
    );

    let rotated = row.rotate(5).expect("rotated time-point row");
    assert_eq!(rotated.order(), &[5, 6, 7, 8, 9, 10, 11, 0, 1, 2, 3, 4]);
}

#[test]
fn rotation_and_nesting_respect_explicit_limits() {
    assert_eq!(rotate_sequence_left(&[0, 1, 2, 3], 5), vec![1, 2, 3, 0]);

    let nested = vec![
        NestedSerialValue::Value("a"),
        NestedSerialValue::Group(vec![
            NestedSerialValue::Value("b"),
            NestedSerialValue::Group(vec![
                NestedSerialValue::Value("c"),
                NestedSerialValue::Value("d"),
            ]),
        ]),
    ];
    let expansion = expand_nested(
        &nested,
        NestingLimits {
            max_depth: 3,
            max_output: 4,
        },
    )
    .expect("bounded nesting");
    assert_eq!(expansion.depth_reached, 3);
    assert_eq!(expansion.values, vec!["a", "b", "c", "d"]);

    assert_eq!(
        expand_nested(
            &nested,
            NestingLimits {
                max_depth: 2,
                max_output: 4,
            },
        ),
        Err(NestingError::DepthExceeded {
            depth: 3,
            max_depth: 2,
        })
    );
}

#[test]
fn malformed_arrays_and_time_point_rows_fail_closed() {
    let pitch_alphabet =
        sim_lib_pitch_serial::PitchClassAlphabet::try_new().expect("pitch alphabet");
    assert_eq!(
        SerialArray::try_new(
            vec![
                SerialArrayRow {
                    label: "row/a".to_owned(),
                    order: pitch_series_with_rule(
                        &[PitchClass::C, PitchClass::CS, PitchClass::D],
                        AggregateRule::declared_omissions(
                            &pitch_alphabet,
                            [
                                PitchClass::DS,
                                PitchClass::E,
                                PitchClass::F,
                                PitchClass::FS,
                                PitchClass::G,
                                PitchClass::GS,
                                PitchClass::A,
                                PitchClass::AS,
                                PitchClass::B,
                            ],
                        )
                        .expect("omission rule"),
                    ),
                },
                SerialArrayRow {
                    label: "row/b".to_owned(),
                    order: pitch_series_with_rule(
                        &[PitchClass::DS, PitchClass::E],
                        AggregateRule::declared_omissions(
                            &pitch_alphabet,
                            [
                                PitchClass::C,
                                PitchClass::CS,
                                PitchClass::D,
                                PitchClass::F,
                                PitchClass::FS,
                                PitchClass::G,
                                PitchClass::GS,
                                PitchClass::A,
                                PitchClass::AS,
                                PitchClass::B,
                            ],
                        )
                        .expect("omission rule"),
                    ),
                },
            ],
            vec![],
        ),
        Err(SerialArrayError::RowLengthMismatch {
            row_label: "row/b".to_owned(),
            expected: 3,
            found: 2,
        })
    );

    let malformed = SerialArray::try_new(
        vec![SerialArrayRow {
            label: "row/solo".to_owned(),
            order: pitch_series_with_rule(
                &[PitchClass::C, PitchClass::CS, PitchClass::D, PitchClass::DS],
                AggregateRule::declared_omissions(
                    &pitch_alphabet,
                    [
                        PitchClass::E,
                        PitchClass::F,
                        PitchClass::FS,
                        PitchClass::G,
                        PitchClass::GS,
                        PitchClass::A,
                        PitchClass::AS,
                        PitchClass::B,
                    ],
                )
                .expect("omission rule"),
            ),
        }],
        vec![VerticalAggregateRequirement {
            id: "bad/coverage".to_owned(),
            partitions: vec![
                ColumnPartition {
                    id: "left".to_owned(),
                    columns: vec![0, 1],
                },
                ColumnPartition {
                    id: "right".to_owned(),
                    columns: vec![1, 2],
                },
            ],
            rule: AggregateRule::no_repeat(),
        }],
    )
    .expect("malformed coverage array");
    let report = &malformed.aggregate_reports()[0];
    assert_eq!(report.coverage.duplicate_columns, vec![1]);
    assert_eq!(report.coverage.omitted_columns, vec![3]);
    assert!(!report.satisfied);

    let system = TimePointSystem {
        modulus: NonZeroU16::new(12).expect("non-zero modulus"),
        unit: Time::new(1, 12),
    };
    assert!(matches!(
        TimePointRow::try_new(&system, vec![0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
        Err(crate::TimePointError::Series(
            SeriesError::MultiplicityMismatch { .. }
        ))
    ));
}
