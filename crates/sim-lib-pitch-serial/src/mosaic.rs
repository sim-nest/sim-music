//! Partition mosaics over one strict tone row.

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_set::PitchClassMask;

use crate::{
    AggregateCoverageReport, BlockOrder, RowPartition, ToneRow, analyze_aggregate_coverage,
};

/// One partition block lifted into pitch-space for mosaic inspection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MosaicBlock {
    /// Source partition index in caller order.
    pub partition_index: usize,
    /// Source block index within the partition.
    pub block_index: usize,
    /// Source partition ordering contract.
    pub order: BlockOrder,
    /// Row ordinals carried by this block.
    pub ordinals: Vec<u8>,
    /// Pitch classes reached on the source row.
    pub pitch_classes: Vec<PitchClass>,
    /// Unordered pitch-class content of the block.
    pub mask: PitchClassMask,
}

/// A combined view of several validated partitions over one row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MosaicReport {
    /// Every lifted block from every supplied partition.
    pub blocks: Vec<MosaicBlock>,
    /// Aggregate coverage across all lifted blocks.
    pub aggregate_coverage: AggregateCoverageReport,
}

/// Lifts several validated partitions into one combined mosaic report.
pub fn analyze_mosaic(row: &ToneRow, partitions: &[RowPartition]) -> MosaicReport {
    let mut blocks = Vec::new();
    let mut masks = Vec::new();
    for (partition_index, partition) in partitions.iter().enumerate() {
        for (block_index, block) in partition.blocks().iter().enumerate() {
            let pitch_classes = block.pitch_classes(row);
            let mask = PitchClassMask::from_pitch_classes(&pitch_classes);
            masks.push(mask);
            blocks.push(MosaicBlock {
                partition_index,
                block_index,
                order: partition.order(),
                ordinals: block.ordinals().to_vec(),
                pitch_classes,
                mask,
            });
        }
    }
    MosaicReport {
        blocks,
        aggregate_coverage: analyze_aggregate_coverage(
            PitchClassMask::from_pitch_classes(row.classes()),
            &masks,
        ),
    }
}
