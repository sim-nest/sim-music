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
    /// The header requested SMPTE division, which is not supported.
    #[error("unsupported SMPTE division 0x{raw:04x} at byte {offset}")]
    UnsupportedSmpteDivision {
        /// Byte offset of the division field.
        offset: usize,
        /// The raw division value.
        raw: u16,
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
    /// The header format and the track count are inconsistent (for example,
    /// format 0 with more than one track).
    #[error("SMF format/track count mismatch")]
    FormatTrackMismatch,
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
}
