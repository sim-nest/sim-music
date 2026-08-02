#![forbid(unsafe_code)]

use thiserror::Error;

/// Errors raised while reading or writing a Standard MIDI File.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SmfError {
    /// The `MThd`/`MTrk` chunk header was malformed.
    #[error("invalid header at byte {offset}")]
    InvalidHeader {
        /// Byte offset of the bad header.
        offset: usize,
    },
    /// The byte stream ended before the structure was complete.
    #[error("unexpected end of file at byte {offset}")]
    UnexpectedEof {
        /// Byte offset where more input was expected.
        offset: usize,
    },
    /// A variable-length quantity was not terminated within four bytes.
    #[error("invalid VLQ at byte {offset}")]
    InvalidVlq {
        /// Byte offset where the VLQ began.
        offset: usize,
    },
    /// The header carried an invalid metrical or SMPTE division.
    #[error("invalid SMF division 0x{raw:04x} at byte {offset}")]
    InvalidDivision {
        /// Byte offset of the division field.
        offset: usize,
        /// The raw division value.
        raw: u16,
    },
    /// A configured defensive read limit was exceeded.
    #[error("SMF {kind} limit exceeded at byte {offset}: requested {actual}, maximum {maximum}")]
    LimitExceeded {
        /// Byte offset of the value or event that crossed the limit.
        offset: usize,
        /// Resource being bounded.
        kind: SmfLimitKind,
        /// Requested or observed amount.
        actual: usize,
        /// Configured maximum.
        maximum: usize,
    },
    /// A bounded allocation failed even though its requested size was within
    /// the configured limits.
    #[error("SMF allocation of {requested} items failed at byte {offset}")]
    AllocationFailed {
        /// Byte offset of the structure that required the allocation.
        offset: usize,
        /// Number of items requested.
        requested: usize,
    },
    /// A data byte appeared with no running status in effect.
    #[error("malformed running status at byte {offset}")]
    MalformedRunningStatus {
        /// Byte offset of the offending data byte.
        offset: usize,
    },
    /// A status byte that the reader does not handle was encountered.
    #[error("unsupported MIDI status 0x{status:02x} at byte {offset}")]
    UnsupportedStatus {
        /// Byte offset of the status byte.
        offset: usize,
        /// The unsupported status byte.
        status: u8,
    },
    /// A channel message carried an out-of-range data byte.
    #[error("invalid channel payload at byte {offset}")]
    InvalidChannelData {
        /// Byte offset of the bad data.
        offset: usize,
    },
    /// A recognised meta event used a payload length forbidden by SMF.
    #[error(
        "invalid length {actual} for meta event 0x{type_byte:02x} at byte {offset}; expected {expected}"
    )]
    InvalidMetaLength {
        /// Byte offset of the meta type byte.
        offset: usize,
        /// Meta event type.
        type_byte: u8,
        /// Required payload length.
        expected: usize,
        /// Encoded payload length.
        actual: usize,
    },
    /// A track chunk ended without its required end-of-track meta event.
    #[error("missing end-of-track event at byte {offset}")]
    MissingEndOfTrack {
        /// Byte offset immediately after the track chunk body.
        offset: usize,
    },
    /// A system message used an invalid status, length, or data byte.
    #[error("invalid system event 0x{status:02x} at byte {offset}")]
    InvalidSystemEvent {
        /// Byte offset of the status or first invalid data byte.
        offset: usize,
        /// System status byte.
        status: u8,
    },
    /// The header format and the track count are inconsistent (for example,
    /// format 0 with more than one track).
    #[error("SMF format/track count mismatch")]
    FormatTrackMismatch,
    /// Format 2 contains independent patterns and cannot be flattened onto a
    /// shared timeline without selecting a track.
    #[error("SMF format 2 patterns require explicit track selection")]
    IndependentPatternsCannotMerge,
    /// The track count cannot be represented in the SMF header.
    #[error("SMF track count {0} is outside 0..=65535")]
    TrackCountOutOfRange(usize),
    /// The ticks-per-quarter value cannot be represented as metrical SMF TPQ.
    #[error("SMF ticks-per-quarter {0} cannot be written as metrical TPQ")]
    TpqOutOfRange(u32),
    /// An event time could not be represented exactly at the file resolution.
    #[error("event time cannot be represented exactly at target TPQ")]
    InexactEventTime,
    /// Track events were not monotonic in absolute time, yielding a negative
    /// delta.
    #[error("track events are not monotonic in absolute time")]
    NegativeDelta,
    /// A track delta cannot be represented as an SMF four-byte VLQ.
    #[error("SMF delta {0} exceeds the four-byte VLQ limit")]
    DeltaOutOfRange(i64),
    /// A track chunk body cannot be represented in the SMF chunk length field.
    #[error("SMF chunk length {0} exceeds u32::MAX")]
    ChunkTooLarge(usize),
    /// A meta or SysEx payload length cannot be represented as an SMF four-byte
    /// VLQ.
    #[error("SMF payload length {0} exceeds the four-byte VLQ limit")]
    PayloadTooLarge(usize),
    /// Absolute tick accumulation overflowed the in-memory event time.
    #[error("SMF absolute tick time overflow at byte {offset}")]
    TimeOverflow {
        /// Byte offset of the delta that overflowed.
        offset: usize,
    },
}

/// Defensive parser resources that can be bounded with [`SmfReadLimits`].
///
/// [`SmfReadLimits`]: crate::SmfReadLimits
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SmfLimitKind {
    /// Complete input bytes.
    FileBytes,
    /// Header chunk body bytes.
    HeaderBytes,
    /// Declared track count.
    TrackCount,
    /// One track chunk body.
    TrackBytes,
    /// Events across the complete file.
    EventCount,
    /// One meta or system-exclusive payload.
    EventPayloadBytes,
    /// Meta and system-exclusive payload bytes across the complete file.
    TotalPayloadBytes,
}

impl std::fmt::Display for SmfLimitKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::FileBytes => "file-bytes",
            Self::HeaderBytes => "header-bytes",
            Self::TrackCount => "track-count",
            Self::TrackBytes => "track-bytes",
            Self::EventCount => "event-count",
            Self::EventPayloadBytes => "event-payload-bytes",
            Self::TotalPayloadBytes => "total-payload-bytes",
        })
    }
}
