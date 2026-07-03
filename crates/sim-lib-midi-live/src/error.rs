use thiserror::Error;

/// Errors raised when constructing live MIDI buffers.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LiveMidiError {
    /// A ring buffer was requested with zero capacity.
    #[error("ring buffer capacity must be at least 1")]
    ZeroCapacity,
}
