use sim_kernel::{Error, Result};
use sim_lib_midi_core::{
    Channel, ChannelMessage, MidiEvent, MidiPayload, MidiSink, MidiSource, RawBytes, SysExEvent,
    U7, U14, synthetic_origin,
};

use crate::{RtmidiEvent, RtmidiTiming};

/// MIDI input wrapper opened through the RtMidi adapter.
#[derive(Clone, Debug)]
pub struct RtmidiMidiSource {
    tpq: u32,
    events: Vec<MidiEvent>,
    cursor: usize,
}

/// MIDI output wrapper opened through the RtMidi adapter.
#[derive(Clone, Debug)]
pub struct RtmidiMidiSink {
    tpq: u32,
    events: Vec<MidiEvent>,
    flushed: bool,
}

impl RtmidiMidiSource {
    /// Builds a source from raw RtMidi events, converting timestamps to ticks
    /// and decoding each payload, sorted by tick.
    pub fn from_events(timing: RtmidiTiming, events: Vec<RtmidiEvent>) -> Result<Self> {
        let mut events = events
            .into_iter()
            .map(|event| {
                Ok(MidiEvent {
                    time: timing.timestamp_to_ticks(event.timestamp_micros()),
                    origin: synthetic_origin(),
                    payload: payload_from_bytes(event.bytes())?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        events.sort_by_key(|event| event.time.ticks);
        Ok(Self {
            tpq: timing.tpq(),
            events,
            cursor: 0,
        })
    }

    /// Returns the decoded events held by this source.
    pub fn events(&self) -> &[MidiEvent] {
        &self.events
    }
}

impl MidiSource for RtmidiMidiSource {
    type Err = Error;

    fn tpq(&self) -> u32 {
        self.tpq
    }

    fn next(&mut self) -> std::result::Result<Option<MidiEvent>, Self::Err> {
        let event = self.events.get(self.cursor).cloned();
        if event.is_some() {
            self.cursor += 1;
        }
        Ok(event)
    }
}

impl RtmidiMidiSink {
    /// Creates a sink at `tpq` resolution, failing if `tpq` is zero.
    pub fn new(tpq: u32) -> Result<Self> {
        if tpq == 0 {
            return Err(Error::Eval(
                "RtMidi sink TPQ must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            tpq,
            events: Vec::new(),
            flushed: false,
        })
    }

    /// Returns the events written to this sink, sorted by tick.
    pub fn events(&self) -> &[MidiEvent] {
        &self.events
    }

    /// Returns whether the sink has been flushed.
    pub fn flushed(&self) -> bool {
        self.flushed
    }
}

impl MidiSink for RtmidiMidiSink {
    type Err = Error;

    fn tpq(&self) -> u32 {
        self.tpq
    }

    fn write(&mut self, event: &MidiEvent) -> std::result::Result<(), Self::Err> {
        let mut event = event.clone();
        if event.time.tpq != self.tpq {
            event.time = event.time.quantize(self.tpq);
        }
        self.events.push(event);
        self.events.sort_by_key(|event| event.time.ticks);
        Ok(())
    }

    fn flush(&mut self) -> std::result::Result<(), Self::Err> {
        self.flushed = true;
        Ok(())
    }
}

/// Converts a raw MIDI byte message into the existing MIDI payload model.
pub fn payload_from_bytes(bytes: &[u8]) -> Result<MidiPayload> {
    let Some((&status, data)) = bytes.split_first() else {
        return Err(Error::Eval("RtMidi event had no status byte".to_owned()));
    };
    let channel = || Channel::new(status & 0x0f).map_err(midi_error);
    let u7 = |value: u8| U7::try_from(u16::from(value)).map_err(midi_error);
    match status & 0xf0 {
        0x80 if data.len() >= 2 => Ok(MidiPayload::Channel(ChannelMessage::NoteOff {
            ch: channel()?,
            key: u7(data[0])?,
            vel: u7(data[1])?,
        })),
        0x90 if data.len() >= 2 => Ok(MidiPayload::Channel(ChannelMessage::NoteOn {
            ch: channel()?,
            key: u7(data[0])?,
            vel: u7(data[1])?,
        })),
        0xa0 if data.len() >= 2 => Ok(MidiPayload::Channel(ChannelMessage::PolyAftertouch {
            ch: channel()?,
            key: u7(data[0])?,
            pressure: u7(data[1])?,
        })),
        0xb0 if data.len() >= 2 => Ok(MidiPayload::Channel(ChannelMessage::ControlChange {
            ch: channel()?,
            cc: u7(data[0])?,
            value: u7(data[1])?,
        })),
        0xc0 if !data.is_empty() => Ok(MidiPayload::Channel(ChannelMessage::ProgramChange {
            ch: channel()?,
            program: u7(data[0])?,
        })),
        0xd0 if !data.is_empty() => Ok(MidiPayload::Channel(ChannelMessage::ChanAftertouch {
            ch: channel()?,
            pressure: u7(data[0])?,
        })),
        0xe0 if data.len() >= 2 => {
            let value = u16::from(data[0] & 0x7f) | (u16::from(data[1] & 0x7f) << 7);
            Ok(MidiPayload::Channel(ChannelMessage::PitchBend {
                ch: channel()?,
                value: U14::try_from(value).map_err(midi_error)?,
            }))
        }
        _ => Ok(MidiPayload::Raw(RawBytes {
            status,
            data: data.to_vec(),
        })),
    }
}

/// Converts an existing MIDI payload into the bytes sent by RtMidi.
pub fn bytes_from_payload(payload: &MidiPayload) -> Result<Vec<u8>> {
    match payload {
        MidiPayload::Channel(message) => Ok(bytes_from_channel_message(*message)),
        MidiPayload::SysEx(SysExEvent::F0 { data }) => {
            let mut bytes = Vec::with_capacity(data.len() + 1);
            bytes.push(0xf0);
            bytes.extend_from_slice(data);
            Ok(bytes)
        }
        MidiPayload::SysEx(SysExEvent::F7 { data }) => {
            let mut bytes = Vec::with_capacity(data.len() + 1);
            bytes.push(0xf7);
            bytes.extend_from_slice(data);
            Ok(bytes)
        }
        MidiPayload::Raw(raw) => {
            let mut bytes = Vec::with_capacity(raw.data.len() + 1);
            bytes.push(raw.status);
            bytes.extend_from_slice(&raw.data);
            Ok(bytes)
        }
        MidiPayload::Meta(_) => Err(Error::Eval(
            "RtMidi cannot send MIDI meta events to a live port".to_owned(),
        )),
    }
}

fn bytes_from_channel_message(message: ChannelMessage) -> Vec<u8> {
    match message {
        ChannelMessage::NoteOff { ch, key, vel } => vec![0x80 | ch.0, key.0, vel.0],
        ChannelMessage::NoteOn { ch, key, vel } => vec![0x90 | ch.0, key.0, vel.0],
        ChannelMessage::PolyAftertouch { ch, key, pressure } => {
            vec![0xa0 | ch.0, key.0, pressure.0]
        }
        ChannelMessage::ControlChange { ch, cc, value } => vec![0xb0 | ch.0, cc.0, value.0],
        ChannelMessage::ProgramChange { ch, program } => vec![0xc0 | ch.0, program.0],
        ChannelMessage::ChanAftertouch { ch, pressure } => vec![0xd0 | ch.0, pressure.0],
        ChannelMessage::PitchBend { ch, value } => {
            let lsb = (value.0 & 0x7f) as u8;
            let msb = ((value.0 >> 7) & 0x7f) as u8;
            vec![0xe0 | ch.0, lsb, msb]
        }
    }
}

fn midi_error(error: sim_lib_midi_core::MidiError) -> Error {
    Error::Eval(format!("invalid RtMidi payload: {error}"))
}
