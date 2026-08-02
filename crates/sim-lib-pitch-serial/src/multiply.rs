//! Boulez-style block multiplication over row partitions.

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_set::PitchClassMask;

use crate::{
    BlockProjection, BlockProjectionSource, OrderedPitchBlock, PitchReservoir, RowPartition,
    ToneRow,
};

/// A block-product reservoir with its exact source partitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockProductReservoir {
    /// The ordered reservoir produced by block multiplication.
    pub reservoir: PitchReservoir,
    /// Partition providing anchor pitches.
    pub anchor_partition: RowPartition,
    /// Partition providing interval content.
    pub interval_partition: RowPartition,
}

/// Projects each interval block onto each anchor block in declaration order.
pub fn multiply_partitions(
    row: &ToneRow,
    anchor_partition: &RowPartition,
    interval_partition: &RowPartition,
) -> BlockProductReservoir {
    let mut blocks = Vec::new();
    let mut provenance = Vec::new();
    for (anchor_block_index, anchor_block) in anchor_partition.blocks().iter().enumerate() {
        let anchor_pitches = anchor_block.pitch_classes(row);
        for (interval_block_index, interval_block) in interval_partition.blocks().iter().enumerate()
        {
            let interval_pitches = interval_block.pitch_classes(row);
            let interval_content = interval_content(&interval_pitches);
            let pitch_classes = anchor_pitches
                .iter()
                .flat_map(|anchor_pitch| {
                    interval_content
                        .iter()
                        .map(move |interval| anchor_pitch.transpose(i32::from(*interval)))
                })
                .collect::<Vec<_>>();
            let block_index = blocks.len();
            blocks.push(OrderedPitchBlock {
                mask: PitchClassMask::from_pitch_classes(&pitch_classes),
                pitch_classes,
            });
            provenance.push(BlockProjection {
                block_index,
                source: BlockProjectionSource::BlockMultiplication {
                    anchor_block_index,
                    anchor_ordinals: anchor_block.ordinals().to_vec(),
                    interval_block_index,
                    interval_ordinals: interval_block.ordinals().to_vec(),
                    interval_content,
                },
            });
        }
    }
    BlockProductReservoir {
        reservoir: PitchReservoir::new(blocks, provenance),
        anchor_partition: anchor_partition.clone(),
        interval_partition: interval_partition.clone(),
    }
}

fn interval_content(block: &[PitchClass]) -> Vec<u8> {
    let first = block[0];
    block
        .iter()
        .map(|pitch_class| pitch_class.value().wrapping_add(12) - first.value())
        .map(|delta| delta % 12)
        .collect()
}
