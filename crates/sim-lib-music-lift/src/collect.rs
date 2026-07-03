use std::{cmp::Ordering, collections::BTreeMap};

use num_rational::Ratio;
use sim_kernel::{Diagnostic, Severity};
use sim_lib_midi_core::{
    ChannelMessage, MetaEvent, MidiEvent, MidiPayload, TrackedMidiEvent, meta_view,
};
use sim_lib_midi_smf::SmfFile;
use sim_lib_music_core::{
    Articulation, ChannelPressureCell, ControlChangeCell, LaneId, LaneKind, Note, PianoRoll,
    PianoRollCell, PianoRollLane, PitchBendCell, PolyPressureCell, Time, TimedNote,
};

use crate::LiftError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CollectedNote {
    pub onset: Time,
    pub duration: Time,
    pub note: Note,
    pub track: usize,
    pub track_name: Option<String>,
    pub order: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CollectedMidi {
    pub notes: Vec<CollectedNote>,
    pub cells: Vec<PianoRollCell>,
    pub diagnostics: Vec<Diagnostic>,
    pub track_names: Vec<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveNote {
    onset: Time,
    velocity: u8,
    track: usize,
    order: usize,
}

pub(crate) fn collect_midi(file: &SmfFile) -> CollectedMidi {
    let track_names = file
        .tracks
        .iter()
        .map(track_name)
        .collect::<Vec<Option<String>>>();
    let mut diagnostics = Vec::new();
    let mut notes = Vec::new();
    let mut cells = Vec::new();
    let mut active: BTreeMap<(usize, u8, u8), Vec<ActiveNote>> = BTreeMap::new();
    let merged = merged_events(file);
    let end_time = file
        .tracks
        .iter()
        .flat_map(|track| track.events.iter())
        .map(|event| tick_to_time(event.time.ticks, file.tpq))
        .max()
        .unwrap_or_else(|| Time::from_integer(0));

    for (order, tracked) in merged.iter().enumerate() {
        handle_event(
            tracked,
            order,
            &track_names,
            &mut active,
            &mut notes,
            &mut cells,
            &mut diagnostics,
        );
    }

    for ((track, channel, key), stacked) in active {
        for unmatched in stacked {
            diagnostics.push(warning(format!(
                "note-on for key {key} on track {track} channel {channel} closed at end-of-track"
            )));
            notes.push(CollectedNote {
                onset: unmatched.onset,
                duration: end_time - unmatched.onset,
                note: lifted_note(key, unmatched.velocity, channel),
                track,
                track_name: track_names.get(track).cloned().flatten(),
                order: unmatched.order,
            });
        }
    }

    notes.sort_by(|left, right| {
        left.onset
            .cmp(&right.onset)
            .then_with(|| left.track.cmp(&right.track))
            .then_with(|| left.note.pitch.semitone().cmp(&right.note.pitch.semitone()))
            .then_with(|| left.order.cmp(&right.order))
    });

    CollectedMidi {
        notes,
        cells,
        diagnostics,
        track_names,
    }
}

impl CollectedMidi {
    pub(crate) fn to_piano_roll(&self) -> Result<PianoRoll, LiftError> {
        let mut lanes = Vec::new();
        let note_cells = self
            .notes
            .iter()
            .map(|note| {
                PianoRollCell::Note(TimedNote {
                    onset: note.onset,
                    note: Note {
                        duration: note.duration,
                        ..note.note.clone()
                    },
                })
            })
            .collect::<Vec<_>>();
        if !note_cells.is_empty() {
            lanes.push(PianoRollLane::new(
                LaneId::new("midi-notes"),
                LaneKind::Note,
                note_cells,
            )?);
        }
        if !self.cells.is_empty() {
            lanes.push(PianoRollLane::new(
                LaneId::new("midi-controls"),
                LaneKind::Control,
                self.cells.clone(),
            )?);
        }
        PianoRoll::from_lanes(lanes).map_err(LiftError::from)
    }
}

fn handle_event(
    tracked: &TrackedMidiEvent,
    order: usize,
    track_names: &[Option<String>],
    active: &mut BTreeMap<(usize, u8, u8), Vec<ActiveNote>>,
    notes: &mut Vec<CollectedNote>,
    cells: &mut Vec<PianoRollCell>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let time = tick_to_time(tracked.event.time.ticks, tracked.event.time.tpq);
    let track = tracked.last_track;
    match &tracked.event.payload {
        MidiPayload::Channel(ChannelMessage::NoteOn { ch, key, vel }) if vel.0 > 0 => {
            active
                .entry((track, ch.0, key.0))
                .or_default()
                .push(ActiveNote {
                    onset: time,
                    velocity: vel.0,
                    track,
                    order,
                });
        }
        MidiPayload::Channel(ChannelMessage::NoteOff { ch, key, .. })
        | MidiPayload::Channel(ChannelMessage::NoteOn {
            ch,
            key,
            vel: sim_lib_midi_core::U7(0),
        }) => match active.get_mut(&(track, ch.0, key.0)).and_then(Vec::pop) {
            Some(start) => notes.push(CollectedNote {
                onset: start.onset,
                duration: time - start.onset,
                note: lifted_note(key.0, start.velocity, ch.0),
                track: start.track,
                track_name: track_names.get(start.track).cloned().flatten(),
                order: start.order,
            }),
            None => diagnostics.push(warning(format!(
                "orphan note-off for key {} on track {} channel {}",
                key.0, track, ch.0
            ))),
        },
        MidiPayload::Meta(MetaEvent::EndOfTrack) => {}
        MidiPayload::Channel(ChannelMessage::ControlChange { ch, cc, value }) => {
            cells.push(PianoRollCell::ControlChange(ControlChangeCell {
                time,
                channel: *ch,
                controller: *cc,
                value: *value,
            }));
        }
        MidiPayload::Channel(ChannelMessage::PitchBend { ch, value }) => {
            cells.push(PianoRollCell::PitchBend(PitchBendCell {
                time,
                channel: *ch,
                value: *value,
            }));
        }
        MidiPayload::Channel(ChannelMessage::PolyAftertouch { ch, key, pressure }) => {
            cells.push(PianoRollCell::PolyPressure(PolyPressureCell {
                time,
                channel: *ch,
                key: *key,
                pressure: *pressure,
            }));
        }
        MidiPayload::Channel(ChannelMessage::ChanAftertouch { ch, pressure }) => {
            cells.push(PianoRollCell::ChannelPressure(ChannelPressureCell {
                time,
                channel: *ch,
                pressure: *pressure,
            }));
        }
        _ => {}
    }
}

fn lifted_note(key: u8, velocity: u8, channel: u8) -> Note {
    Note::new(
        Time::from_integer(0),
        sim_lib_music_core::Pitch::from_midi(key),
        velocity.max(1),
        sim_lib_music_core::Channel::new(channel).expect("collected MIDI channel is valid"),
        Articulation::Normal,
    )
    .expect("zero-duration placeholder note is valid")
}

fn track_name(track: &sim_lib_midi_smf::SmfTrack) -> Option<String> {
    track.events.iter().find_map(|event| match &event.payload {
        MidiPayload::Meta(MetaEvent::Other(bucket)) => {
            meta_view::as_track_name(bucket).map(str::to_owned)
        }
        _ => None,
    })
}

fn tick_to_time(ticks: i64, tpq: u32) -> Time {
    Ratio::new(ticks, i64::from(tpq) * 4)
}

fn warning(message: String) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        message,
        source: None,
        span: None,
        code: None,
        related: Vec::new(),
    }
}

fn merged_events(file: &SmfFile) -> Vec<TrackedMidiEvent> {
    let mut events = file
        .tracks
        .iter()
        .enumerate()
        .flat_map(|(track, midi_track)| {
            midi_track
                .events
                .iter()
                .cloned()
                .map(move |event| TrackedMidiEvent {
                    last_track: track,
                    event,
                })
        })
        .collect::<Vec<_>>();
    events.sort_by(compare_tracked_events);
    events
}

fn compare_tracked_events(left: &TrackedMidiEvent, right: &TrackedMidiEvent) -> Ordering {
    compare_time(left.event.time, right.event.time)
        .then_with(|| event_priority(&left.event).cmp(&event_priority(&right.event)))
        .then_with(|| left.last_track.cmp(&right.last_track))
}

fn compare_time(left: sim_lib_midi_core::TickTime, right: sim_lib_midi_core::TickTime) -> Ordering {
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
