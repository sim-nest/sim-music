use std::{cmp::Ordering, collections::BTreeMap};

use num_rational::Ratio;
use sim_kernel::{Diagnostic, Severity, Symbol};
use sim_lib_midi_core::{
    CC_ALL_NOTES_OFF, CC_ALL_SOUND_OFF, CC_RESET_ALL_CONTROLLERS, CC_SOSTENUTO, CC_SUSTAIN_PEDAL,
    ChannelMessage, MidiEvent, MidiPayload, MidiTempoMap, TickTime,
};
use sim_lib_midi_smf::{SmfFile, SmfFormat, SmfTempoMaps};
use sim_lib_music_core::{
    Articulation, ChannelPressureCell, ControlChangeCell, LaneId, LaneKind, Note, PianoRoll,
    PianoRollCell, PianoRollLane, PitchBendCell, PolyPressureCell, Time, TimeGrid, TimedNote,
};

use crate::{
    DanglingNotePolicy, LiftError, MidiNoteEnd, MidiNoteId, MidiRealization, MidiRealizationPolicy,
    MidiTimelineId, MidiTimelineRealization, OverlapPolicy, PedalPolicy, RealizedMidiNote,
    SameTickPolicy,
};

#[derive(Copy, Clone)]
struct SourceEvent<'a> {
    track: usize,
    event_index: usize,
    event: &'a MidiEvent,
}

#[derive(Clone, Debug)]
struct OpenNote {
    id: MidiNoteId,
    onset: Time,
    velocity: u8,
    channel: sim_lib_midi_core::Channel,
    key: u8,
    key_down: bool,
    key_release: Option<Time>,
    release_velocity: u8,
    sostenuto_captured: bool,
}

#[derive(Copy, Clone, Debug, Default)]
struct ChannelPedals {
    sustain: bool,
    sostenuto: bool,
}

struct TimelineState {
    policy: MidiRealizationPolicy,
    active: BTreeMap<(u8, u8), Vec<OpenNote>>,
    pedals: [ChannelPedals; 16],
    notes: Vec<RealizedMidiNote>,
    controls: Vec<(usize, PianoRollCell)>,
    diagnostics: Vec<Diagnostic>,
}

impl TimelineState {
    fn new(policy: MidiRealizationPolicy) -> Self {
        Self {
            policy,
            active: BTreeMap::new(),
            pedals: [ChannelPedals::default(); 16],
            notes: Vec::new(),
            controls: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn observe(
        &mut self,
        source: SourceEvent<'_>,
        timeline: MidiTimelineId,
    ) -> Result<(), LiftError> {
        let time = tick_to_time(source.event.time);
        match &source.event.payload {
            MidiPayload::Channel(ChannelMessage::NoteOn { ch, key, vel }) if vel.0 > 0 => {
                self.note_on(source, timeline, time, *ch, key.0, vel.0)?;
            }
            MidiPayload::Channel(ChannelMessage::NoteOff { ch, key, vel }) => {
                self.note_off(time, *ch, key.0, vel.0, MidiNoteEnd::NoteOff);
            }
            MidiPayload::Channel(ChannelMessage::NoteOn {
                ch,
                key,
                vel: sim_lib_midi_core::U7(0),
            }) => {
                self.note_off(time, *ch, key.0, 0, MidiNoteEnd::NoteOff);
            }
            MidiPayload::Channel(ChannelMessage::ControlChange { ch, cc, value }) => {
                self.controls
                    .push((source.track, control_cell(time, *ch, *cc, *value)));
                self.control_change(time, *ch, cc.0, value.0);
            }
            MidiPayload::Channel(ChannelMessage::PitchBend { ch, value }) => {
                self.controls.push((
                    source.track,
                    PianoRollCell::PitchBend(PitchBendCell {
                        time,
                        channel: *ch,
                        value: *value,
                    }),
                ));
            }
            MidiPayload::Channel(ChannelMessage::PolyAftertouch { ch, key, pressure }) => {
                self.controls.push((
                    source.track,
                    PianoRollCell::PolyPressure(PolyPressureCell {
                        time,
                        channel: *ch,
                        key: *key,
                        pressure: *pressure,
                    }),
                ));
            }
            MidiPayload::Channel(ChannelMessage::ChanAftertouch { ch, pressure }) => {
                self.controls.push((
                    source.track,
                    PianoRollCell::ChannelPressure(ChannelPressureCell {
                        time,
                        channel: *ch,
                        pressure: *pressure,
                    }),
                ));
            }
            MidiPayload::Channel(_)
            | MidiPayload::Meta(_)
            | MidiPayload::SysEx(_)
            | MidiPayload::Raw(_) => {}
        }
        Ok(())
    }

    fn note_on(
        &mut self,
        source: SourceEvent<'_>,
        timeline: MidiTimelineId,
        time: Time,
        channel: sim_lib_midi_core::Channel,
        key: u8,
        velocity: u8,
    ) -> Result<(), LiftError> {
        let active = self.active.entry((channel.0, key)).or_default();
        if self.policy.overlap == OverlapPolicy::Reject && active.iter().any(|note| note.key_down) {
            return Err(LiftError::OverlappingNote {
                tick: source.event.time.ticks,
                channel: channel.0,
                key,
            });
        }
        active.push(OpenNote {
            id: MidiNoteId {
                timeline,
                track: source.track,
                event_index: source.event_index,
            },
            onset: time,
            velocity,
            channel,
            key,
            key_down: true,
            key_release: None,
            release_velocity: 0,
            sostenuto_captured: false,
        });
        Ok(())
    }

    fn note_off(
        &mut self,
        time: Time,
        channel: sim_lib_midi_core::Channel,
        key: u8,
        velocity: u8,
        ended_by: MidiNoteEnd,
    ) {
        let pair = (channel.0, key);
        let position = self
            .active
            .get(&pair)
            .and_then(|notes| match self.policy.overlap {
                OverlapPolicy::Fifo | OverlapPolicy::Reject => {
                    notes.iter().position(|note| note.key_down)
                }
                OverlapPolicy::Lifo => notes.iter().rposition(|note| note.key_down),
            });
        let Some(position) = position else {
            self.unmatched_note_off(time, channel.0, key);
            return;
        };
        let notes = self
            .active
            .get_mut(&pair)
            .expect("a located MIDI note remains in the active table");
        let note = &mut notes[position];
        note.key_down = false;
        note.key_release = Some(time);
        note.release_velocity = velocity;
        if !is_held(note, self.pedals[channel.0 as usize], self.policy.pedals) {
            let note = notes.remove(position);
            self.finish_note(note, time, ended_by);
        }
    }

    fn unmatched_note_off(&mut self, time: Time, channel: u8, key: u8) {
        self.diagnostics.push(warning(
            "unmatched-note-off",
            format!(
                "unmatched note-off at {}/{} for channel {channel} key {key}",
                time.numer(),
                time.denom()
            ),
        ));
    }

    fn control_change(
        &mut self,
        time: Time,
        channel: sim_lib_midi_core::Channel,
        controller: u8,
        value: u8,
    ) {
        let channel_index = channel.0 as usize;
        match controller {
            cc if cc == CC_SUSTAIN_PEDAL.0 && self.policy.pedals != PedalPolicy::Ignore => {
                let down = value >= 64;
                let was_down = self.pedals[channel_index].sustain;
                self.pedals[channel_index].sustain = down;
                if was_down && !down {
                    self.release_deferred(channel.0, time, MidiNoteEnd::SustainRelease);
                }
            }
            cc if cc == CC_SOSTENUTO.0
                && self.policy.pedals == PedalPolicy::SustainAndSostenuto =>
            {
                let down = value >= 64;
                let was_down = self.pedals[channel_index].sostenuto;
                if !was_down && down {
                    for notes in self.active.values_mut() {
                        for note in notes.iter_mut().filter(|note| note.channel == channel) {
                            note.sostenuto_captured = true;
                        }
                    }
                }
                self.pedals[channel_index].sostenuto = down;
                if was_down && !down {
                    self.release_deferred(channel.0, time, MidiNoteEnd::SostenutoRelease);
                    for notes in self.active.values_mut() {
                        for note in notes.iter_mut().filter(|note| note.channel == channel) {
                            note.sostenuto_captured = false;
                        }
                    }
                }
            }
            cc if cc == CC_ALL_NOTES_OFF.0 => {
                for notes in self.active.values_mut() {
                    for note in notes
                        .iter_mut()
                        .filter(|note| note.channel == channel && note.key_down)
                    {
                        note.key_down = false;
                        note.key_release = Some(time);
                        note.release_velocity = 0;
                    }
                }
                self.release_deferred(channel.0, time, MidiNoteEnd::AllNotesOff);
            }
            cc if cc == CC_ALL_SOUND_OFF.0 => {
                self.close_channel(channel.0, time, MidiNoteEnd::AllSoundOff);
            }
            cc if cc == CC_RESET_ALL_CONTROLLERS.0 => {
                self.pedals[channel_index] = ChannelPedals::default();
                for notes in self.active.values_mut() {
                    for note in notes.iter_mut().filter(|note| note.channel == channel) {
                        note.sostenuto_captured = false;
                    }
                }
                self.release_deferred(channel.0, time, MidiNoteEnd::ResetControllers);
            }
            _ => {}
        }
    }

    fn release_deferred(&mut self, channel: u8, time: Time, ended_by: MidiNoteEnd) {
        let active = std::mem::take(&mut self.active);
        for (key, notes) in active {
            for note in notes {
                if note.channel.0 == channel
                    && !note.key_down
                    && !is_held(&note, self.pedals[channel as usize], self.policy.pedals)
                {
                    self.finish_note(note, time, ended_by);
                } else {
                    self.active.entry(key).or_default().push(note);
                }
            }
        }
    }

    fn close_channel(&mut self, channel: u8, time: Time, ended_by: MidiNoteEnd) {
        let active = std::mem::take(&mut self.active);
        for (key, notes) in active {
            for note in notes {
                if note.channel.0 == channel {
                    self.finish_note(note, time, ended_by);
                } else {
                    self.active.entry(key).or_default().push(note);
                }
            }
        }
    }

    fn finish_note(&mut self, note: OpenNote, until: Time, ended_by: MidiNoteEnd) {
        let duration = until - note.onset;
        self.notes.push(RealizedMidiNote {
            id: note.id,
            onset: note.onset,
            key_release: note.key_release,
            sounding_until: until,
            release_velocity: note.release_velocity,
            note: Note::new(
                duration,
                sim_lib_music_core::Pitch::from_midi(note.key),
                note.velocity,
                note.channel,
                Articulation::Normal,
            )
            .expect("ordered MIDI events produce non-negative note durations"),
            ended_by,
        });
    }

    fn finish(mut self, end: Time) -> Result<Self, LiftError> {
        let dangling_count = self.active.values().map(Vec::len).sum();
        if dangling_count > 0 && self.policy.dangling_notes == DanglingNotePolicy::Reject {
            return Err(LiftError::DanglingNotes {
                count: dangling_count,
            });
        }
        let active = std::mem::take(&mut self.active);
        for notes in active.into_values() {
            for note in notes {
                self.diagnostics.push(warning(
                    "dangling-note-on",
                    format!(
                        "note-on {} (track {}, event {}) closed at end-of-track/timeline",
                        note.key, note.id.track, note.id.event_index
                    ),
                ));
                self.finish_note(note, end, MidiNoteEnd::EndOfTimeline);
            }
        }
        self.notes.sort_by(|left, right| {
            left.onset
                .cmp(&right.onset)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(self)
    }
}

pub(crate) fn realize_midi_impl(
    file: &SmfFile,
    policy: MidiRealizationPolicy,
) -> Result<MidiRealization, LiftError> {
    if file.ticks_per_quarter().is_none() {
        return Err(LiftError::MetricalTimingRequired);
    }
    let tempo_maps = file.tempo_maps()?;
    let timelines = match (file.format, tempo_maps) {
        (SmfFormat::Independent, SmfTempoMaps::Independent(maps)) => file
            .tracks
            .iter()
            .enumerate()
            .zip(maps)
            .map(|((track, contents), tempo_map)| {
                let events = contents
                    .events
                    .iter()
                    .enumerate()
                    .map(|(event_index, event)| SourceEvent {
                        track,
                        event_index,
                        event,
                    })
                    .collect();
                realize_timeline(
                    MidiTimelineId::Pattern(track),
                    vec![track],
                    events,
                    tempo_map,
                    policy,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        (_, SmfTempoMaps::Shared(tempo_map)) => {
            let events = file
                .tracks
                .iter()
                .enumerate()
                .flat_map(|(track, contents)| {
                    contents
                        .events
                        .iter()
                        .enumerate()
                        .map(move |(event_index, event)| SourceEvent {
                            track,
                            event_index,
                            event,
                        })
                })
                .collect::<Vec<_>>();
            vec![realize_timeline(
                MidiTimelineId::Shared,
                (0..file.tracks.len()).collect(),
                events,
                tempo_map,
                policy,
            )?]
        }
        _ => unreachable!("SMF format and tempo-map topology agree by construction"),
    };
    Ok(MidiRealization { timelines })
}

fn realize_timeline(
    id: MidiTimelineId,
    source_tracks: Vec<usize>,
    mut events: Vec<SourceEvent<'_>>,
    tempo_map: MidiTempoMap,
    policy: MidiRealizationPolicy,
) -> Result<MidiTimelineRealization, LiftError> {
    events.sort_by(|left, right| compare_events(*left, *right, policy.same_tick));
    let end = events
        .iter()
        .map(|source| tick_to_time(source.event.time))
        .max()
        .unwrap_or_else(|| Time::from_integer(0));
    let mut state = TimelineState::new(policy);
    for source in &events {
        state.observe(*source, id)?;
    }
    let state = state.finish(end)?;
    let piano_roll = build_piano_roll(tempo_map.tpq(), &state.notes, &state.controls)?;
    Ok(MidiTimelineRealization {
        id,
        source_tracks,
        tempo_map,
        notes: state.notes,
        piano_roll,
        diagnostics: state.diagnostics,
    })
}

fn build_piano_roll(
    tpq: u32,
    notes: &[RealizedMidiNote],
    controls: &[(usize, PianoRollCell)],
) -> Result<PianoRoll, LiftError> {
    let mut lanes = Vec::new();
    let mut notes_by_track = BTreeMap::<usize, Vec<PianoRollCell>>::new();
    for note in notes {
        notes_by_track
            .entry(note.id.track)
            .or_default()
            .push(PianoRollCell::Note(TimedNote {
                onset: note.onset,
                note: note.note.clone(),
            }));
    }
    for (track, cells) in notes_by_track {
        lanes.push(PianoRollLane::new(
            LaneId::new(format!("midi-track-{track}-notes")),
            LaneKind::Note,
            cells,
        )?);
    }
    let mut controls_by_track = BTreeMap::<usize, Vec<PianoRollCell>>::new();
    for (track, cell) in controls {
        controls_by_track
            .entry(*track)
            .or_default()
            .push(cell.clone());
    }
    for (track, cells) in controls_by_track {
        lanes.push(PianoRollLane::new(
            LaneId::new(format!("midi-track-{track}-controls")),
            LaneKind::Control,
            cells,
        )?);
    }
    let denominator = i64::from(tpq) * 4;
    let grid = TimeGrid::new(tpq, Ratio::new(1, denominator))?;
    Ok(PianoRoll::from_lanes_with_time(lanes, grid)?)
}

fn control_cell(
    time: Time,
    channel: sim_lib_midi_core::Channel,
    controller: sim_lib_midi_core::U7,
    value: sim_lib_midi_core::U7,
) -> PianoRollCell {
    PianoRollCell::ControlChange(ControlChangeCell {
        time,
        channel,
        controller,
        value,
    })
}

fn is_held(note: &OpenNote, pedals: ChannelPedals, policy: PedalPolicy) -> bool {
    match policy {
        PedalPolicy::Ignore => false,
        PedalPolicy::Sustain => pedals.sustain,
        PedalPolicy::SustainAndSostenuto => {
            pedals.sustain || (pedals.sostenuto && note.sostenuto_captured)
        }
    }
}

fn compare_events(
    left: SourceEvent<'_>,
    right: SourceEvent<'_>,
    policy: SameTickPolicy,
) -> Ordering {
    compare_time(left.event.time, right.event.time)
        .then_with(|| {
            same_tick_priority(left.event, policy).cmp(&same_tick_priority(right.event, policy))
        })
        .then_with(|| left.track.cmp(&right.track))
        .then_with(|| left.event_index.cmp(&right.event_index))
}

fn compare_time(left: TickTime, right: TickTime) -> Ordering {
    let left_scaled = i128::from(left.ticks) * i128::from(right.tpq);
    let right_scaled = i128::from(right.ticks) * i128::from(left.tpq);
    left_scaled.cmp(&right_scaled)
}

fn same_tick_priority(event: &MidiEvent, policy: SameTickPolicy) -> u8 {
    match policy {
        SameTickPolicy::Encoded => 0,
        SameTickPolicy::NoteOffsFirst => match &event.payload {
            MidiPayload::Channel(ChannelMessage::NoteOff { .. })
            | MidiPayload::Channel(ChannelMessage::NoteOn {
                vel: sim_lib_midi_core::U7(0),
                ..
            }) => 0,
            MidiPayload::Channel(ChannelMessage::NoteOn { .. }) => 2,
            MidiPayload::Meta(sim_lib_midi_core::MetaEvent::EndOfTrack) => 3,
            _ => 1,
        },
        SameTickPolicy::NoteOnsFirst => match &event.payload {
            MidiPayload::Channel(ChannelMessage::NoteOn { vel, .. }) if vel.0 > 0 => 0,
            MidiPayload::Channel(ChannelMessage::NoteOff { .. })
            | MidiPayload::Channel(ChannelMessage::NoteOn {
                vel: sim_lib_midi_core::U7(0),
                ..
            }) => 2,
            MidiPayload::Meta(sim_lib_midi_core::MetaEvent::EndOfTrack) => 3,
            _ => 1,
        },
    }
}

fn tick_to_time(time: TickTime) -> Time {
    Ratio::new(time.ticks, i64::from(time.tpq) * 4)
}

fn warning(code: &'static str, message: String) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        message,
        source: None,
        span: None,
        code: Some(Symbol::qualified("music/midi-realization", code)),
        related: Vec::new(),
    }
}
