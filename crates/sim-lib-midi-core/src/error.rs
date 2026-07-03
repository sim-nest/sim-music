use thiserror::Error;

/// Errors produced when constructing or converting core MIDI values.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MidiError {
    /// A ticks-per-quarter resolution was zero.
    #[error("tpq must be non-zero")]
    ZeroTpq,
    /// A value exceeded the 7-bit ([`U7`](crate::U7)) range.
    #[error("value {0} is out of u7 range")]
    InvalidU7(u16),
    /// A value exceeded the 14-bit ([`U14`](crate::U14)) range.
    #[error("value {0} is out of u14 range")]
    InvalidU14(u16),
    /// A value exceeded the valid [`Channel`](crate::Channel) range.
    #[error("value {0} is out of channel range")]
    InvalidChannel(u8),
    /// A scaling ratio had a zero numerator or denominator.
    #[error("invalid ratio {0}/{1}")]
    InvalidRatio(i64, i64),
    /// A [`TickTime::rebase`](crate::TickTime::rebase) could not be performed
    /// exactly.
    #[error("inexact TPQ rebase")]
    InexactRebase,
}

/// An error raised while pumping events from a source into a sink, recording
/// which side failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PumpError<S, T> {
    /// The [`MidiSource`](crate::MidiSource) returned an error.
    Source(S),
    /// The [`MidiSink`](crate::MidiSink) returned an error.
    Sink(T),
}
