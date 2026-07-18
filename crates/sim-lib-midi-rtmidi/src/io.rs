use sim_kernel::{Error, Result};
use sim_lib_midi_core::{
    MidiEvent, MidiPayload, MidiSink, MidiSource, RawBytes, SysExEvent, synthetic_origin, wire,
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
    match status {
        0xf0 => {
            return Ok(MidiPayload::SysEx(SysExEvent::F0 {
                data: data.to_vec(),
            }));
        }
        0xf7 => {
            return Ok(MidiPayload::SysEx(SysExEvent::F7 {
                data: data.to_vec(),
            }));
        }
        _ => {}
    }
    // A live port may deliver a truncated or non-channel buffer; keep those as
    // raw bytes rather than failing, and decode only complete channel messages
    // through the shared `wire::decode_channel`.
    let complete = match status & 0xf0 {
        0x80 | 0x90 | 0xa0 | 0xb0 | 0xe0 => data.len() >= 2,
        0xc0 | 0xd0 => !data.is_empty(),
        _ => false,
    };
    if complete {
        return Ok(MidiPayload::Channel(
            wire::decode_channel(status, data).map_err(midi_error)?,
        ));
    }
    Ok(MidiPayload::Raw(RawBytes {
        status,
        data: data.to_vec(),
    }))
}

/// Converts an existing MIDI payload into the bytes sent by RtMidi.
pub fn bytes_from_payload(payload: &MidiPayload) -> Result<Vec<u8>> {
    match payload {
        MidiPayload::Channel(message) => {
            let (status, data) = wire::encode_channel(message);
            let mut bytes = Vec::with_capacity(data.len() + 1);
            bytes.push(status);
            bytes.extend_from_slice(&data);
            Ok(bytes)
        }
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

fn midi_error(error: sim_lib_midi_core::MidiError) -> Error {
    Error::Eval(format!("invalid RtMidi payload: {error}"))
}
