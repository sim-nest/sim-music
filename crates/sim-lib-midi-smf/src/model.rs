#![forbid(unsafe_code)]

use std::cmp::Ordering;

use sim_lib_midi_core::{
    ChannelMessage, MetaEvent, MidiEvent, MidiPayload, TickTime, TrackedMidiEvent, synthetic_origin,
};

/// The SMF header format field: how the file's tracks relate.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SmfFormat {
    /// Format 0: a single multi-channel track.
    SingleTrack,
    /// Format 1: several tracks played simultaneously.
    Simultaneous,
    /// Format 2: several independent single-track patterns.
    Independent,
}

/// One track: an ordered list of timestamped events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmfTrack {
    /// Events in this track, in absolute time order after canonicalisation.
    pub events: Vec<MidiEvent>,
}

/// A parsed Standard MIDI File: its format, resolution, and tracks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmfFile {
    /// Header format field.
    pub format: SmfFormat,
    /// Resolution in ticks per quarter note.
    pub tpq: u32,
    /// Tracks in file order.
    pub tracks: Vec<SmfTrack>,
}

/// Options controlling SMF serialisation.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct SmfWriteOptions {
    /// When set, omit a redundant status byte for consecutive channel messages
    /// (MIDI running status).
    pub running_status: bool,
}

/// An iterator that merges all tracks of an [`SmfFile`] into a single
/// time-ordered, track-tagged stream.
///
/// Created by [`SmfFile::merge_cursor`]; yields
/// [`TrackedMidiEvent`](sim_lib_midi_core::TrackedMidiEvent)s.
pub struct SmfMergeCursor<'a> {
    file: &'a SmfFile,
    next_index: Vec<usize>,
}

impl SmfFile {
    /// Sorts every track into canonical order and ensures each ends with an
    /// end-of-track meta event.
    pub fn canonicalize(&mut self) {
        for track in &mut self.tracks {
            canonicalize_track(track, self.tpq);
        }
    }

    /// Returns a cursor that merges all tracks into one time-ordered stream.
    pub fn merge_cursor(&self) -> SmfMergeCursor<'_> {
        SmfMergeCursor {
            file: self,
            next_index: vec![0; self.tracks.len()],
        }
    }

    /// Collects every track's events into a single time-ordered,
    /// track-tagged vector.
    pub fn merged_events(&self) -> Vec<TrackedMidiEvent> {
        let mut merged = Vec::new();
        for event in self.merge_cursor() {
            merged.push(event);
        }
        merged
    }
}

impl<'a> Iterator for SmfMergeCursor<'a> {
    type Item = TrackedMidiEvent;

    fn next(&mut self) -> Option<Self::Item> {
        let mut best: Option<(usize, &MidiEvent)> = None;
        for (track_idx, track) in self.file.tracks.iter().enumerate() {
            let event = track.events.get(self.next_index[track_idx])?;
            match best {
                None => best = Some((track_idx, event)),
                Some((best_track, best_event)) => {
                    if compare_event_order(event, track_idx, best_event, best_track)
                        == Ordering::Less
                    {
                        best = Some((track_idx, event));
                    }
                }
            }
        }
        let (track_idx, event) = best?;
        self.next_index[track_idx] += 1;
        Some(TrackedMidiEvent {
            last_track: track_idx,
            event: event.clone(),
        })
    }
}

pub(crate) fn canonicalize_track(track: &mut SmfTrack, tpq: u32) {
    track.events.sort_by(compare_events_same_track);
    if !track
        .events
        .iter()
        .any(|event| matches!(event.payload, MidiPayload::Meta(MetaEvent::EndOfTrack)))
    {
        let last_ticks = track
            .events
            .last()
            .map(|event| event.time.ticks)
            .unwrap_or(0);
        track.events.push(MidiEvent {
            time: TickTime::new(last_ticks, tpq).unwrap_or(TickTime::ZERO),
            origin: synthetic_origin(),
            payload: MidiPayload::Meta(MetaEvent::EndOfTrack),
        });
    }
    track.events.sort_by(compare_events_same_track);
}

pub(crate) fn compare_event_order(
    left: &MidiEvent,
    left_track: usize,
    right: &MidiEvent,
    right_track: usize,
) -> Ordering {
    compare_time(left.time, right.time)
        .then_with(|| event_priority(left).cmp(&event_priority(right)))
        .then_with(|| left_track.cmp(&right_track))
}

fn compare_events_same_track(left: &MidiEvent, right: &MidiEvent) -> Ordering {
    compare_time(left.time, right.time)
        .then_with(|| event_priority(left).cmp(&event_priority(right)))
}

fn compare_time(left: TickTime, right: TickTime) -> Ordering {
    let left_scaled = i128::from(left.ticks) * i128::from(right.tpq);
    let right_scaled = i128::from(right.ticks) * i128::from(left.tpq);
    left_scaled.cmp(&right_scaled)
}

fn event_priority(event: &MidiEvent) -> u8 {
    match event.payload {
        MidiPayload::Meta(MetaEvent::EndOfTrack) => 4,
        MidiPayload::Meta(_) => 0,
        MidiPayload::Channel(ChannelMessage::NoteOff { .. }) => 1,
        MidiPayload::Channel(ChannelMessage::NoteOn { .. }) => 2,
        _ => 3,
    }
}
