use std::collections::BTreeMap;

use crate::{
    Articulation, Chord, ConversionError, ConversionLoss, ConversionLossKind, Counterpoint, Melody,
    MelodyItem, MusicChange, MusicChangeStream, MusicConversion, Note, ObjectId, PianoRoll,
    PianoRollCell, Progression, ScoreForm, ScoreVoice, SnapshotStream, Staff, StaffNote,
    StaffVoice, Time, TimeGrid,
};

use super::target::{clean_staff, finish_staff};

pub(super) fn to_staff(source: &ScoreForm) -> Result<MusicConversion<Staff>, ConversionError> {
    match source {
        ScoreForm::Melody(value) => staff_from_melody(value),
        ScoreForm::Chord(value) => staff_from_chord(value),
        ScoreForm::Staff(value) => Ok(clean_staff(value.clone())),
        ScoreForm::Counterpoint(value) => staff_from_counterpoint(value),
        ScoreForm::PianoRoll(value) => staff_from_piano_roll(value),
        ScoreForm::Snapshot(value) => staff_from_snapshots(value),
        ScoreForm::ChangeStream(value) => staff_from_changes(value),
        ScoreForm::Progression(value) => staff_from_progression(value),
    }
}

fn staff_from_melody(melody: &Melody) -> Result<MusicConversion<Staff>, ConversionError> {
    let voice_id = ObjectId::derived("voice", "melody");
    let mut onset = Time::from_integer(0);
    let mut notes = Vec::new();
    let mut losses = Vec::new();
    for (index, item) in melody.items.iter().enumerate() {
        match item {
            MelodyItem::Note(note) => notes.push(StaffNote {
                voice_id: voice_id.clone(),
                note_id: ObjectId::derived("note", format!("melody/{index}")),
                event_id: ObjectId::derived("event", format!("melody/{index}")),
                onset,
                note: note.clone(),
            }),
            MelodyItem::Rest(_) => losses.push(ConversionLoss::new(
                ConversionLossKind::ExplicitRest,
                None,
                format!("melody item {index} becomes implicit staff silence"),
            )),
        }
        onset += item.duration();
    }
    finish_staff(
        vec![StaffVoice {
            id: voice_id,
            name: "Melody".to_owned(),
            duration: onset,
            notes,
        }],
        losses,
    )
}

fn staff_from_chord(chord: &Chord) -> Result<MusicConversion<Staff>, ConversionError> {
    let voices = chord
        .pitches
        .iter()
        .enumerate()
        .map(|(index, pitch)| {
            let voice_id = ObjectId::derived("voice", format!("chord/{index}"));
            StaffVoice {
                id: voice_id.clone(),
                name: format!("Chord tone {}", index + 1),
                duration: chord.duration,
                notes: vec![StaffNote {
                    voice_id,
                    note_id: ObjectId::derived("note", format!("chord/{index}")),
                    event_id: ObjectId::derived("event", format!("chord/{index}")),
                    onset: Time::from_integer(0),
                    note: Note {
                        duration: chord.duration,
                        pitch: *pitch,
                        velocity: chord.velocity,
                        channel: chord.channel,
                        articulation: Articulation::Normal,
                    },
                }],
            }
        })
        .collect();
    finish_staff(
        voices,
        vec![ConversionLoss::new(
            ConversionLossKind::HarmonicLabel,
            None,
            format!("chord label {:?} is not a staff event", chord.symbol),
        )],
    )
}

fn staff_from_counterpoint(
    counterpoint: &Counterpoint,
) -> Result<MusicConversion<Staff>, ConversionError> {
    let names = counterpoint.normalized_voice_names();
    let mut losses = Vec::new();
    let voices = counterpoint
        .voices
        .iter()
        .enumerate()
        .map(|(voice_index, melody)| {
            let voice_id = ObjectId::derived("voice", format!("counterpoint/{voice_index}"));
            let mut onset = Time::from_integer(0);
            let mut notes = Vec::new();
            for (item_index, item) in melody.items.iter().enumerate() {
                match item {
                    MelodyItem::Note(note) => notes.push(StaffNote {
                        voice_id: voice_id.clone(),
                        note_id: ObjectId::derived(
                            "note",
                            format!("counterpoint/{voice_index}/{item_index}"),
                        ),
                        event_id: ObjectId::derived(
                            "event",
                            format!("counterpoint/{voice_index}/{item_index}"),
                        ),
                        onset,
                        note: note.clone(),
                    }),
                    MelodyItem::Rest(_) => losses.push(ConversionLoss::new(
                        ConversionLossKind::ExplicitRest,
                        Some(voice_id.clone()),
                        format!(
                            "counterpoint voice {voice_index} item {item_index} becomes implicit silence"
                        ),
                    )),
                }
                onset += item.duration();
            }
            StaffVoice {
                id: voice_id,
                name: names[voice_index].clone(),
                duration: onset,
                notes,
            }
        })
        .collect();
    finish_staff(voices, losses)
}

fn staff_from_piano_roll(roll: &PianoRoll) -> Result<MusicConversion<Staff>, ConversionError> {
    let duration = roll
        .items
        .iter()
        .map(|item| item.onset + item.note.duration)
        .max()
        .unwrap_or_else(|| Time::from_integer(0));
    let mut voices = Vec::new();
    let mut losses = Vec::new();
    if roll.time != TimeGrid::default() {
        losses.push(ConversionLoss::new(
            ConversionLossKind::PianoRollGrid,
            None,
            format!(
                "piano-roll grid {} TPQ step {}/{} has no staff equivalent",
                roll.time.tpq,
                roll.time.step.numer(),
                roll.time.step.denom()
            ),
        ));
    }
    for (lane_index, lane) in roll.lanes.iter().enumerate() {
        let voice_id = ObjectId::derived("voice", format!("piano-roll/{}", lane.id.0));
        let mut notes = Vec::new();
        for (cell_index, cell) in lane.cells.iter().enumerate() {
            match cell {
                PianoRollCell::Note(cell) => notes.push(StaffNote {
                    voice_id: voice_id.clone(),
                    note_id: ObjectId::derived(
                        "note",
                        format!("piano-roll/{lane_index}/{cell_index}"),
                    ),
                    event_id: ObjectId::derived(
                        "event",
                        format!("piano-roll/{lane_index}/{cell_index}"),
                    ),
                    onset: cell.onset,
                    note: cell.note.clone(),
                }),
                _ => losses.push(ConversionLoss::new(
                    ConversionLossKind::NonNoteCell,
                    Some(voice_id.clone()),
                    format!(
                        "piano-roll lane {} cell {cell_index} ({}) is not a staff note",
                        lane.id.0,
                        cell.kind_label()
                    ),
                )),
            }
        }
        if !notes.is_empty() {
            voices.push(StaffVoice {
                id: voice_id,
                name: lane.id.0.clone(),
                duration,
                notes,
            });
        }
    }
    finish_staff(voices, losses)
}

fn staff_from_progression(
    progression: &Progression,
) -> Result<MusicConversion<Staff>, ConversionError> {
    let mut onset = Time::from_integer(0);
    let mut voices = Vec::new();
    let mut losses = Vec::new();
    if progression.key.is_some() {
        losses.push(ConversionLoss::new(
            ConversionLossKind::KeyAnnotation,
            None,
            "progression key is not carried by staff notes",
        ));
    }
    for (chord_index, chord) in progression.chords.iter().enumerate() {
        losses.push(ConversionLoss::new(
            ConversionLossKind::HarmonicLabel,
            None,
            format!(
                "progression chord {chord_index} label {:?} is not a staff event",
                chord.symbol
            ),
        ));
        for (pitch_index, pitch) in chord.pitches.iter().enumerate() {
            let path = format!("progression/{chord_index}/{pitch_index}");
            let voice_id = ObjectId::derived("voice", &path);
            voices.push(StaffVoice {
                id: voice_id.clone(),
                name: format!("Chord {} tone {}", chord_index + 1, pitch_index + 1),
                duration: onset + chord.duration,
                notes: vec![StaffNote {
                    voice_id,
                    note_id: ObjectId::derived("note", &path),
                    event_id: ObjectId::derived("event", &path),
                    onset,
                    note: Note {
                        duration: chord.duration,
                        pitch: *pitch,
                        velocity: chord.velocity,
                        channel: chord.channel,
                        articulation: Articulation::Normal,
                    },
                }],
            });
        }
        onset += chord.duration;
        for voice in &mut voices {
            voice.duration = onset;
        }
    }
    finish_staff(voices, losses)
}

fn staff_from_snapshots(
    stream: &SnapshotStream,
) -> Result<MusicConversion<Staff>, ConversionError> {
    let mut by_event = BTreeMap::<ObjectId, StaffNote>::new();
    let mut losses = Vec::new();
    for snapshot in &stream.snapshots {
        if snapshot.at < Time::from_integer(0) || snapshot.at > stream.duration {
            losses.push(ConversionLoss::new(
                ConversionLossKind::InconsistentChange,
                None,
                format!("snapshot at {} lies outside its stream", snapshot.at),
            ));
        }
        for note in &snapshot.sounding {
            if !(note.onset <= snapshot.at && snapshot.at < note.end()) {
                losses.push(ConversionLoss::new(
                    ConversionLossKind::InconsistentChange,
                    Some(note.event_id.clone()),
                    format!("note is not sounding at snapshot {}", snapshot.at),
                ));
            }
            match by_event.get(&note.event_id) {
                Some(existing) if existing != note => losses.push(ConversionLoss::new(
                    ConversionLossKind::InconsistentChange,
                    Some(note.event_id.clone()),
                    "snapshot payload changed while the event was sounding",
                )),
                Some(_) => {}
                None => {
                    by_event.insert(note.event_id.clone(), note.clone());
                }
            }
        }
    }
    staff_from_identified_notes(
        by_event.into_values(),
        &stream.voices,
        stream.duration,
        losses,
    )
}

fn staff_from_changes(
    stream: &MusicChangeStream,
) -> Result<MusicConversion<Staff>, ConversionError> {
    let mut starts = BTreeMap::<ObjectId, StaffNote>::new();
    let mut ends = BTreeMap::<ObjectId, Time>::new();
    let mut losses = Vec::new();
    for change in &stream.changes {
        match change {
            MusicChange::NoteStarted(note) => {
                if starts.insert(note.event_id.clone(), note.clone()).is_some() {
                    return Err(ConversionError::DuplicateIdentity(note.event_id.clone()));
                }
            }
            MusicChange::NoteEnded {
                at,
                voice_id,
                note_id,
                event_id,
            } => {
                if ends.insert(event_id.clone(), *at).is_some() {
                    return Err(ConversionError::DuplicateIdentity(event_id.clone()));
                }
                if let Some(start) = starts.get(event_id)
                    && (&start.voice_id != voice_id
                        || &start.note_id != note_id
                        || start.end() != *at)
                {
                    losses.push(ConversionLoss::new(
                        ConversionLossKind::InconsistentChange,
                        Some(event_id.clone()),
                        "note-end identity or time disagrees with note-start payload",
                    ));
                }
            }
        }
    }
    for event_id in starts.keys() {
        if !ends.contains_key(event_id) {
            losses.push(ConversionLoss::new(
                ConversionLossKind::InconsistentChange,
                Some(event_id.clone()),
                "note-start has no matching note-end",
            ));
        }
    }
    for event_id in ends.keys() {
        if !starts.contains_key(event_id) {
            losses.push(ConversionLoss::new(
                ConversionLossKind::InconsistentChange,
                Some(event_id.clone()),
                "note-end has no matching note-start",
            ));
        }
    }
    for (event_id, start) in &starts {
        if let Some(end) = ends.get(event_id)
            && start.end() != *end
        {
            losses.push(ConversionLoss::new(
                ConversionLossKind::InconsistentChange,
                Some(event_id.clone()),
                "note-end time disagrees with note-start duration",
            ));
        }
    }
    staff_from_identified_notes(
        starts.into_values(),
        &stream.voices,
        stream.duration,
        losses,
    )
}

fn staff_from_identified_notes(
    notes: impl IntoIterator<Item = StaffNote>,
    metadata: &[ScoreVoice],
    duration: Time,
    mut losses: Vec<ConversionLoss>,
) -> Result<MusicConversion<Staff>, ConversionError> {
    let mut voices = BTreeMap::<ObjectId, Vec<StaffNote>>::new();
    for note in notes {
        voices.entry(note.voice_id.clone()).or_default().push(note);
    }
    let mut result = Vec::new();
    for voice in metadata {
        result.push(StaffVoice {
            id: voice.id.clone(),
            name: voice.name.clone(),
            duration: voice.duration,
            notes: voices.remove(&voice.id).unwrap_or_default(),
        });
    }
    for (id, notes) in voices {
        losses.push(ConversionLoss::new(
            ConversionLossKind::InconsistentChange,
            Some(id.clone()),
            "event names a voice absent from stream metadata",
        ));
        result.push(StaffVoice {
            name: id.as_str().to_owned(),
            id,
            duration,
            notes,
        });
    }
    finish_staff(result, losses)
}
