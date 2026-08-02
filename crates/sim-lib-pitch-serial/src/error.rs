//! Typed tone-row construction failures.

use sim_lib_serial_core::{AlphabetError, OrdinalMapError, SeriesError};
use thiserror::Error;

/// Failure while constructing the canonical alphabet or a strict tone row.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum RowError {
    /// The canonical pitch-class alphabet could not be constructed.
    #[error(transparent)]
    Alphabet(#[from] AlphabetError),
    /// The supplied row failed canonical membership or exactly-once validation.
    #[error(transparent)]
    Aggregate(#[from] SeriesError),
    /// A caller-supplied ordinal permutation was not a complete bijection.
    #[error(transparent)]
    OrdinalMap(#[from] OrdinalMapError),
    /// A contiguous row segment reached outside the twelve row positions.
    #[error("segment start {start} with length {len} is out of bounds for a twelve-tone row")]
    SegmentOutOfBounds {
        /// Zero-based start ordinal requested by the caller.
        start: usize,
        /// Number of row positions requested from `start`.
        len: usize,
    },
    /// A wrapped row segment exceeded the row length.
    #[error("wrapped segment length {len} exceeds the row length")]
    WrappedSegmentTooLong {
        /// Number of row positions requested for the wrapped segment.
        len: usize,
    },
    /// An indexed segment referenced an invalid row ordinal.
    #[error("row ordinal {ordinal} is out of bounds for a twelve-tone row")]
    InvalidOrdinal {
        /// Zero-based row ordinal that fell outside `0..12`.
        ordinal: usize,
    },
    /// A requested partition block contained no row ordinals.
    #[error("partition block {block_index} is empty")]
    EmptyPartitionBlock {
        /// Zero-based block index in caller order.
        block_index: usize,
    },
    /// A requested partition assigned one row ordinal to more than one block.
    #[error(
        "row ordinal {ordinal} appears in multiple partition blocks ({first_block_index} and {second_block_index})"
    )]
    DuplicatePartitionOrdinal {
        /// Zero-based row ordinal that appeared more than once.
        ordinal: u8,
        /// First block index that claimed the ordinal.
        first_block_index: usize,
        /// Later block index that duplicated the ordinal.
        second_block_index: usize,
    },
    /// A requested partition failed to cover the full row exactly once.
    #[error("partition coverage is incomplete; missing row ordinals {missing:?}")]
    PartitionCoverageMismatch {
        /// Zero-based row ordinals that were not assigned to any block.
        missing: Vec<u8>,
    },
    /// A derivation or combinatoriality partition size was unsupported.
    #[error("partition size {size} is invalid; expected one of 2, 3, 4, or 6")]
    InvalidPartitionSize {
        /// The invalid partition size requested by the caller.
        size: usize,
    },
}
