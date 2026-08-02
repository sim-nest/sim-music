use std::num::NonZeroU16;

use sim_lib_music_core::{PitchClass, Time};
use sim_lib_music_serial::{
    ColumnPartition, SerialArray, SerialArrayRow, TimePointRow, TimePointSystem,
    VerticalAggregateRequirement,
};
use sim_lib_pitch_serial::PitchClassAlphabet;
use sim_lib_serial_core::{AggregateRule, Series};

fn pitch_series(values: &[PitchClass]) -> Series<PitchClassAlphabet> {
    Series::try_new(
        PitchClassAlphabet::try_new().expect("pitch alphabet"),
        AggregateRule::exhaustive_exactly_once(),
        values.to_vec(),
    )
    .expect("pitch series")
}

fn pitch_series_with_rule(
    values: &[PitchClass],
    rule: AggregateRule,
) -> Series<PitchClassAlphabet> {
    Series::try_new(
        PitchClassAlphabet::try_new().expect("pitch alphabet"),
        rule,
        values.to_vec(),
    )
    .expect("pitch series with rule")
}

pub fn arrays_time_points() -> Result<(), Box<dyn std::error::Error>> {
    let pitch_alphabet = PitchClassAlphabet::try_new()?;
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
                    )?,
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
                    )?,
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
    )?;
    assert_eq!(array.row_count(), 2);
    assert!(array.aggregate_reports()[0].satisfied);

    let rotated_array = SerialArray::try_new(
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
    )?;
    let report = rotated_array.all_partition_report(3)?;
    assert!(report.satisfied);
    assert_eq!(report.partitions.len(), 4);

    let system = TimePointSystem {
        modulus: NonZeroU16::new(12).expect("non-zero modulus"),
        unit: Time::new(1, 12),
    };
    let row = TimePointRow::try_new(&system, (0..12).collect())?;
    let rotated = row.rotate(5)?;
    assert_eq!(rotated.order(), &[5, 6, 7, 8, 9, 10, 11, 0, 1, 2, 3, 4]);
    Ok(())
}
