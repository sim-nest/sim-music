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
}
