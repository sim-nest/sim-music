use std::fmt::Debug;

use sim_kernel::{Error, Result};
use sim_lib_midi_core::{MidiEvent, MidiSource};
use sim_lib_stream_core::{
    MidiPacket, MidiPacketEvent, StreamEnvelope, StreamItem, StreamMetadata, StreamPacket,
    TransportProfile,
};

use crate::codec::payload_to_bytes;

/// Encodes a single MIDI event into a stream [`MidiPacketEvent`].
///
/// Errors when the event's ticks-per-quarter exceeds the stream packet range.
pub fn midi_event_to_packet_event(event: &MidiEvent) -> Result<MidiPacketEvent> {
    MidiPacketEvent::new(
        event.time.ticks,
        tpq_u16(event.time.tpq)?,
        payload_to_bytes(&event.payload),
    )
}

/// Wraps a MIDI packet as a sequenced LAN MIDI-control transport envelope.
pub fn midi_packet_to_lan_control_envelope(
    metadata: &StreamMetadata,
    sequence: u64,
    packet: MidiPacket,
) -> Result<StreamEnvelope> {
    let item = StreamItem::new(StreamPacket::Midi(packet));
    StreamEnvelope::from_item_with_profile(
        metadata,
        sequence,
        &item,
        TransportProfile::lan_midi_control(),
    )
}

/// Drains a MIDI source into MIDI packets of at most `max_events` events each.
///
/// A new packet is started whenever the current one fills or the ticks-per-quarter
/// changes. Errors when `max_events` is zero or the source yields an error.
pub fn packetize_midi_source<S>(source: &mut S, max_events: usize) -> Result<Vec<MidiPacket>>
where
    S: MidiSource,
    S::Err: Debug,
{
    if max_events == 0 {
        return Err(Error::Eval(
            "MIDI packet max-events must be greater than zero".to_owned(),
        ));
    }
    let mut out = Vec::new();
    let mut current = PacketBuilder::new(max_events);
    while let Some(event) = source
        .next()
        .map_err(|err| Error::Eval(format!("MIDI source error: {err:?}")))?
    {
        if let Some(packet) = current.push(midi_event_to_packet_event(&event)?)? {
            out.push(packet);
        }
    }
    if let Some(packet) = current.finish()? {
        out.push(packet);
    }
    Ok(out)
}

pub(crate) struct PacketBuilder {
    max_events: usize,
    tpq: Option<u16>,
    events: Vec<MidiPacketEvent>,
}

impl PacketBuilder {
    pub(crate) fn new(max_events: usize) -> Self {
        Self {
            max_events,
            tpq: None,
            events: Vec::new(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub(crate) fn accepts_tpq(&self, tpq: u16) -> bool {
        self.tpq.is_none_or(|current| current == tpq)
    }

    pub(crate) fn is_full(&self) -> bool {
        self.events.len() >= self.max_events
    }

    pub(crate) fn push(&mut self, event: MidiPacketEvent) -> Result<Option<MidiPacket>> {
        let mut ready = None;
        if !self.is_empty() && (!self.accepts_tpq(event.tpq()) || self.is_full()) {
            ready = self.finish()?;
        }
        self.tpq = Some(event.tpq());
        self.events.push(event);
        Ok(ready)
    }

    pub(crate) fn finish(&mut self) -> Result<Option<MidiPacket>> {
        if self.events.is_empty() {
            return Ok(None);
        }
        self.tpq = None;
        let events = std::mem::take(&mut self.events);
        MidiPacket::new(events).map(Some)
    }
}

pub(crate) fn tpq_u16(tpq: u32) -> Result<u16> {
    u16::try_from(tpq)
        .map_err(|_| Error::Eval(format!("MIDI TPQ {tpq} exceeds stream packet range")))
}
