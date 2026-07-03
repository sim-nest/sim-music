#![forbid(unsafe_code)]

use std::io::Read;

use crate::SmfError;

/// Encodes `value` as a MIDI variable-length quantity (big-endian, 7 bits per
/// byte with the high bit marking continuation).
pub fn encode_vlq(mut value: u32) -> Vec<u8> {
    let mut bytes = vec![(value & 0x7f) as u8];
    value >>= 7;
    while value > 0 {
        bytes.push(((value & 0x7f) as u8) | 0x80);
        value >>= 7;
    }
    bytes.reverse();
    bytes
}

/// Decodes a MIDI variable-length quantity from `reader`.
///
/// Fails with [`SmfError::UnexpectedEof`] if the stream ends mid-quantity, or
/// [`SmfError::InvalidVlq`] if it is not terminated within four bytes.
pub fn decode_vlq<R: Read>(reader: &mut R) -> Result<u32, SmfError> {
    let mut value = 0u32;
    for idx in 0..4 {
        let mut byte = [0u8; 1];
        reader
            .read_exact(&mut byte)
            .map_err(|_| SmfError::UnexpectedEof { offset: idx })?;
        value = (value << 7) | u32::from(byte[0] & 0x7f);
        if byte[0] & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(SmfError::InvalidVlq { offset: 0 })
}

pub(crate) fn decode_vlq_at(bytes: &[u8], pos: &mut usize) -> Result<u32, SmfError> {
    let start = *pos;
    let mut value = 0u32;
    for _ in 0..4 {
        let byte = *bytes
            .get(*pos)
            .ok_or(SmfError::UnexpectedEof { offset: *pos })?;
        *pos += 1;
        value = (value << 7) | u32::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(SmfError::InvalidVlq { offset: start })
}
