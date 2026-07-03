use sim_kernel::{Error, Result};
use sim_lib_midi_core::{MidiEvent, TickTime, synthetic_origin};
use sim_lib_midi_rtmidi::{bytes_from_payload, payload_from_bytes};

/// Encodes one MIDI event as a BLE-MIDI timestamp packet.
pub fn encode_ble_midi_event(event: &MidiEvent) -> Result<Vec<u8>> {
    let timestamp = event.time.ticks.rem_euclid(8192) as u16;
    let mut payload = bytes_from_payload(&event.payload)
        .map_err(|error| Error::Eval(format!("BLE-MIDI payload encode failed: {error}")))?;
    if payload.is_empty() {
        return Err(Error::Eval(
            "BLE-MIDI payload must include a status byte".to_owned(),
        ));
    }
    let mut packet = Vec::with_capacity(payload.len() + 2);
    packet.push(0x80 | (((timestamp >> 7) as u8) & 0x3f));
    packet.push(0x80 | ((timestamp as u8) & 0x7f));
    packet.append(&mut payload);
    Ok(packet)
}

/// Decodes BLE-MIDI timestamp packets into MIDI events.
pub fn decode_ble_midi_packet(packet: &[u8], tpq: u32) -> Result<Vec<MidiEvent>> {
    if tpq == 0 {
        return Err(Error::Eval(
            "BLE-MIDI decode TPQ must be greater than zero".to_owned(),
        ));
    }
    let Some((&header, rest)) = packet.split_first() else {
        return Err(Error::Eval(
            "BLE-MIDI packet must contain a timestamp header".to_owned(),
        ));
    };
    if header & 0x80 == 0 {
        return Err(Error::Eval(
            "BLE-MIDI timestamp header must set the high bit".to_owned(),
        ));
    }
    let mut cursor = 0usize;
    let mut events = Vec::new();
    while cursor < rest.len() {
        let timestamp_low = rest[cursor];
        if timestamp_low & 0x80 == 0 {
            return Err(Error::Eval(
                "BLE-MIDI timestamp byte must set the high bit".to_owned(),
            ));
        }
        cursor += 1;
        if cursor >= rest.len() {
            return Err(Error::Eval(
                "BLE-MIDI timestamp byte must be followed by a MIDI status byte".to_owned(),
            ));
        }
        let message_len = midi_message_len(&rest[cursor..])?;
        let message = &rest[cursor..cursor + message_len];
        cursor += message_len;
        let ticks = (u16::from(header & 0x3f) << 7) | u16::from(timestamp_low & 0x7f);
        events.push(MidiEvent {
            time: TickTime::new(i64::from(ticks), tpq)
                .map_err(|error| Error::Eval(format!("invalid BLE-MIDI timestamp: {error}")))?,
            origin: synthetic_origin(),
            payload: payload_from_bytes(message)
                .map_err(|error| Error::Eval(format!("BLE-MIDI payload decode failed: {error}")))?,
        });
    }
    Ok(events)
}

fn midi_message_len(bytes: &[u8]) -> Result<usize> {
    let Some(&status) = bytes.first() else {
        return Err(Error::Eval(
            "BLE-MIDI packet ended before MIDI status byte".to_owned(),
        ));
    };
    if status & 0x80 == 0 {
        return Err(Error::Eval(
            "BLE-MIDI MIDI message must start with a status byte".to_owned(),
        ));
    }
    let expected = match status & 0xf0 {
        0x80 | 0x90 | 0xa0 | 0xb0 | 0xe0 => 3,
        0xc0 | 0xd0 => 2,
        _ if status >= 0xf8 => 1,
        _ => bytes.len(),
    };
    if bytes.len() < expected {
        return Err(Error::Eval(format!(
            "BLE-MIDI message status 0x{status:02x} expected {expected} bytes"
        )));
    }
    Ok(expected)
}
