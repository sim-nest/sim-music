//! Verticalized views of row partitions without chord reification.

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_set::PitchClassMask;

use crate::{
    AggregateCoverageReport, BlockOrder, RowPartition, ToneRow,
    analyze_partition_aggregate_coverage,
};

/// One unordered vertical collection extracted from a partition block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerticalSlice {
    /// Source block index in the validated partition.
    pub block_index: usize,
    /// Row ordinals contributing to the slice.
    pub ordinals: Vec<u8>,
    /// Exact pitch classes contributed by those ordinals.
    pub pitch_classes: Vec<PitchClass>,
    /// Unordered pitch-class content of the slice.
    pub mask: PitchClassMask,
}

/// A partition rendered as a set of vertical collections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerticalCollection {
    /// The partition's ordering contract.
    pub order: BlockOrder,
    /// One vertical slice per partition block.
    pub slices: Vec<VerticalSlice>,
    /// Aggregate pitch-class coverage across all slices.
    pub aggregate_coverage: AggregateCoverageReport,
}

/// Produces vertical collection data from one validated partition.
pub fn verticalize(row: &ToneRow, partition: &RowPartition) -> VerticalCollection {
    let slices = partition
        .blocks()
        .iter()
        .enumerate()
        .map(|(block_index, block)| {
            let pitch_classes = block.pitch_classes(row);
            VerticalSlice {
                block_index,
                ordinals: block.ordinals().to_vec(),
                mask: PitchClassMask::from_pitch_classes(&pitch_classes),
                pitch_classes,
            }
        })
        .collect();
    VerticalCollection {
        order: partition.order(),
        slices,
        aggregate_coverage: analyze_partition_aggregate_coverage(row, partition),
    }
}
