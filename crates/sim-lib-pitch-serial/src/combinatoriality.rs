//! Combinatorial partner analysis for strict tone rows.

use sim_lib_discrete_comb::permutations;
use sim_lib_pitch_set::PitchClassMask;

use crate::{RowError, RowFamilySet, RowOperation, ToneRow};

const COMBINATORIAL_PARTITIONS: [usize; 4] = [2, 3, 4, 6];

/// One paired source/partner block that covers the full aggregate exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombinatorialBlockEvidence {
    /// Source block index in contiguous partition order.
    pub source_block_index: usize,
    /// Partner block index in contiguous partition order.
    pub partner_block_index: usize,
    /// Source row ordinals contributing the source block.
    pub source_ordinals: Vec<u8>,
    /// Partner row ordinals contributing the complementary block.
    pub partner_ordinals: Vec<u8>,
    /// The source block's pitch-class mask.
    pub source: PitchClassMask,
    /// The partner block's complementary pitch-class mask.
    pub complement: PitchClassMask,
}

/// One successful contiguous equal-cell partition witnessing combinatoriality.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombinatorialPartition {
    /// The block size in row positions.
    pub block_size: usize,
    /// The permutation mapping source blocks onto partner blocks.
    pub partner_block_order: Vec<usize>,
    /// Exact complementary evidence for every paired block.
    pub blocks: Vec<CombinatorialBlockEvidence>,
}

/// One partner row form that is combinatorial with the source row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombinatorialPartner {
    /// The exact row-family operation producing the partner row.
    pub operation: RowOperation,
    /// One representative source block mask.
    pub source: PitchClassMask,
    /// The complementary partner block mask matching `source`.
    pub complement: PitchClassMask,
    /// The validated equal-cell partition witness.
    pub partition: CombinatorialPartition,
}

/// Complete combinatorial partner evidence for a strict row.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CombinatorialityReport {
    /// Every detected prime, inversional, retrograde, or retrograde-inversional partner.
    pub partners: Vec<CombinatorialPartner>,
}

/// Detects combinatorial partners over the supported equal-cell partitions.
pub fn analyze_combinatoriality(row: &ToneRow) -> CombinatorialityReport {
    let family = RowFamilySet::of(row);
    let mut partners = Vec::new();
    for alias in family.aliases() {
        for partition_size in COMBINATORIAL_PARTITIONS {
            if let Ok(Some(partner)) =
                analyze_combinatoriality_partition(row, alias.operation, partition_size)
            {
                partners.push(partner);
            }
        }
    }
    CombinatorialityReport { partners }
}

/// Detects combinatoriality for one partner operation and one supported partition size.
pub fn analyze_combinatoriality_partition(
    row: &ToneRow,
    operation: RowOperation,
    partition_size: usize,
) -> Result<Option<CombinatorialPartner>, RowError> {
    validate_partition_size(partition_size)?;
    let source_blocks = contiguous_masks(row, partition_size);
    let partner_row = row.apply(operation).into_row();
    let partner_blocks = contiguous_masks(&partner_row, partition_size);
    let block_count = source_blocks.len();
    let aggregate = PitchClassMask::from_pitch_classes(row.classes());

    for partner_block_order in permutations(block_count) {
        let blocks = source_blocks
            .iter()
            .enumerate()
            .map(|(source_block_index, source)| {
                let partner_block_index = partner_block_order[source_block_index];
                let complement = partner_blocks[partner_block_index];
                let start = source_block_index * partition_size;
                let partner_start = partner_block_index * partition_size;
                CombinatorialBlockEvidence {
                    source_block_index,
                    partner_block_index,
                    source_ordinals: (start..start + partition_size)
                        .map(|ordinal| ordinal as u8)
                        .collect(),
                    partner_ordinals: (partner_start..partner_start + partition_size)
                        .map(|ordinal| ordinal as u8)
                        .collect(),
                    source: *source,
                    complement,
                }
            })
            .collect::<Vec<_>>();
        let exact_cover = blocks.iter().all(|block| {
            block.source.is_disjoint_from(block.complement)
                && block.source.union(block.complement) == aggregate
        });
        if exact_cover {
            let partition = CombinatorialPartition {
                block_size: partition_size,
                partner_block_order,
                blocks,
            };
            return Ok(Some(CombinatorialPartner {
                operation,
                source: partition.blocks[0].source,
                complement: partition.blocks[0].complement,
                partition,
            }));
        }
    }

    Ok(None)
}

fn validate_partition_size(size: usize) -> Result<(), RowError> {
    if COMBINATORIAL_PARTITIONS.contains(&size) {
        Ok(())
    } else {
        Err(RowError::InvalidPartitionSize { size })
    }
}

fn contiguous_masks(row: &ToneRow, partition_size: usize) -> Vec<PitchClassMask> {
    row.classes()
        .chunks(partition_size)
        .map(PitchClassMask::from_pitch_classes)
        .collect()
}
