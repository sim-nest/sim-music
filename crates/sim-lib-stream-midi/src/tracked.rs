use std::fmt::Debug;

use sim_kernel::{Error, Result};
use sim_lib_midi_core::TrackedMidiSource;
use sim_lib_stream_core::MidiPacket;

use crate::packetize::{PacketBuilder, midi_event_to_packet_event};

/// A MIDI packet paired with the source-track index of each of its events.
///
/// # Examples
///
/// ```
/// use sim_lib_stream_core::{MidiPacket, MidiPacketEvent};
/// use sim_lib_stream_midi::TrackedMidiPacket;
///
/// let event = MidiPacketEvent::new(0, 480, vec![0x90, 60, 100]).unwrap();
/// let packet = MidiPacket::new(vec![event]).unwrap();
/// let tracked = TrackedMidiPacket::new(packet, vec![2]).unwrap();
/// assert_eq!(tracked.tracks(), &[2]);
/// assert_eq!(tracked.packet().events().len(), 1);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackedMidiPacket {
    packet: MidiPacket,
    tracks: Vec<usize>,
}

impl TrackedMidiPacket {
    /// Pairs `packet` with per-event `tracks`.
    ///
    /// Errors when the number of track indices does not match the packet's
    /// event count.
    pub fn new(packet: MidiPacket, tracks: Vec<usize>) -> Result<Self> {
        if packet.events().len() != tracks.len() {
            return Err(Error::Eval(
                "tracked MIDI packet track count must match event count".to_owned(),
            ));
        }
        Ok(Self { packet, tracks })
    }

    /// Borrows the underlying MIDI packet.
    pub fn packet(&self) -> &MidiPacket {
        &self.packet
    }

    /// Returns the source-track index for each event in the packet.
    pub fn tracks(&self) -> &[usize] {
        &self.tracks
    }
}

/// Drains a tracked MIDI source into [`TrackedMidiPacket`]s, preserving track
/// provenance.
///
/// Packets carry at most `max_events` events each and are split on a full packet
/// or a ticks-per-quarter change. Errors when `max_events` is zero.
pub fn packetize_tracked_midi_source<S>(
    source: &mut S,
    max_events: usize,
) -> Result<Vec<TrackedMidiPacket>>
where
    S: TrackedMidiSource,
    S::Err: Debug,
{
    if max_events == 0 {
        return Err(Error::Eval(
            "MIDI packet max-events must be greater than zero".to_owned(),
        ));
    }
    let mut out = Vec::new();
    let mut current = TrackedPacketBuilder::new(max_events);
    while let Some(item) = source
        .next_tracked()
        .map_err(|err| Error::Eval(format!("tracked MIDI source error: {err:?}")))?
    {
        if let Some(packet) =
            current.push(midi_event_to_packet_event(&item.event)?, item.last_track)?
        {
            out.push(packet);
        }
    }
    if let Some(packet) = current.finish()? {
        out.push(packet);
    }
    Ok(out)
}

struct TrackedPacketBuilder {
    packets: PacketBuilder,
    tracks: Vec<usize>,
}

impl TrackedPacketBuilder {
    fn new(max_events: usize) -> Self {
        Self {
            packets: PacketBuilder::new(max_events),
            tracks: Vec::new(),
        }
    }

    fn push(
        &mut self,
        event: sim_lib_stream_core::MidiPacketEvent,
        track: usize,
    ) -> Result<Option<TrackedMidiPacket>> {
        let mut ready = None;
        if !self.packets.is_empty()
            && (!self.packets.accepts_tpq(event.tpq()) || self.packets.is_full())
        {
            ready = self.finish()?;
        }
        self.packets.push(event)?;
        self.tracks.push(track);
        Ok(ready)
    }

    fn finish(&mut self) -> Result<Option<TrackedMidiPacket>> {
        let Some(packet) = self.packets.finish()? else {
            return Ok(None);
        };
        let tracks = std::mem::take(&mut self.tracks);
        TrackedMidiPacket::new(packet, tracks).map(Some)
    }
}
