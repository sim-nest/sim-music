//! Typed tone-row construction failures.

use sim_lib_serial_core::{AlphabetError, SeriesError};
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
}
