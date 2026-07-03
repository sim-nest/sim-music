use std::convert::TryFrom;
use std::ops::{Add, Sub};

use sim_kernel::{CodecId, Origin, SourceId, Span};

use crate::MidiError;

/// A point in musical time measured in ticks at a given resolution.
///
/// `ticks` counts the offset and `tpq` is the ticks-per-quarter-note
/// resolution, so the same instant can be represented at different
/// resolutions. Helpers convert between resolutions via [`rebase`](Self::rebase)
/// (exact) and [`quantize`](Self::quantize) (rounding).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TickTime {
    /// Tick offset, interpreted at this value's [`tpq`](Self::tpq) resolution.
    pub ticks: i64,
    /// Resolution in ticks per quarter note; must be non-zero.
    pub tpq: u32,
}

impl TickTime {
    /// The origin: zero ticks at unit resolution.
    pub const ZERO: Self = Self { ticks: 0, tpq: 1 };

    /// Creates a time at `tpq` resolution, failing with [`MidiError::ZeroTpq`]
    /// when `tpq` is zero.
    pub fn new(ticks: i64, tpq: u32) -> Result<Self, MidiError> {
        if tpq == 0 {
            Err(MidiError::ZeroTpq)
        } else {
            Ok(Self { ticks, tpq })
        }
    }

    /// Builds a time of whole quarter notes at unit resolution (`tpq == 1`).
    pub fn from_quarters(quarters: i64) -> Self {
        Self {
            ticks: quarters,
            tpq: 1,
        }
    }

    /// Scales the tick count by an integer factor, keeping the resolution.
    pub fn mul_int(self, factor: i64) -> Self {
        Self {
            ticks: self.ticks * factor,
            tpq: self.tpq,
        }
    }

    /// Scales the tick count by `numerator / denominator`, failing with
    /// [`MidiError::InvalidRatio`] when `denominator` is zero.
    pub fn mul_ratio(self, numerator: i64, denominator: i64) -> Result<Self, MidiError> {
        if denominator == 0 {
            return Err(MidiError::InvalidRatio(numerator, denominator));
        }
        Ok(Self {
            ticks: self.ticks * numerator / denominator,
            tpq: self.tpq,
        })
    }

    /// Scales the tick count by `denominator / numerator`, failing with
    /// [`MidiError::InvalidRatio`] when `numerator` is zero.
    pub fn div_ratio(self, numerator: i64, denominator: i64) -> Result<Self, MidiError> {
        if numerator == 0 {
            return Err(MidiError::InvalidRatio(numerator, denominator));
        }
        Ok(Self {
            ticks: self.ticks * denominator / numerator,
            tpq: self.tpq,
        })
    }

    /// Re-expresses this time at `new_tpq` resolution without rounding.
    ///
    /// Fails with [`MidiError::ZeroTpq`] when `new_tpq` is zero, or
    /// [`MidiError::InexactRebase`] when the conversion would not be exact.
    pub fn rebase(self, new_tpq: u32) -> Result<Self, MidiError> {
        if new_tpq == 0 {
            return Err(MidiError::ZeroTpq);
        }
        let scaled = self.ticks * i64::from(new_tpq);
        if scaled % i64::from(self.tpq) != 0 {
            return Err(MidiError::InexactRebase);
        }
        Ok(Self {
            ticks: scaled / i64::from(self.tpq),
            tpq: new_tpq,
        })
    }

    /// Re-expresses this time at `new_tpq` resolution, rounding to the nearest
    /// tick. A zero `new_tpq` is clamped to unit resolution.
    pub fn quantize(self, new_tpq: u32) -> Self {
        let scaled = self.ticks as f64 * f64::from(new_tpq) / f64::from(self.tpq);
        Self {
            ticks: scaled.round() as i64,
            tpq: new_tpq.max(1),
        }
    }

    /// Returns the time as a `(ticks, tpq)` rational in quarter notes.
    pub fn as_rational(self) -> (i64, i64) {
        (self.ticks, i64::from(self.tpq))
    }

    /// Returns the time as a floating-point count of quarter notes.
    pub fn as_f64_quarters(self) -> f64 {
        self.ticks as f64 / self.tpq as f64
    }
}

impl Add for TickTime {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        let other = other.quantize(self.tpq);
        Self {
            ticks: self.ticks + other.ticks,
            tpq: self.tpq,
        }
    }
}

impl Sub for TickTime {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        let other = other.quantize(self.tpq);
        Self {
            ticks: self.ticks - other.ticks,
            tpq: self.tpq,
        }
    }
}

/// A 7-bit MIDI data value in the range `0..=127`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct U7(pub u8);

impl TryFrom<u16> for U7 {
    type Error = MidiError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if value <= 127 {
            Ok(Self(value as u8))
        } else {
            Err(MidiError::InvalidU7(value))
        }
    }
}

/// A 14-bit MIDI data value in the range `0..=16_383`, as used by pitch bend.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct U14(pub u16);

impl TryFrom<u16> for U14 {
    type Error = MidiError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if value <= 16_383 {
            Ok(Self(value))
        } else {
            Err(MidiError::InvalidU14(value))
        }
    }
}

/// A MIDI channel number in the range `0..=15`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Channel(pub u8);

impl Channel {
    /// Creates a channel, failing with [`MidiError::InvalidChannel`] when
    /// `value` exceeds 15.
    pub fn new(value: u8) -> Result<Self, MidiError> {
        if value <= 15 {
            Ok(Self(value))
        } else {
            Err(MidiError::InvalidChannel(value))
        }
    }
}

/// A MIDI channel-voice message, carrying its target [`Channel`] and payload.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChannelMessage {
    /// Note-off: release `key` on `ch` with release velocity `vel`.
    NoteOff {
        /// Target channel.
        ch: Channel,
        /// Note number.
        key: U7,
        /// Release velocity.
        vel: U7,
    },
    /// Note-on: start `key` on `ch` with velocity `vel` (velocity 0 is a
    /// conventional note-off).
    NoteOn {
        /// Target channel.
        ch: Channel,
        /// Note number.
        key: U7,
        /// Attack velocity.
        vel: U7,
    },
    /// Polyphonic key pressure (aftertouch) for a single note.
    PolyAftertouch {
        /// Target channel.
        ch: Channel,
        /// Note number.
        key: U7,
        /// Key pressure.
        pressure: U7,
    },
    /// Control-change message setting controller `cc` to `value`.
    ControlChange {
        /// Target channel.
        ch: Channel,
        /// Controller number (see the `CC_*` constants).
        cc: U7,
        /// Controller value.
        value: U7,
    },
    /// Program (patch) change.
    ProgramChange {
        /// Target channel.
        ch: Channel,
        /// Program number.
        program: U7,
    },
    /// Channel pressure (aftertouch) applied to the whole channel.
    ChanAftertouch {
        /// Target channel.
        ch: Channel,
        /// Channel pressure.
        pressure: U7,
    },
    /// Pitch-bend wheel position as a 14-bit value (`8192` is centred).
    PitchBend {
        /// Target channel.
        ch: Channel,
        /// Bend amount.
        value: U14,
    },
}

/// A meta event: track-scoped information that is not transmitted over the
/// wire, recognised types plus an [`Other`](Self::Other) escape hatch.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MetaEvent {
    /// End-of-track marker.
    EndOfTrack,
    /// Tempo change expressed in microseconds per quarter note.
    Tempo {
        /// Microseconds per quarter note.
        us_per_quarter: u32,
    },
    /// Time signature.
    TimeSig {
        /// Numerator (beats per bar).
        num: u8,
        /// Denominator as a power of two (e.g. `2` for a quarter-note beat).
        den_pow2: u8,
        /// MIDI clocks per metronome click.
        clocks_per_click: u8,
        /// Notated 32nd notes per quarter note.
        thirty_seconds_per_quarter: u8,
    },
    /// Key signature.
    KeySig {
        /// Number of sharps (positive) or flats (negative).
        sharps_flats: i8,
        /// Whether the key is minor.
        minor: bool,
    },
    /// Any other meta type, carried as a raw [`MetaBucket`].
    Other(MetaBucket),
}

/// A raw meta event: its type byte plus payload bytes.
///
/// The [`meta_view`](crate::meta_view) helpers interpret common buckets such as
/// text, track name, marker, and SMPTE offset.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MetaBucket {
    /// MIDI meta type byte.
    pub type_byte: u8,
    /// Raw payload bytes.
    pub data: Vec<u8>,
}

/// An SMPTE timecode offset, as carried by the SMPTE-offset meta event.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SmpteOffset {
    /// Hours field.
    pub hours: u8,
    /// Minutes field.
    pub minutes: u8,
    /// Seconds field.
    pub seconds: u8,
    /// Frames field.
    pub frames: u8,
    /// Fractional-frame (subframe) field.
    pub subframes: u8,
}

/// A system-exclusive event, distinguished by its leading status byte.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SysExEvent {
    /// A complete or first-packet message introduced by `0xF0`.
    F0 {
        /// Message body bytes (excluding the leading status byte).
        data: Vec<u8>,
    },
    /// A continuation or escape packet introduced by `0xF7`.
    F7 {
        /// Message body bytes (excluding the leading status byte).
        data: Vec<u8>,
    },
}

/// An uninterpreted run of bytes with a status byte, used as a fallback.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RawBytes {
    /// Status byte.
    pub status: u8,
    /// Data bytes following the status byte.
    pub data: Vec<u8>,
}

/// The payload of a [`MidiEvent`]: one of the four event families.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MidiPayload {
    /// A channel-voice message.
    Channel(ChannelMessage),
    /// A meta event.
    Meta(MetaEvent),
    /// A system-exclusive event.
    SysEx(SysExEvent),
    /// Uninterpreted raw bytes.
    Raw(RawBytes),
}

/// A timestamped MIDI event with provenance.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MidiEvent {
    /// Event time.
    pub time: TickTime,
    /// Source provenance carried from the originating codec.
    pub origin: Origin,
    /// Event payload.
    pub payload: MidiPayload,
}

/// Returns an [`Origin`] tagging events synthesised by the music stack rather
/// than read from an input codec.
pub fn synthetic_origin() -> Origin {
    Origin {
        codec: CodecId(0),
        source: SourceId("music-stack".to_owned()),
        span: Span { start: 0, end: 0 },
        trivia: Vec::new(),
    }
}
