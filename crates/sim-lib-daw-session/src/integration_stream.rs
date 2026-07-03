use sim_kernel::{Error, Result, Symbol};
use sim_lib_music_core::{PlayContext, PlayEvent};
use sim_lib_stream_core::{
    BufferPolicy, ClockDomain, MidiPacket, MidiPacketEvent, StreamDirection, StreamEnvelope,
    StreamItem, StreamMedia, StreamMetadata, StreamPacket, TransportProfile,
};
use sim_lib_stream_midi::midi_event_to_packet_event;

/// Owned plugin event data exported from generated performance events.
#[derive(Clone, Debug, PartialEq)]
pub enum DawPluginEventExport {
    /// A raw MIDI message addressed to a plugin at a tick offset.
    Midi {
        /// Tick offset from the play-context start.
        offset: u32,
        /// Encoded MIDI bytes.
        bytes: Vec<u8>,
    },
    /// A note-on event.
    NoteOn {
        /// Tick offset from the play-context start.
        offset: u32,
        /// MIDI channel.
        channel: u8,
        /// MIDI key number.
        key: u8,
        /// Normalized velocity in `0.0..=1.0`.
        velocity: f32,
    },
    /// A note-off event (always emitted with zero velocity).
    NoteOff {
        /// Tick offset from the play-context start.
        offset: u32,
        /// MIDI channel.
        channel: u8,
        /// MIDI key number.
        key: u8,
        /// Normalized velocity, always `0.0`.
        velocity: f32,
    },
    /// A parameter-set event targeting a plugin control symbol.
    ParamSet {
        /// Tick offset from the play-context start.
        offset: u32,
        /// Control parameter symbol being set.
        target: Symbol,
        /// New parameter value.
        value: f64,
    },
}

pub(crate) fn event_stream_envelopes(events: &[PlayEvent]) -> Result<Vec<StreamEnvelope>> {
    let metadata = StreamMetadata::new(
        Symbol::qualified("daw/performance-stream", "events"),
        StreamMedia::Data,
        StreamDirection::Source,
        ClockDomain::MidiTick.symbol(),
        BufferPolicy::bounded(events.len().max(1))?,
    );
    events
        .iter()
        .enumerate()
        .map(|(sequence, event)| {
            let item = event.to_stream_item(ClockDomain::MidiTick.symbol())?;
            StreamEnvelope::from_item_with_profile(
                &metadata,
                sequence as u64,
                &item,
                TransportProfile::remote_stream_fabric(),
            )
        })
        .collect()
}

pub(crate) fn midi_stream_envelopes(events: &[PlayEvent]) -> Result<Vec<StreamEnvelope>> {
    let packets = midi_packets(events)?;
    let metadata = StreamMetadata::new(
        Symbol::qualified("daw/performance-stream", "midi-control"),
        StreamMedia::Midi,
        StreamDirection::Source,
        ClockDomain::MidiTick.symbol(),
        BufferPolicy::bounded(packets.len().max(1))?,
    );
    packets
        .into_iter()
        .enumerate()
        .map(|(sequence, packet)| {
            let item = StreamItem::new(StreamPacket::Midi(packet));
            StreamEnvelope::from_item_with_profile(
                &metadata,
                sequence as u64,
                &item,
                TransportProfile::lan_midi_control(),
            )
        })
        .collect()
}

pub(crate) fn plugin_event_exports(
    events: &[PlayEvent],
    cx: &PlayContext,
) -> Result<Vec<DawPluginEventExport>> {
    let mut out = Vec::new();
    for event in events {
        match event {
            PlayEvent::Note(note) => {
                let Some(key) = note.pitch.to_midi() else {
                    continue;
                };
                let velocity = f32::from(note.velocity) / 127.0;
                out.push(DawPluginEventExport::NoteOn {
                    offset: tick_offset(note.time, cx),
                    channel: note.channel.0,
                    key,
                    velocity,
                });
                out.push(DawPluginEventExport::NoteOff {
                    offset: tick_offset(note.time + note.duration, cx),
                    channel: note.channel.0,
                    key,
                    velocity: 0.0,
                });
            }
            PlayEvent::Midi(midi) => {
                let packet_event = midi_event_to_packet_event(&midi.event)?;
                out.push(DawPluginEventExport::Midi {
                    offset: tick_offset(midi.event.time, cx),
                    bytes: packet_event.bytes().to_vec(),
                });
            }
            PlayEvent::Control(control) => out.push(DawPluginEventExport::ParamSet {
                offset: tick_offset(control.time, cx),
                target: control.control.clone(),
                value: control.value as f64,
            }),
            PlayEvent::Pitch(pitch) => out.push(DawPluginEventExport::ParamSet {
                offset: tick_offset(pitch.time, cx),
                target: Symbol::qualified("music/plugin-param", "pitch"),
                value: pitch.pitch.semitone() as f64,
            }),
            _ => {}
        }
    }
    Ok(out)
}

fn midi_packets(events: &[PlayEvent]) -> Result<Vec<MidiPacket>> {
    let mut packet_events = Vec::new();
    for event in events {
        match event {
            PlayEvent::Note(note) => {
                let Some(key) = note.pitch.to_midi() else {
                    continue;
                };
                let status = 0x90 | note.channel.0;
                packet_events.push(MidiPacketEvent::new(
                    note.time.ticks,
                    tpq_u16(note.time.tpq)?,
                    vec![status, key, note.velocity],
                )?);
                packet_events.push(MidiPacketEvent::new(
                    (note.time + note.duration).ticks,
                    tpq_u16(note.time.tpq)?,
                    vec![0x80 | note.channel.0, key, 0],
                )?);
            }
            PlayEvent::Midi(midi) => packet_events.push(midi_event_to_packet_event(&midi.event)?),
            _ => {}
        }
    }
    packet_events.sort_by(|left, right| {
        left.ticks()
            .cmp(&right.ticks())
            .then_with(|| left.bytes().cmp(right.bytes()))
    });

    let mut packets = Vec::new();
    let mut current_tpq = None;
    let mut current = Vec::new();
    for event in packet_events {
        if current_tpq.is_some_and(|tpq| tpq != event.tpq()) {
            packets.push(MidiPacket::new(std::mem::take(&mut current))?);
        }
        current_tpq = Some(event.tpq());
        current.push(event);
    }
    if !current.is_empty() {
        packets.push(MidiPacket::new(current)?);
    }
    Ok(packets)
}

fn tick_offset(tick: sim_lib_music_core::Tick, cx: &PlayContext) -> u32 {
    let tick = tick.quantize(cx.range.start.tpq);
    let delta = tick.ticks.saturating_sub(cx.range.start.ticks).max(0);
    u32::try_from(delta).unwrap_or(u32::MAX)
}

fn tpq_u16(tpq: u32) -> Result<u16> {
    u16::try_from(tpq)
        .map_err(|_| Error::Eval(format!("MIDI TPQ {tpq} exceeds stream packet range")))
}
