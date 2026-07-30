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
    /// A channel-message decode ran out of data bytes.
    #[error("channel message data is truncated")]
    TruncatedChannel,
    /// A status byte's high nibble is not a channel-voice message.
    #[error("status byte {0:#04x} is not a channel message")]
    NotChannelStatus(u8),
    /// A [`TickTime::rebase`](crate::TickTime::rebase) could not be performed
    /// exactly.
    #[error("inexact TPQ rebase")]
    InexactRebase,
    /// A tempo meta event carried the forbidden zero microseconds-per-quarter
    /// value.
    #[error("MIDI tempo must be non-zero")]
    ZeroTempo,
    /// An event or conversion used a negative tick, which is outside an SMF
    /// tempo timeline.
    #[error("MIDI tempo-map ticks must be non-negative")]
    NegativeTempoTick,
    /// Tempo-map source events were not ordered by exact tick time.
    #[error("MIDI tempo events must be ordered by tick")]
    TempoEventsOutOfOrder,
    /// A tempo map was requested for a non-metrical time division.
    #[error("MIDI tempo maps require a metrical ticks-per-quarter division")]
    MetricalTempoRequired,
    /// An exact beat or wall-time position does not land on an integer MIDI
    /// tick at the map's resolution.
    #[error("time does not land on an exact MIDI tick")]
    InexactTempoTick,
    /// A tempo conversion exceeded its integer representation.
    #[error("MIDI tempo conversion overflowed")]
    TempoOverflow,
    /// The shared stream-clock chart rejected tempo arithmetic.
    #[error("MIDI tempo chart failed: {0}")]
    TempoChart(String),
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
