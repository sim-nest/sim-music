//! Yamaha manufacturer SysEx and DX7 voice/bank patch formats.

use sim_lib_midi_core::SysExEvent;

use crate::{SysExViewError, validate_data_byte, validate_data_bytes};

/// Yamaha's MIDI manufacturer id (`0x43`).
pub const YAMAHA_MANUFACTURER_ID: u8 = 0x43;
/// DX7 format byte for a single (edit-buffer) voice (`0x00`).
pub const DX7_SINGLE_VOICE_FORMAT: u8 = 0x00;
/// DX7 format byte for a 32-voice bank (`0x09`).
pub const DX7_VOICE_BANK_FORMAT: u8 = 0x09;
/// Number of operators in a DX7 voice.
pub const DX7_OPERATOR_COUNT: usize = 6;
/// Bytes per operator in the unpacked (edit-buffer) layout.
pub const DX7_OPERATOR_EDIT_BYTE_COUNT: usize = 21;
/// Bytes per operator in the packed layout.
pub const DX7_OPERATOR_PACKED_BYTE_COUNT: usize = 17;
/// Total bytes of an unpacked single voice.
pub const DX7_SINGLE_VOICE_BYTE_COUNT: usize = 155;
/// Total bytes of a packed voice.
pub const DX7_PACKED_VOICE_BYTE_COUNT: usize = 128;
/// Number of voices in a DX7 bank.
pub const DX7_BANK_VOICE_COUNT: usize = 32;
/// Total bytes of a packed DX7 bank.
pub const DX7_BANK_BYTE_COUNT: usize = DX7_PACKED_VOICE_BYTE_COUNT * DX7_BANK_VOICE_COUNT;

/// A parsed Yamaha SysEx message body (sub-status, format, and data, framed by
/// the manufacturer id, byte count, and checksum).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct YamahaSysEx {
    /// Sub-status / channel byte.
    pub sub_status: u8,
    /// Format byte selecting the payload layout.
    pub format: u8,
    /// Payload data bytes (excluding the trailing checksum).
    pub data: Vec<u8>,
}

impl YamahaSysEx {
    /// Builds a Yamaha message, validating the framing bytes as 7-bit.
    pub fn new(sub_status: u8, format: u8, data: Vec<u8>) -> Result<Self, SysExViewError> {
        validate_data_byte(1, sub_status)?;
        validate_data_byte(2, format)?;
        validate_data_bytes(5, &data)?;
        Ok(Self {
            sub_status,
            format,
            data,
        })
    }

    /// Parses a Yamaha message from an `F0` SysEx event.
    pub fn from_event(event: &SysExEvent) -> Result<Self, SysExViewError> {
        let SysExEvent::F0 { data } = event else {
            return Err(SysExViewError::NotF0);
        };
        Self::from_f0_payload(data)
    }

    /// Parses a Yamaha message from an `F0` payload, checking the manufacturer
    /// id, declared byte count, and trailing checksum.
    pub fn from_f0_payload(payload: &[u8]) -> Result<Self, SysExViewError> {
        if payload.len() < 6 {
            return Err(SysExViewError::TooShort { len: payload.len() });
        }
        validate_data_bytes(0, payload)?;
        if payload[0] != YAMAHA_MANUFACTURER_ID {
            return Err(SysExViewError::NotYamaha { id: payload[0] });
        }
        let declared_len = yamaha_byte_count(payload[3], payload[4]);
        let actual_len = payload.len() - 6;
        if declared_len != actual_len {
            return Err(SysExViewError::InvalidByteCount {
                expected: declared_len,
                actual: actual_len,
            });
        }
        let data = payload[5..payload.len() - 1].to_vec();
        let actual_checksum = payload[payload.len() - 1];
        let expected_checksum = yamaha_checksum(&data);
        if actual_checksum != expected_checksum {
            return Err(SysExViewError::InvalidChecksum {
                expected: expected_checksum,
                actual: actual_checksum,
            });
        }
        Self::new(payload[1], payload[2], data)
    }

    /// Serialises this message into an `F0` [`SysExEvent`].
    pub fn to_event(&self) -> Result<SysExEvent, SysExViewError> {
        Ok(SysExEvent::F0 {
            data: self.to_f0_payload()?,
        })
    }

    /// Serialises this message into an `F0` payload, adding the byte count and
    /// checksum framing.
    pub fn to_f0_payload(&self) -> Result<Vec<u8>, SysExViewError> {
        validate_data_byte(1, self.sub_status)?;
        validate_data_byte(2, self.format)?;
        validate_data_bytes(5, &self.data)?;
        let (count_msb, count_lsb) = yamaha_count_bytes(self.data.len())?;
        let mut payload = Vec::with_capacity(self.data.len() + 6);
        payload.push(YAMAHA_MANUFACTURER_ID);
        payload.push(self.sub_status);
        payload.push(self.format);
        payload.push(count_msb);
        payload.push(count_lsb);
        payload.extend_from_slice(&self.data);
        payload.push(yamaha_checksum(&self.data));
        Ok(payload)
    }
}

/// A DX7 bulk-dump message: either a single voice or a full bank.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Dx7Bulk {
    /// A single edit-buffer voice.
    SingleVoice(Dx7Voice),
    /// A 32-voice bank.
    VoiceBank(Dx7VoiceBank),
}

impl Dx7Bulk {
    /// Parses a DX7 bulk dump from a SysEx event.
    pub fn from_event(event: &SysExEvent) -> Result<Self, SysExViewError> {
        Self::from_yamaha(YamahaSysEx::from_event(event)?)
    }

    /// Parses a DX7 bulk dump from a Yamaha message, dispatching on the format
    /// byte and payload length.
    pub fn from_yamaha(message: YamahaSysEx) -> Result<Self, SysExViewError> {
        match (message.format, message.data.len()) {
            (DX7_SINGLE_VOICE_FORMAT, DX7_SINGLE_VOICE_BYTE_COUNT) => {
                Ok(Self::SingleVoice(Dx7Voice::from_edit_buffer(message.data)?))
            }
            (DX7_SINGLE_VOICE_FORMAT, actual) => Err(SysExViewError::InvalidDx7Length {
                context: "single voice",
                expected: DX7_SINGLE_VOICE_BYTE_COUNT,
                actual,
            }),
            (DX7_VOICE_BANK_FORMAT, DX7_BANK_BYTE_COUNT) => Ok(Self::VoiceBank(
                Dx7VoiceBank::from_packed_bank(message.data)?,
            )),
            (DX7_VOICE_BANK_FORMAT, actual) => Err(SysExViewError::InvalidDx7Length {
                context: "voice bank",
                expected: DX7_BANK_BYTE_COUNT,
                actual,
            }),
            (format, _) => Err(SysExViewError::UnsupportedYamahaFormat { format }),
        }
    }

    /// Serialises this bulk dump into a [`YamahaSysEx`] with the given
    /// sub-status byte.
    pub fn to_yamaha(&self, sub_status: u8) -> Result<YamahaSysEx, SysExViewError> {
        match self {
            Self::SingleVoice(voice) => YamahaSysEx::new(
                sub_status,
                DX7_SINGLE_VOICE_FORMAT,
                voice.edit_buffer().to_vec(),
            ),
            Self::VoiceBank(bank) => {
                YamahaSysEx::new(sub_status, DX7_VOICE_BANK_FORMAT, bank.to_packed_bank())
            }
        }
    }
}

/// A DX7 bank of 32 voices, retaining the raw packed bytes it was parsed from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Dx7VoiceBank {
    voices: Vec<Dx7Voice>,
    raw_packed: Vec<u8>,
}

impl Dx7VoiceBank {
    /// Parses a bank from its packed byte form (32 packed voices).
    pub fn from_packed_bank(data: Vec<u8>) -> Result<Self, SysExViewError> {
        validate_exact_len("voice bank", DX7_BANK_BYTE_COUNT, data.len())?;
        validate_data_bytes(0, &data)?;
        let voices = data
            .chunks_exact(DX7_PACKED_VOICE_BYTE_COUNT)
            .map(|chunk| Dx7Voice::from_packed_voice(chunk.to_vec()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            voices,
            raw_packed: data,
        })
    }

    /// Builds a bank from exactly [`DX7_BANK_VOICE_COUNT`] voices, repacking
    /// them into the raw bank bytes.
    pub fn from_voices(voices: Vec<Dx7Voice>) -> Result<Self, SysExViewError> {
        validate_exact_len("voice bank", DX7_BANK_VOICE_COUNT, voices.len())?;
        let mut raw_packed = Vec::with_capacity(DX7_BANK_BYTE_COUNT);
        for voice in &voices {
            raw_packed.extend_from_slice(&voice.packed_voice());
        }
        Ok(Self { voices, raw_packed })
    }

    /// Returns the unpacked voices in the bank.
    pub fn voices(&self) -> &[Dx7Voice] {
        &self.voices
    }

    /// Returns the raw packed bank bytes this value was built from.
    pub fn raw_packed_bank(&self) -> &[u8] {
        &self.raw_packed
    }

    /// Repacks the current voices into bank byte form.
    pub fn to_packed_bank(&self) -> Vec<u8> {
        self.voices
            .iter()
            .flat_map(Dx7Voice::packed_voice)
            .collect()
    }
}

/// A single DX7 voice, stored as its unpacked edit buffer with the original
/// packed bytes retained when it was parsed from a bank.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Dx7Voice {
    edit_buffer: Vec<u8>,
    raw_packed: Option<Vec<u8>>,
}

impl Dx7Voice {
    /// Parses a voice from its unpacked edit-buffer bytes.
    pub fn from_edit_buffer(data: Vec<u8>) -> Result<Self, SysExViewError> {
        validate_exact_len("single voice", DX7_SINGLE_VOICE_BYTE_COUNT, data.len())?;
        validate_data_bytes(0, &data)?;
        Ok(Self {
            edit_buffer: data,
            raw_packed: None,
        })
    }

    /// Parses a voice from its packed bank bytes, unpacking the edit buffer and
    /// retaining the packed form.
    pub fn from_packed_voice(data: Vec<u8>) -> Result<Self, SysExViewError> {
        validate_exact_len("packed voice", DX7_PACKED_VOICE_BYTE_COUNT, data.len())?;
        validate_data_bytes(0, &data)?;
        Ok(Self {
            edit_buffer: unpack_packed_voice(&data),
            raw_packed: Some(data),
        })
    }

    /// Returns the unpacked edit-buffer bytes.
    pub fn edit_buffer(&self) -> &[u8] {
        &self.edit_buffer
    }

    /// Returns the original packed bytes, if this voice came from a packed bank.
    pub fn raw_packed_voice(&self) -> Option<&[u8]> {
        self.raw_packed.as_deref()
    }

    /// Packs the edit buffer into the 128-byte packed voice form.
    pub fn packed_voice(&self) -> Vec<u8> {
        pack_edit_buffer(&self.edit_buffer)
    }

    /// Returns all six operators as decoded structures.
    pub fn operators(&self) -> Vec<Dx7Operator> {
        (0..DX7_OPERATOR_COUNT)
            .map(|index| self.operator(index))
            .collect()
    }

    /// Decodes the operator at `index` (0-based) from the edit buffer.
    pub fn operator(&self, index: usize) -> Dx7Operator {
        let base = index * DX7_OPERATOR_EDIT_BYTE_COUNT;
        Dx7Operator {
            rates: array4(&self.edit_buffer[base..base + 4]),
            levels: array4(&self.edit_buffer[base + 4..base + 8]),
            breakpoint: self.edit_buffer[base + 8],
            left_depth: self.edit_buffer[base + 9],
            right_depth: self.edit_buffer[base + 10],
            left_curve: self.edit_buffer[base + 11],
            right_curve: self.edit_buffer[base + 12],
            rate_scale: self.edit_buffer[base + 13],
            amp_mod_sens: self.edit_buffer[base + 14],
            key_velocity_sens: self.edit_buffer[base + 15],
            output_level: self.edit_buffer[base + 16],
            oscillator_mode: self.edit_buffer[base + 17],
            frequency_coarse: self.edit_buffer[base + 18],
            frequency_fine: self.edit_buffer[base + 19],
            detune: self.edit_buffer[base + 20],
        }
    }

    /// Decodes the voice-wide (common) parameters from the edit buffer.
    pub fn common(&self) -> Dx7VoiceCommon {
        Dx7VoiceCommon {
            pitch_rates: array4(&self.edit_buffer[126..130]),
            pitch_levels: array4(&self.edit_buffer[130..134]),
            algorithm: self.edit_buffer[134],
            feedback: self.edit_buffer[135],
            oscillator_sync: self.edit_buffer[136] != 0,
            lfo_speed: self.edit_buffer[137],
            lfo_delay: self.edit_buffer[138],
            lfo_pitch_mod_depth: self.edit_buffer[139],
            lfo_amp_mod_depth: self.edit_buffer[140],
            lfo_sync: self.edit_buffer[141] != 0,
            lfo_waveform: self.edit_buffer[142],
            pitch_mod_sens: self.edit_buffer[143],
            transpose: self.edit_buffer[144],
            name: self.name(),
            name_bytes: array10(&self.edit_buffer[145..155]),
        }
    }

    /// Returns the 10-character voice name, trimmed of trailing whitespace.
    pub fn name(&self) -> String {
        String::from_utf8_lossy(&self.edit_buffer[145..155])
            .trim_end()
            .to_owned()
    }
}

/// One DX7 operator's decoded envelope and oscillator parameters.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Dx7Operator {
    /// Envelope generator rates (R1-R4).
    pub rates: [u8; 4],
    /// Envelope generator levels (L1-L4).
    pub levels: [u8; 4],
    /// Keyboard level scaling breakpoint note.
    pub breakpoint: u8,
    /// Level scaling depth to the left of the breakpoint.
    pub left_depth: u8,
    /// Level scaling depth to the right of the breakpoint.
    pub right_depth: u8,
    /// Level scaling curve to the left of the breakpoint.
    pub left_curve: u8,
    /// Level scaling curve to the right of the breakpoint.
    pub right_curve: u8,
    /// Keyboard rate scaling.
    pub rate_scale: u8,
    /// Amplitude modulation sensitivity.
    pub amp_mod_sens: u8,
    /// Key velocity sensitivity.
    pub key_velocity_sens: u8,
    /// Operator output level.
    pub output_level: u8,
    /// Oscillator mode (ratio or fixed frequency).
    pub oscillator_mode: u8,
    /// Coarse frequency.
    pub frequency_coarse: u8,
    /// Fine frequency.
    pub frequency_fine: u8,
    /// Detune.
    pub detune: u8,
}

/// The voice-wide DX7 parameters shared across operators.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Dx7VoiceCommon {
    /// Pitch envelope generator rates.
    pub pitch_rates: [u8; 4],
    /// Pitch envelope generator levels.
    pub pitch_levels: [u8; 4],
    /// Algorithm number selecting operator routing.
    pub algorithm: u8,
    /// Feedback amount.
    pub feedback: u8,
    /// Oscillator key sync flag.
    pub oscillator_sync: bool,
    /// LFO speed.
    pub lfo_speed: u8,
    /// LFO delay.
    pub lfo_delay: u8,
    /// LFO pitch modulation depth.
    pub lfo_pitch_mod_depth: u8,
    /// LFO amplitude modulation depth.
    pub lfo_amp_mod_depth: u8,
    /// LFO key sync flag.
    pub lfo_sync: bool,
    /// LFO waveform selector.
    pub lfo_waveform: u8,
    /// Pitch modulation sensitivity.
    pub pitch_mod_sens: u8,
    /// Transpose in semitones (offset form).
    pub transpose: u8,
    /// Voice name, trimmed.
    pub name: String,
    /// Raw 10-byte voice name field.
    pub name_bytes: [u8; 10],
}

/// Computes the Yamaha SysEx checksum (two's-complement of the 7-bit sum).
pub fn yamaha_checksum(data: &[u8]) -> u8 {
    let sum = data.iter().fold(0_u32, |sum, byte| sum + u32::from(*byte));
    ((0x80 - (sum & 0x7f)) & 0x7f) as u8
}

fn yamaha_byte_count(msb: u8, lsb: u8) -> usize {
    (usize::from(msb) << 7) | usize::from(lsb)
}

fn yamaha_count_bytes(len: usize) -> Result<(u8, u8), SysExViewError> {
    if len > 0x3fff {
        return Err(SysExViewError::InvalidByteCount {
            expected: 0x3fff,
            actual: len,
        });
    }
    Ok((((len >> 7) & 0x7f) as u8, (len & 0x7f) as u8))
}

fn validate_exact_len(
    context: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), SysExViewError> {
    if expected == actual {
        Ok(())
    } else {
        Err(SysExViewError::InvalidDx7Length {
            context,
            expected,
            actual,
        })
    }
}

fn unpack_packed_voice(packed: &[u8]) -> Vec<u8> {
    let mut edit = vec![0; DX7_SINGLE_VOICE_BYTE_COUNT];
    for operator in 0..DX7_OPERATOR_COUNT {
        let packed_base = operator * DX7_OPERATOR_PACKED_BYTE_COUNT;
        let edit_base = operator * DX7_OPERATOR_EDIT_BYTE_COUNT;
        edit[edit_base..edit_base + 11].copy_from_slice(&packed[packed_base..packed_base + 11]);
        edit[edit_base + 11] = packed[packed_base + 11] & 0x03;
        edit[edit_base + 12] = (packed[packed_base + 11] >> 2) & 0x03;
        edit[edit_base + 13] = packed[packed_base + 12] & 0x07;
        edit[edit_base + 20] = (packed[packed_base + 12] >> 3) & 0x0f;
        edit[edit_base + 14] = packed[packed_base + 13] & 0x03;
        edit[edit_base + 15] = (packed[packed_base + 13] >> 2) & 0x07;
        edit[edit_base + 16] = packed[packed_base + 14] & 0x7f;
        edit[edit_base + 17] = packed[packed_base + 15] & 0x01;
        edit[edit_base + 18] = (packed[packed_base + 15] >> 1) & 0x1f;
        edit[edit_base + 19] = packed[packed_base + 16] & 0x7f;
    }
    edit[126..134].copy_from_slice(&packed[102..110]);
    edit[134] = packed[110] & 0x1f;
    edit[135] = packed[111] & 0x07;
    edit[136] = (packed[111] >> 3) & 0x01;
    edit[137..141].copy_from_slice(&packed[112..116]);
    edit[141] = packed[116] & 0x01;
    edit[142] = (packed[116] >> 1) & 0x07;
    edit[143] = (packed[116] >> 4) & 0x07;
    edit[144] = packed[117] & 0x7f;
    edit[145..155].copy_from_slice(&packed[118..128]);
    edit
}

fn pack_edit_buffer(edit: &[u8]) -> Vec<u8> {
    let mut packed = vec![0; DX7_PACKED_VOICE_BYTE_COUNT];
    for operator in 0..DX7_OPERATOR_COUNT {
        let packed_base = operator * DX7_OPERATOR_PACKED_BYTE_COUNT;
        let edit_base = operator * DX7_OPERATOR_EDIT_BYTE_COUNT;
        packed[packed_base..packed_base + 11].copy_from_slice(&edit[edit_base..edit_base + 11]);
        packed[packed_base + 11] =
            (edit[edit_base + 11] & 0x03) | ((edit[edit_base + 12] & 0x03) << 2);
        packed[packed_base + 12] =
            (edit[edit_base + 13] & 0x07) | ((edit[edit_base + 20] & 0x0f) << 3);
        packed[packed_base + 13] =
            (edit[edit_base + 14] & 0x03) | ((edit[edit_base + 15] & 0x07) << 2);
        packed[packed_base + 14] = edit[edit_base + 16] & 0x7f;
        packed[packed_base + 15] =
            (edit[edit_base + 17] & 0x01) | ((edit[edit_base + 18] & 0x1f) << 1);
        packed[packed_base + 16] = edit[edit_base + 19] & 0x7f;
    }
    packed[102..110].copy_from_slice(&edit[126..134]);
    packed[110] = edit[134] & 0x1f;
    packed[111] = (edit[135] & 0x07) | ((edit[136] & 0x01) << 3);
    packed[112..116].copy_from_slice(&edit[137..141]);
    packed[116] = (edit[141] & 0x01) | ((edit[142] & 0x07) << 1) | ((edit[143] & 0x07) << 4);
    packed[117] = edit[144] & 0x7f;
    packed[118..128].copy_from_slice(&edit[145..155]);
    packed
}

fn array4(bytes: &[u8]) -> [u8; 4] {
    bytes.try_into().expect("DX7 parser uses 4-byte slices")
}

fn array10(bytes: &[u8]) -> [u8; 10] {
    bytes.try_into().expect("DX7 parser uses 10-byte slices")
}
