use sim_lib_midi_core::{ChannelMessage, MidiEvent, MidiPayload, synthetic_origin};
use sim_lib_midi_smf::{SmfFile, SmfFormat, SmfTrack};
use sim_lib_music_core::{
    Articulation, Note, PianoRoll, PianoRollCell, PianoRollLane, Pitch, Score, Time,
};

use crate::model::{
    checked_tick_time, score_meta_events, tempo_meta_events, time_to_ticks, validate_note,
    with_track_name,
};
use crate::{LowerError, LowerOpts};

pub(crate) fn build_piano_roll_file(
    roll: &PianoRoll,
    score: Option<&Score>,
    opts: &LowerOpts,
) -> Result<SmfFile, LowerError> {
    let mut events = tempo_meta_events(opts)?;
    if let Some(score) = score {
        events.extend(score_meta_events(score, opts.tpq)?);
    }
    events.extend(piano_roll_midi_events(roll, opts.tpq)?);
    let name = score.map_or("PianoRoll", |_| "Score");
    let mut file = SmfFile {
        format: SmfFormat::SingleTrack,
        tpq: opts.tpq,
        tracks: vec![SmfTrack {
            events: with_track_name(name, events, opts.tpq)?,
        }],
    };
    file.canonicalize();
    Ok(file)
}

fn piano_roll_midi_events(roll: &PianoRoll, tpq: u32) -> Result<Vec<MidiEvent>, LowerError> {
    let mut events = Vec::new();
    for lane in &roll.lanes {
        for cell in &lane.cells {
            events.extend(cell_midi_events(lane, cell, tpq)?);
        }
    }
    Ok(events)
}

fn cell_midi_events(
    lane: &PianoRollLane,
    cell: &PianoRollCell,
    tpq: u32,
) -> Result<Vec<MidiEvent>, LowerError> {
    match cell {
        PianoRollCell::Note(cell) => note_events(cell.onset, &cell.note, tpq),
        PianoRollCell::Drum(cell) => {
            let note = Note {
                duration: cell.duration,
                pitch: Pitch::from_midi(cell.key.0),
                velocity: cell.velocity.0,
                channel: cell.channel,
                articulation: Articulation::Normal,
            };
            note_events(cell.onset, &note, tpq)
        }
        PianoRollCell::ControlChange(cell) => Ok(vec![MidiEvent {
            time: checked_tick_time(time_to_ticks(cell.time, tpq)?, tpq)?,
            origin: synthetic_origin(),
            payload: MidiPayload::Channel(ChannelMessage::ControlChange {
                ch: cell.channel,
                cc: cell.controller,
                value: cell.value,
            }),
        }]),
        PianoRollCell::PitchBend(cell) => Ok(vec![MidiEvent {
            time: checked_tick_time(time_to_ticks(cell.time, tpq)?, tpq)?,
            origin: synthetic_origin(),
            payload: MidiPayload::Channel(ChannelMessage::PitchBend {
                ch: cell.channel,
                value: cell.value,
            }),
        }]),
        PianoRollCell::PolyPressure(cell) => Ok(vec![MidiEvent {
            time: checked_tick_time(time_to_ticks(cell.time, tpq)?, tpq)?,
            origin: synthetic_origin(),
            payload: MidiPayload::Channel(ChannelMessage::PolyAftertouch {
                ch: cell.channel,
                key: cell.key,
                pressure: cell.pressure,
            }),
        }]),
        PianoRollCell::ChannelPressure(cell) => Ok(vec![MidiEvent {
            time: checked_tick_time(time_to_ticks(cell.time, tpq)?, tpq)?,
            origin: synthetic_origin(),
            payload: MidiPayload::Channel(ChannelMessage::ChanAftertouch {
                ch: cell.channel,
                pressure: cell.pressure,
            }),
        }]),
        PianoRollCell::Midi(event) => Ok(vec![event.clone()]),
        PianoRollCell::ScaleDegree(_) | PianoRollCell::Object(_) | PianoRollCell::Automation(_) => {
            Err(LowerError::UnsupportedPianoRollCell {
                lane: lane.id.0.clone(),
                cell_kind: cell.kind_label().to_owned(),
            })
        }
    }
}

fn note_events(onset: Time, note: &Note, tpq: u32) -> Result<Vec<MidiEvent>, LowerError> {
    let (midi_key, channel, velocity) = validate_note(note)?;
    let start = time_to_ticks(onset, tpq)?;
    let end = time_to_ticks(onset + note.duration, tpq)?;
    Ok(vec![
        MidiEvent {
            time: checked_tick_time(start, tpq)?,
            origin: synthetic_origin(),
            payload: MidiPayload::Channel(ChannelMessage::NoteOn {
                ch: channel,
                key: sim_lib_midi_core::U7(midi_key),
                vel: velocity,
            }),
        },
        MidiEvent {
            time: checked_tick_time(end, tpq)?,
            origin: synthetic_origin(),
            payload: MidiPayload::Channel(ChannelMessage::NoteOff {
                ch: channel,
                key: sim_lib_midi_core::U7(midi_key),
                vel: sim_lib_midi_core::U7(0),
            }),
        },
    ])
}
