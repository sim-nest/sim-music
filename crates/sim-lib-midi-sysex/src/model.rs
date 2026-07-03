use std::fmt;

use sim_lib_midi_core::SysExEvent;

/// Status id introducing a Universal Non-Real-Time SysEx message (`0x7e`).
pub const UNIVERSAL_NON_REAL_TIME_ID: u8 = 0x7e;
/// Status id introducing a Universal Real-Time SysEx message (`0x7f`).
pub const UNIVERSAL_REAL_TIME_ID: u8 = 0x7f;
/// Sub-id 1 identifying a MIDI Tuning Standard message (`0x08`).
pub const MIDI_TUNING_STANDARD_SUB_ID: u8 = 0x08;

/// Which Universal SysEx realm a message belongs to.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum UniversalRealm {
    /// Non-real-time (status id `0x7e`).
    NonRealTime,
    /// Real-time (status id `0x7f`).
    RealTime,
}

impl UniversalRealm {
    /// Resolves a realm from its status id, failing with
    /// [`SysExViewError::NotUniversal`] for any other byte.
    pub fn from_status_id(value: u8) -> Result<Self, SysExViewError> {
        match value {
            UNIVERSAL_NON_REAL_TIME_ID => Ok(Self::NonRealTime),
            UNIVERSAL_REAL_TIME_ID => Ok(Self::RealTime),
            _ => Err(SysExViewError::NotUniversal { id: value }),
        }
    }

    /// Returns the status id byte for this realm.
    pub fn status_id(self) -> u8 {
        match self {
            Self::NonRealTime => UNIVERSAL_NON_REAL_TIME_ID,
            Self::RealTime => UNIVERSAL_REAL_TIME_ID,
        }
    }
}

/// A parsed Universal SysEx message (realm, device id, two sub-ids, payload).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UniversalSysEx {
    /// Real-time or non-real-time realm.
    pub realm: UniversalRealm,
    /// Target device id (`0x7f` is the broadcast id).
    pub device_id: u8,
    /// First sub-id (message category).
    pub sub_id1: u8,
    /// Second sub-id (message within the category).
    pub sub_id2: u8,
    /// Remaining payload data bytes.
    pub data: Vec<u8>,
}

impl UniversalSysEx {
    /// Builds a message, validating that every field is a 7-bit data byte.
    pub fn new(
        realm: UniversalRealm,
        device_id: u8,
        sub_id1: u8,
        sub_id2: u8,
        data: Vec<u8>,
    ) -> Result<Self, SysExViewError> {
        validate_data_byte(1, device_id)?;
        validate_data_byte(2, sub_id1)?;
        validate_data_byte(3, sub_id2)?;
        validate_data_bytes(4, &data)?;
        Ok(Self {
            realm,
            device_id,
            sub_id1,
            sub_id2,
            data,
        })
    }

    /// Parses a Universal message from an `F0` SysEx event, failing with
    /// [`SysExViewError::NotF0`] for an `F7` event.
    pub fn from_event(event: &SysExEvent) -> Result<Self, SysExViewError> {
        let SysExEvent::F0 { data } = event else {
            return Err(SysExViewError::NotF0);
        };
        Self::from_f0_payload(data)
    }

    /// Parses a Universal message from an `F0` payload (excluding the `0xF0`
    /// and terminating `0xF7` bytes).
    pub fn from_f0_payload(payload: &[u8]) -> Result<Self, SysExViewError> {
        if payload.len() < 4 {
            return Err(SysExViewError::TooShort { len: payload.len() });
        }
        validate_data_bytes(0, payload)?;
        Self::new(
            UniversalRealm::from_status_id(payload[0])?,
            payload[1],
            payload[2],
            payload[3],
            payload[4..].to_vec(),
        )
    }

    /// Serialises this message back into an `F0` [`SysExEvent`].
    pub fn to_event(&self) -> Result<SysExEvent, SysExViewError> {
        Ok(SysExEvent::F0 {
            data: self.to_f0_payload()?,
        })
    }

    /// Serialises this message into an `F0` payload byte vector.
    pub fn to_f0_payload(&self) -> Result<Vec<u8>, SysExViewError> {
        validate_data_byte(1, self.device_id)?;
        validate_data_byte(2, self.sub_id1)?;
        validate_data_byte(3, self.sub_id2)?;
        validate_data_bytes(4, &self.data)?;
        let mut payload = Vec::with_capacity(self.data.len() + 4);
        payload.push(self.realm.status_id());
        payload.push(self.device_id);
        payload.push(self.sub_id1);
        payload.push(self.sub_id2);
        payload.extend_from_slice(&self.data);
        Ok(payload)
    }

    /// Returns whether sub-id 1 marks this as a MIDI Tuning Standard message.
    pub fn is_midi_tuning_standard(&self) -> bool {
        self.sub_id1 == MIDI_TUNING_STANDARD_SUB_ID
    }
}

/// The kind of MIDI Tuning Standard message, keyed by sub-id 2.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MtsMessageKind {
    /// Bulk tuning dump request (`0x00`).
    BulkTuningDumpRequest,
    /// Bulk tuning dump reply (`0x01`).
    BulkTuningDumpReply,
    /// Single-note tuning change (`0x02`).
    SingleNoteTuningChange,
    /// Any other sub-id 2 value.
    Unknown(u8),
}

impl MtsMessageKind {
    /// Resolves the message kind from its sub-id 2 byte.
    pub fn from_sub_id2(value: u8) -> Self {
        match value {
            0x00 => Self::BulkTuningDumpRequest,
            0x01 => Self::BulkTuningDumpReply,
            0x02 => Self::SingleNoteTuningChange,
            _ => Self::Unknown(value),
        }
    }

    /// Returns the sub-id 2 byte for this kind.
    pub fn sub_id2(self) -> u8 {
        match self {
            Self::BulkTuningDumpRequest => 0x00,
            Self::BulkTuningDumpReply => 0x01,
            Self::SingleNoteTuningChange => 0x02,
            Self::Unknown(value) => value,
        }
    }
}

/// A MIDI Tuning Standard message: a [`UniversalSysEx`] whose sub-id 1 is the
/// tuning-standard id.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MtsMessage {
    universal: UniversalSysEx,
}

impl MtsMessage {
    /// Builds a tuning-standard message of `kind` carrying `data`.
    pub fn new(
        realm: UniversalRealm,
        device_id: u8,
        kind: MtsMessageKind,
        data: Vec<u8>,
    ) -> Result<Self, SysExViewError> {
        Ok(Self {
            universal: UniversalSysEx::new(
                realm,
                device_id,
                MIDI_TUNING_STANDARD_SUB_ID,
                kind.sub_id2(),
                data,
            )?,
        })
    }

    /// Parses a tuning-standard message from a SysEx event.
    pub fn from_event(event: &SysExEvent) -> Result<Self, SysExViewError> {
        Self::from_universal(UniversalSysEx::from_event(event)?)
    }

    /// Wraps a [`UniversalSysEx`], failing with
    /// [`SysExViewError::NotMidiTuningStandard`] if it is not a tuning message.
    pub fn from_universal(universal: UniversalSysEx) -> Result<Self, SysExViewError> {
        if !universal.is_midi_tuning_standard() {
            return Err(SysExViewError::NotMidiTuningStandard {
                sub_id1: universal.sub_id1,
            });
        }
        Ok(Self { universal })
    }

    /// Serialises this message into an `F0` [`SysExEvent`].
    pub fn to_event(&self) -> Result<SysExEvent, SysExViewError> {
        self.universal.to_event()
    }

    /// Returns the underlying Universal SysEx view.
    pub fn universal(&self) -> &UniversalSysEx {
        &self.universal
    }

    /// Returns the message realm.
    pub fn realm(&self) -> UniversalRealm {
        self.universal.realm
    }

    /// Returns the target device id.
    pub fn device_id(&self) -> u8 {
        self.universal.device_id
    }

    /// Returns the tuning-message kind.
    pub fn kind(&self) -> MtsMessageKind {
        MtsMessageKind::from_sub_id2(self.universal.sub_id2)
    }

    /// Returns the message payload bytes.
    pub fn payload(&self) -> &[u8] {
        &self.universal.data
    }
}

/// Errors raised while interpreting or building typed SysEx views.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SysExViewError {
    /// A typed view was requested from an `F7` (non-`F0`) event.
    NotF0,
    /// The payload was shorter than the message requires.
    TooShort {
        /// Actual payload length.
        len: usize,
    },
    /// The leading status id was not a Universal SysEx id.
    NotUniversal {
        /// The observed status id.
        id: u8,
    },
    /// The sub-id 1 was not the MIDI Tuning Standard id.
    NotMidiTuningStandard {
        /// The observed sub-id 1.
        sub_id1: u8,
    },
    /// The manufacturer id was not Yamaha's.
    NotYamaha {
        /// The observed manufacturer id.
        id: u8,
    },
    /// The Yamaha format byte is not a recognised DX7 format.
    UnsupportedYamahaFormat {
        /// The observed format byte.
        format: u8,
    },
    /// A declared byte count did not match the actual payload length.
    InvalidByteCount {
        /// Expected byte count.
        expected: usize,
        /// Actual byte count.
        actual: usize,
    },
    /// A Yamaha checksum did not match the recomputed value.
    InvalidChecksum {
        /// Expected checksum.
        expected: u8,
        /// Actual checksum.
        actual: u8,
    },
    /// A DX7 structure had the wrong length.
    InvalidDx7Length {
        /// Which structure was being parsed.
        context: &'static str,
        /// Expected length.
        expected: usize,
        /// Actual length.
        actual: usize,
    },
    /// A byte that must be 7-bit exceeded `0x7f`.
    InvalidDataByte {
        /// Index of the offending byte.
        index: usize,
        /// The offending value.
        value: u8,
    },
}

impl fmt::Display for SysExViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotF0 => f.write_str("typed SysEx views require an F0 SysEx event"),
            Self::TooShort { len } => {
                write!(f, "SysEx payload is too short: {len} byte(s)")
            }
            Self::NotUniversal { id } => {
                write!(f, "payload is not universal SysEx: status id 0x{id:02x}")
            }
            Self::NotMidiTuningStandard { sub_id1 } => {
                write!(
                    f,
                    "payload is not MIDI Tuning Standard: sub-id1 0x{sub_id1:02x}"
                )
            }
            Self::NotYamaha { id } => {
                write!(f, "payload is not Yamaha SysEx: manufacturer 0x{id:02x}")
            }
            Self::UnsupportedYamahaFormat { format } => {
                write!(f, "unsupported Yamaha DX7 format 0x{format:02x}")
            }
            Self::InvalidByteCount { expected, actual } => {
                write!(
                    f,
                    "Yamaha SysEx byte count declares {expected} byte(s), got {actual}"
                )
            }
            Self::InvalidChecksum { expected, actual } => {
                write!(
                    f,
                    "Yamaha SysEx checksum 0x{actual:02x} does not match 0x{expected:02x}"
                )
            }
            Self::InvalidDx7Length {
                context,
                expected,
                actual,
            } => {
                write!(f, "DX7 {context} expects {expected} byte(s), got {actual}")
            }
            Self::InvalidDataByte { index, value } => {
                write!(
                    f,
                    "SysEx data byte at index {index} is not 7-bit: 0x{value:02x}"
                )
            }
        }
    }
}

impl std::error::Error for SysExViewError {}

pub(crate) fn validate_data_bytes(offset: usize, bytes: &[u8]) -> Result<(), SysExViewError> {
    for (index, byte) in bytes.iter().enumerate() {
        validate_data_byte(offset + index, *byte)?;
    }
    Ok(())
}

pub(crate) fn validate_data_byte(index: usize, value: u8) -> Result<(), SysExViewError> {
    if value <= 0x7f {
        Ok(())
    } else {
        Err(SysExViewError::InvalidDataByte { index, value })
    }
}
