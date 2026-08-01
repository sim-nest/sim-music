//! Validated row partitions and ordinal overlap analysis.

use std::collections::BTreeMap;

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_set::PitchClassMask;

use crate::{RowError, ToneRow};

/// How strongly a partition treats order at one structural level.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OrderKind {
    /// Relative order is fully significant.
    Total,
    /// Some order matters, but the structure is not a strict sequence.
    Partial,
    /// Order is intentionally not part of the claim.
    Absent,
}

/// The ordering contract for a row partition within and between its blocks.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BlockOrder {
    /// Whether ordinal order matters inside each block.
    pub within_blocks: OrderKind,
    /// Whether the block list itself is ordered.
    pub between_blocks: OrderKind,
}

impl BlockOrder {
    /// Creates an explicit ordering contract.
    pub const fn new(within_blocks: OrderKind, between_blocks: OrderKind) -> Self {
        Self {
            within_blocks,
            between_blocks,
        }
    }

    /// Marks both block-internal and block-to-block order as total.
    pub const fn total() -> Self {
        Self::new(OrderKind::Total, OrderKind::Total)
    }

    /// Marks block-internal order as total but block-to-block order as partial.
    pub const fn partially_ordered_blocks() -> Self {
        Self::new(OrderKind::Total, OrderKind::Partial)
    }

    /// Marks both block-internal and block-to-block order as intentionally absent.
    pub const fn unordered() -> Self {
        Self::new(OrderKind::Absent, OrderKind::Absent)
    }
}

/// One nonempty block of row ordinals in caller-declared order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowPartitionBlock {
    ordinals: Vec<u8>,
}

impl RowPartitionBlock {
    /// Returns the block's row ordinals in the caller-declared order.
    pub fn ordinals(&self) -> &[u8] {
        &self.ordinals
    }

    /// Returns the pitch classes reached by this block on `row`.
    pub fn pitch_classes(&self, row: &ToneRow) -> Vec<PitchClass> {
        self.ordinals
            .iter()
            .map(|ordinal| row.classes()[usize::from(*ordinal)])
            .collect()
    }

    /// Returns the unordered pitch-class mask reached by this block on `row`.
    pub fn mask(&self, row: &ToneRow) -> PitchClassMask {
        PitchClassMask::from_pitch_classes(&self.pitch_classes(row))
    }
}

/// A validated partition of all twelve row ordinals into disjoint nonempty blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowPartition {
    blocks: Vec<RowPartitionBlock>,
    order: BlockOrder,
}

impl RowPartition {
    /// Returns the validated blocks in caller order.
    pub fn blocks(&self) -> &[RowPartitionBlock] {
        &self.blocks
    }

    /// Returns the partition's ordering contract.
    pub const fn order(&self) -> BlockOrder {
        self.order
    }

    /// Returns the number of validated blocks.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Returns the size of each block in caller order.
    pub fn block_sizes(&self) -> Vec<usize> {
        self.blocks
            .iter()
            .map(|block| block.ordinals.len())
            .collect()
    }

    /// Returns all row ordinals in block-major caller order.
    pub fn ordinals(&self) -> Vec<u8> {
        self.blocks
            .iter()
            .flat_map(|block| block.ordinals.iter().copied())
            .collect()
    }
}

/// One exact block match shared by two validated partitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartitionBlockMatch {
    /// Matching block index in the left partition.
    pub left_block_index: usize,
    /// Matching block index in the right partition.
    pub right_block_index: usize,
    /// Shared row ordinals, preserved in the left block's order.
    pub ordinals: Vec<u8>,
}

/// Similarity evidence between two validated row partitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartitionSimilarityReport {
    /// Block sizes from the left partition.
    pub left_block_sizes: Vec<usize>,
    /// Block sizes from the right partition.
    pub right_block_sizes: Vec<usize>,
    /// Whether the two partitions use the same ordering contract.
    pub same_order_contract: bool,
    /// Whether the two partitions have the same block-size multiset.
    pub same_block_size_multiset: bool,
    /// Exact block matches regardless of block position.
    pub exact_block_matches: Vec<PartitionBlockMatch>,
    /// Cardinality of every left/right block overlap.
    pub overlap_matrix: Vec<Vec<usize>>,
}

/// Aggregate pitch-class coverage assembled from partition-derived blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateCoverageReport {
    /// The target aggregate expected by the caller.
    pub aggregate: PitchClassMask,
    /// The union of every supplied block mask.
    pub covered: PitchClassMask,
    /// Pitch classes still missing from `covered`.
    pub missing: PitchClassMask,
    /// Whether `covered` equals the requested aggregate exactly.
    pub complete: bool,
}

/// Interlocking evidence between two validated row partitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterlockingPartitionReport {
    /// Cardinality of every left/right block overlap.
    pub overlap_matrix: Vec<Vec<usize>>,
    /// For each left block, the right blocks with nonzero overlap.
    pub left_to_right_links: Vec<Vec<usize>>,
    /// For each right block, the left blocks with nonzero overlap.
    pub right_to_left_links: Vec<Vec<usize>>,
    /// Whether every block on both sides overlaps more than one opposite block.
    pub is_interlocking: bool,
}

/// Validates a caller-declared row partition over ordinals `0..12`.
pub fn try_partition(blocks: Vec<Vec<u8>>, order: BlockOrder) -> Result<RowPartition, RowError> {
    let mut seen = BTreeMap::new();
    let mut validated = Vec::with_capacity(blocks.len());
    for (block_index, ordinals) in blocks.into_iter().enumerate() {
        if ordinals.is_empty() {
            return Err(RowError::EmptyPartitionBlock { block_index });
        }
        for ordinal in &ordinals {
            if usize::from(*ordinal) >= 12 {
                return Err(RowError::InvalidOrdinal {
                    ordinal: usize::from(*ordinal),
                });
            }
            if let Some(first_block_index) = seen.insert(*ordinal, block_index) {
                return Err(RowError::DuplicatePartitionOrdinal {
                    ordinal: *ordinal,
                    first_block_index,
                    second_block_index: block_index,
                });
            }
        }
        validated.push(RowPartitionBlock { ordinals });
    }
    let missing = (0u8..12)
        .filter(|ordinal| !seen.contains_key(ordinal))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(RowError::PartitionCoverageMismatch { missing });
    }
    Ok(RowPartition {
        blocks: validated,
        order,
    })
}

/// Compares two validated partitions without collapsing their block order contracts.
pub fn analyze_partition_similarity(
    left: &RowPartition,
    right: &RowPartition,
) -> PartitionSimilarityReport {
    let left_block_sizes = left.block_sizes();
    let right_block_sizes = right.block_sizes();
    let overlap_matrix = left
        .blocks()
        .iter()
        .map(|left_block| {
            right
                .blocks()
                .iter()
                .map(|right_block| {
                    left_block
                        .ordinals()
                        .iter()
                        .filter(|ordinal| right_block.ordinals().contains(ordinal))
                        .count()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut left_sizes = left_block_sizes.clone();
    let mut right_sizes = right_block_sizes.clone();
    left_sizes.sort_unstable();
    right_sizes.sort_unstable();

    let exact_block_matches = left
        .blocks()
        .iter()
        .enumerate()
        .flat_map(|(left_block_index, left_block)| {
            right
                .blocks()
                .iter()
                .enumerate()
                .filter(move |(_, right_block)| left_block.ordinals() == right_block.ordinals())
                .map(
                    move |(right_block_index, _right_block)| PartitionBlockMatch {
                        left_block_index,
                        right_block_index,
                        ordinals: left_block.ordinals().to_vec(),
                    },
                )
        })
        .collect();

    PartitionSimilarityReport {
        left_block_sizes,
        right_block_sizes,
        same_order_contract: left.order() == right.order(),
        same_block_size_multiset: left_sizes == right_sizes,
        exact_block_matches,
        overlap_matrix,
    }
}

/// Computes aggregate pitch-class coverage for one validated partition on `row`.
pub fn analyze_partition_aggregate_coverage(
    row: &ToneRow,
    partition: &RowPartition,
) -> AggregateCoverageReport {
    let masks = partition
        .blocks()
        .iter()
        .map(|block| block.mask(row))
        .collect::<Vec<_>>();
    analyze_aggregate_coverage(PitchClassMask::from_pitch_classes(row.classes()), &masks)
}

/// Computes aggregate pitch-class coverage for any caller-supplied block masks.
pub fn analyze_aggregate_coverage(
    aggregate: PitchClassMask,
    masks: &[PitchClassMask],
) -> AggregateCoverageReport {
    let covered = masks
        .iter()
        .copied()
        .fold(PitchClassMask::default(), PitchClassMask::union);
    let missing = aggregate.difference(covered);
    AggregateCoverageReport {
        aggregate,
        covered,
        missing,
        complete: missing.bits() == 0,
    }
}

/// Reports how two validated partitions weave their ordinals across one another.
pub fn analyze_interlocking_partitions(
    left: &RowPartition,
    right: &RowPartition,
) -> InterlockingPartitionReport {
    let overlap_matrix = analyze_partition_similarity(left, right).overlap_matrix;
    let left_to_right_links = overlap_matrix
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .filter_map(|(index, overlap)| (*overlap > 0).then_some(index))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let right_to_left_links = (0..right.block_count())
        .map(|right_index| {
            overlap_matrix
                .iter()
                .enumerate()
                .filter_map(|(left_index, row)| (row[right_index] > 0).then_some(left_index))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let is_interlocking = left_to_right_links.iter().all(|links| links.len() > 1)
        && right_to_left_links.iter().all(|links| links.len() > 1);
    InterlockingPartitionReport {
        overlap_matrix,
        left_to_right_links,
        right_to_left_links,
        is_interlocking,
    }
}
