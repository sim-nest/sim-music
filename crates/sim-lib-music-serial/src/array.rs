//! Serial arrays with independent horizontal row-order and vertical aggregate evidence.

use std::collections::BTreeMap;

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::PitchClassAlphabet;
use sim_lib_serial_core::{AggregateRule, Series, SeriesError};
use thiserror::Error;

use crate::rotate_sequence_left;

/// One horizontally validated row in a serial array.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialArrayRow {
    /// Stable row label retained in reports.
    pub label: String,
    /// Horizontal order validated independently of vertical requirements.
    pub order: Series<PitchClassAlphabet>,
}

/// One named set of column indices checked together as one vertical aggregate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnPartition {
    /// Stable partition label retained in reports.
    pub id: String,
    /// Column indices included in caller order.
    pub columns: Vec<usize>,
}

/// One named vertical aggregate requirement over array column partitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerticalAggregateRequirement {
    /// Stable requirement label retained in the report.
    pub id: String,
    /// Column partitions checked under the shared rule.
    pub partitions: Vec<ColumnPartition>,
    /// Aggregate rule enforced over every partition's flattened vertical values.
    pub rule: AggregateRule,
}

/// Coverage evidence for one set of column partitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartitionCoverageReport {
    /// Column indices named by more than one partition.
    pub duplicate_columns: Vec<usize>,
    /// Column indices not named by any partition.
    pub omitted_columns: Vec<usize>,
    /// Whether the partitions cover every column exactly once.
    pub complete: bool,
}

/// Aggregate evidence for one named array partition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregatePartitionReport {
    /// Partition label retained from the requirement.
    pub id: String,
    /// Column indices checked by this partition.
    pub columns: Vec<usize>,
    /// Flattened vertical values in row-major order.
    pub values: Vec<PitchClass>,
    /// Pitch classes that repeated in this aggregate.
    pub duplicates: Vec<PitchClass>,
    /// Pitch classes omitted from this aggregate.
    pub omissions: Vec<PitchClass>,
    /// Whether the retained rule accepted this partition.
    pub satisfied: bool,
    /// Exact rule failure when the partition did not satisfy the rule.
    pub error: Option<SeriesError>,
}

/// Aggregate evidence for one named vertical requirement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateArrayReport {
    /// Requirement label retained from the array.
    pub id: String,
    /// Partition coverage evidence over the array's columns.
    pub coverage: PartitionCoverageReport,
    /// Per-partition aggregate evidence.
    pub partitions: Vec<AggregatePartitionReport>,
    /// Whether coverage and every partition both satisfied the requirement.
    pub satisfied: bool,
}

/// One serial array with reusable vertical aggregate analyses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialArray {
    rows: Vec<SerialArrayRow>,
    vertical_requirements: Vec<VerticalAggregateRequirement>,
    column_count: usize,
}

impl SerialArray {
    /// Constructs an array after checking non-empty, equal-length horizontal rows
    /// and structurally valid vertical partitions.
    pub fn try_new(
        rows: Vec<SerialArrayRow>,
        vertical_requirements: Vec<VerticalAggregateRequirement>,
    ) -> Result<Self, SerialArrayError> {
        let Some(first_row) = rows.first() else {
            return Err(SerialArrayError::EmptyArray);
        };
        let column_count = first_row.order.order().len();
        if column_count == 0 {
            return Err(SerialArrayError::EmptyRowOrder {
                row_label: first_row.label.clone(),
            });
        }
        for row in &rows {
            if row.label.trim().is_empty() {
                return Err(SerialArrayError::EmptyRowLabel);
            }
            if row.order.order().len() != column_count {
                return Err(SerialArrayError::RowLengthMismatch {
                    row_label: row.label.clone(),
                    expected: column_count,
                    found: row.order.order().len(),
                });
            }
        }
        for requirement in &vertical_requirements {
            if requirement.id.trim().is_empty() {
                return Err(SerialArrayError::EmptyRequirementId);
            }
            for partition in &requirement.partitions {
                if partition.id.trim().is_empty() {
                    return Err(SerialArrayError::EmptyPartitionId {
                        requirement_id: requirement.id.clone(),
                    });
                }
                if partition.columns.is_empty() {
                    return Err(SerialArrayError::EmptyPartition {
                        requirement_id: requirement.id.clone(),
                        partition_id: partition.id.clone(),
                    });
                }
                for &column in &partition.columns {
                    if column >= column_count {
                        return Err(SerialArrayError::ColumnOutOfRange {
                            requirement_id: requirement.id.clone(),
                            partition_id: partition.id.clone(),
                            column,
                            column_count,
                        });
                    }
                }
            }
        }
        Ok(Self {
            rows,
            vertical_requirements,
            column_count,
        })
    }

    /// Returns the number of retained rows.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Returns the number of retained columns.
    pub fn column_count(&self) -> usize {
        self.column_count
    }

    /// Returns the horizontally validated rows.
    pub fn rows(&self) -> &[SerialArrayRow] {
        &self.rows
    }

    /// Returns the named vertical requirements.
    pub fn vertical_requirements(&self) -> &[VerticalAggregateRequirement] {
        &self.vertical_requirements
    }

    /// Returns one left-rotated copy of the array, preserving vertical requirements.
    pub fn rotate_columns(&self, steps: usize) -> Self {
        let rows = self
            .rows
            .iter()
            .map(|row| SerialArrayRow {
                label: row.label.clone(),
                order: Series::try_new(
                    row.order.alphabet().clone(),
                    row.order.rule().clone(),
                    rotate_sequence_left(row.order.order(), steps),
                )
                .expect("rotating a validated row preserves its aggregate contract"),
            })
            .collect();
        Self {
            rows,
            vertical_requirements: self.vertical_requirements.clone(),
            column_count: self.column_count,
        }
    }

    /// Materializes aggregate evidence for every retained vertical requirement.
    pub fn aggregate_reports(&self) -> Vec<AggregateArrayReport> {
        self.vertical_requirements
            .iter()
            .map(|requirement| self.aggregate_report(requirement))
            .collect()
    }

    /// Reports all-partition evidence for contiguous blocks of `block_width` columns.
    pub fn all_partition_report(
        &self,
        block_width: usize,
    ) -> Result<AggregateArrayReport, SerialArrayError> {
        if block_width == 0 {
            return Err(SerialArrayError::ZeroBlockWidth);
        }
        if !self.column_count.is_multiple_of(block_width) {
            return Err(SerialArrayError::BlockWidthMismatch {
                block_width,
                column_count: self.column_count,
            });
        }
        let partitions = (0..self.column_count / block_width)
            .map(|index| ColumnPartition {
                id: format!("partition/{}", index + 1),
                columns: ((index * block_width)..((index + 1) * block_width)).collect(),
            })
            .collect::<Vec<_>>();
        Ok(self.aggregate_report(&VerticalAggregateRequirement {
            id: format!("all-partition/{block_width}"),
            partitions,
            rule: AggregateRule::exhaustive_exactly_once(),
        }))
    }

    fn aggregate_report(&self, requirement: &VerticalAggregateRequirement) -> AggregateArrayReport {
        let coverage = coverage_report(self.column_count, &requirement.partitions);
        let alphabet = PitchClassAlphabet::try_new().expect("canonical pitch-class alphabet");
        let partitions = requirement
            .partitions
            .iter()
            .map(|partition| {
                let values = self.flatten_partition(&partition.columns);
                let duplicates = duplicate_pitch_classes(&values);
                let omissions = omitted_pitch_classes(&values);
                match Series::try_new(alphabet.clone(), requirement.rule.clone(), values.clone()) {
                    Ok(_) => AggregatePartitionReport {
                        id: partition.id.clone(),
                        columns: partition.columns.clone(),
                        values,
                        duplicates,
                        omissions,
                        satisfied: true,
                        error: None,
                    },
                    Err(error) => AggregatePartitionReport {
                        id: partition.id.clone(),
                        columns: partition.columns.clone(),
                        values,
                        duplicates,
                        omissions,
                        satisfied: false,
                        error: Some(error),
                    },
                }
            })
            .collect::<Vec<_>>();
        let satisfied = coverage.complete && partitions.iter().all(|partition| partition.satisfied);
        AggregateArrayReport {
            id: requirement.id.clone(),
            coverage,
            partitions,
            satisfied,
        }
    }

    fn flatten_partition(&self, columns: &[usize]) -> Vec<PitchClass> {
        let mut values = Vec::with_capacity(self.rows.len() * columns.len());
        for row in &self.rows {
            for &column in columns {
                values.push(row.order.order()[column]);
            }
        }
        values
    }
}

/// Failure while constructing or deriving one serial array analysis.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum SerialArrayError {
    /// The array declared no horizontal rows.
    #[error("serial array must contain at least one row")]
    EmptyArray,
    /// One row label was empty.
    #[error("serial array rows must carry a non-empty label")]
    EmptyRowLabel,
    /// One row carried no horizontal values.
    #[error("serial array row {row_label:?} must contain at least one value")]
    EmptyRowOrder {
        /// Label of the empty row.
        row_label: String,
    },
    /// Rows did not agree on one shared column count.
    #[error("serial array row {row_label:?} has length {found}; expected {expected}")]
    RowLengthMismatch {
        /// Label of the row with the wrong length.
        row_label: String,
        /// Shared array width established by the first row.
        expected: usize,
        /// Row width found on the offending row.
        found: usize,
    },
    /// One vertical requirement label was empty.
    #[error("serial array vertical requirements must carry a non-empty id")]
    EmptyRequirementId,
    /// One partition label was empty.
    #[error("serial array requirement {requirement_id:?} contains an empty partition id")]
    EmptyPartitionId {
        /// Requirement that owned the malformed partition.
        requirement_id: String,
    },
    /// One partition named no columns.
    #[error(
        "serial array requirement {requirement_id:?} partition {partition_id:?} must name at least one column"
    )]
    EmptyPartition {
        /// Requirement that owned the malformed partition.
        requirement_id: String,
        /// Partition that named no columns.
        partition_id: String,
    },
    /// One partition named a column outside the array width.
    #[error(
        "serial array requirement {requirement_id:?} partition {partition_id:?} names column {column} outside 0..{column_count}"
    )]
    ColumnOutOfRange {
        /// Requirement that owned the malformed partition.
        requirement_id: String,
        /// Partition that named the bad column.
        partition_id: String,
        /// Rejected column index.
        column: usize,
        /// Shared array width.
        column_count: usize,
    },
    /// An all-partition report named a zero-width block.
    #[error("all-partition block width must be at least 1")]
    ZeroBlockWidth,
    /// An all-partition block width did not divide the array width.
    #[error("all-partition block width {block_width} does not divide column count {column_count}")]
    BlockWidthMismatch {
        /// Requested contiguous block width.
        block_width: usize,
        /// Shared array width.
        column_count: usize,
    },
}

fn coverage_report(column_count: usize, partitions: &[ColumnPartition]) -> PartitionCoverageReport {
    let mut counts = vec![0usize; column_count];
    for partition in partitions {
        for &column in &partition.columns {
            counts[column] += 1;
        }
    }
    let duplicate_columns = counts
        .iter()
        .enumerate()
        .filter_map(|(column, count)| (*count > 1).then_some(column))
        .collect::<Vec<_>>();
    let omitted_columns = counts
        .iter()
        .enumerate()
        .filter_map(|(column, count)| (*count == 0).then_some(column))
        .collect::<Vec<_>>();
    PartitionCoverageReport {
        complete: duplicate_columns.is_empty() && omitted_columns.is_empty(),
        duplicate_columns,
        omitted_columns,
    }
}

fn duplicate_pitch_classes(values: &[PitchClass]) -> Vec<PitchClass> {
    let mut counts = BTreeMap::new();
    for &value in values {
        *counts.entry(value).or_insert(0usize) += 1;
    }
    counts
        .into_iter()
        .filter_map(|(pitch_class, count)| (count > 1).then_some(pitch_class))
        .collect()
}

fn omitted_pitch_classes(values: &[PitchClass]) -> Vec<PitchClass> {
    PitchClassAlphabet::try_new()
        .expect("canonical pitch-class alphabet")
        .classes()
        .iter()
        .copied()
        .filter(|pitch_class| !values.contains(pitch_class))
        .collect()
}
