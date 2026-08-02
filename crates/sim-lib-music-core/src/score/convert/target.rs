mod event;

use std::collections::BTreeSet;

use crate::{
    AmbiguousConversionPolicy, Chord, ConversionError, ConversionLoss, ConversionLossKind,
    Counterpoint, LaneId, LaneKind, Melody, MelodyItem, MusicChange, MusicConversion, ObjectId,
    PianoRoll, PianoRollCell, PianoRollLane, Rest, ScoreForm, ScoreFormKind, Staff, StaffNote,
    StaffVoice, Time, TimeGrid, TimedNote,
};

use self::event::{staff_to_changes, staff_to_progression, staff_to_snapshots};
use super::super::staff_note_order;

pub(super) fn from_staff(
    staff: &Staff,
    source: ScoreFormKind,
    target: ScoreFormKind,
    policy: AmbiguousConversionPolicy,
) -> Result<MusicConversion<ScoreForm>, ConversionError> {
    match target {
        ScoreFormKind::Melody => {
            staff_to_melody(staff, source, policy).map(|report| report.map(ScoreForm::Melody))
        }
        ScoreFormKind::Chord => {
            staff_to_chord(staff, source, policy).map(|report| report.map(ScoreForm::Chord))
        }
        ScoreFormKind::Staff => Ok(clean_staff(staff.clone()).map(ScoreForm::Staff)),
        ScoreFormKind::Counterpoint => {
            staff_to_counterpoint(staff).map(|report| report.map(ScoreForm::Counterpoint))
        }
        ScoreFormKind::PianoRoll => {
            staff_to_piano_roll(staff).map(|report| report.map(ScoreForm::PianoRoll))
        }
        ScoreFormKind::Snapshot => Ok(staff_to_snapshots(staff).map(ScoreForm::Snapshot)),
        ScoreFormKind::ChangeStream => Ok(staff_to_changes(staff).map(ScoreForm::ChangeStream)),
        ScoreFormKind::Progression => staff_to_progression(staff, source, policy)
            .map(|report| report.map(ScoreForm::Progression)),
    }
}

fn staff_to_melody(
    staff: &Staff,
    source: ScoreFormKind,
    policy: AmbiguousConversionPolicy,
) -> Result<MusicConversion<Melody>, ConversionError> {
    let lines = monophonic_lines(staff);
    let nonempty = lines
        .iter()
        .filter(|line| !line.notes.is_empty())
        .collect::<Vec<_>>();
    if nonempty.len() > 1 && policy == AmbiguousConversionPolicy::Reject {
        return Err(ambiguous(
            source,
            ScoreFormKind::Melody,
            format!("{} simultaneous lines require selection", nonempty.len()),
        ));
    }

    let chosen = choose_line(&nonempty, policy);
    let chosen_event_ids = chosen
        .map(|voice| {
            voice
                .notes
                .iter()
                .map(|note| note.event_id.clone())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let notes = chosen.map(|voice| voice.notes.as_slice()).unwrap_or(&[]);
    let mut report = melody_from_notes(notes, staff.duration())?;
    for line in nonempty {
        if !line
            .notes
            .iter()
            .all(|note| chosen_event_ids.contains(&note.event_id))
        {
            for note in &line.notes {
                if !chosen_event_ids.contains(&note.event_id) {
                    report.losses.push(ConversionLoss::new(
                        ConversionLossKind::DiscardedVoice,
                        Some(note.event_id.clone()),
                        format!("voice {} was not selected for monophonic melody", line.id),
                    ));
                }
            }
        }
    }
    if staff.voices.len() > 1 {
        report.losses.push(ConversionLoss::new(
            ConversionLossKind::VoiceBoundary,
            None,
            "melody cannot retain distinct staff voice boundaries",
        ));
    }
    report
        .losses
        .push(identity_sidecar_loss(ScoreFormKind::Melody));
    report.preserved = ids_for_notes(notes);
    Ok(report)
}

fn staff_to_chord(
    staff: &Staff,
    source: ScoreFormKind,
    policy: AmbiguousConversionPolicy,
) -> Result<MusicConversion<Chord>, ConversionError> {
    let mut notes = staff.notes().cloned().collect::<Vec<_>>();
    notes.sort_by(staff_note_order);
    let exact = notes.iter().all(|item| {
        item.onset == Time::from_integer(0)
            && item.note.duration == staff.duration()
            && notes.first().is_none_or(|first| {
                item.note.velocity == first.note.velocity && item.note.channel == first.note.channel
            })
    });
    if !exact && policy == AmbiguousConversionPolicy::Reject {
        return Err(ambiguous(
            source,
            ScoreFormKind::Chord,
            "notes do not share onset, duration, velocity, and channel",
        ));
    }
    let first_onset = notes
        .first()
        .map(|note| note.onset)
        .unwrap_or_else(|| Time::from_integer(0));
    let selected = if exact {
        notes.clone()
    } else {
        notes
            .iter()
            .filter(|note| note.onset == first_onset)
            .cloned()
            .collect::<Vec<_>>()
    };
    let duration = selected
        .first()
        .map(|note| note.note.duration)
        .unwrap_or_else(|| staff.duration());
    let velocity = selected.first().map_or(100, |note| note.note.velocity);
    let channel = selected
        .first()
        .map(|note| note.note.channel)
        .unwrap_or_else(|| crate::Channel::new(0).expect("MIDI channel zero is valid"));
    let chord = Chord::new(
        duration,
        "",
        selected.iter().map(|note| note.note.pitch).collect(),
        velocity,
        channel,
    )?;
    let selected_ids = selected
        .iter()
        .map(|note| note.event_id.clone())
        .collect::<BTreeSet<_>>();
    let mut losses = vec![
        ConversionLoss::new(
            ConversionLossKind::SynthesizedLabel,
            None,
            "staff has no harmonic label; the chord label is empty",
        ),
        identity_sidecar_loss(ScoreFormKind::Chord),
    ];
    for note in &notes {
        if !selected_ids.contains(&note.event_id)
            || note.note.duration != duration
            || note.note.velocity != velocity
            || note.note.channel != channel
        {
            losses.push(ConversionLoss::new(
                ConversionLossKind::DiscardedVoice,
                Some(note.event_id.clone()),
                "note attributes cannot be represented by the selected chord",
            ));
        }
    }
    Ok(MusicConversion {
        value: chord,
        preserved: ids_for_notes(&selected),
        losses,
    })
}

fn staff_to_counterpoint(staff: &Staff) -> Result<MusicConversion<Counterpoint>, ConversionError> {
    let lines = monophonic_lines(staff);
    let mut melodies = Vec::new();
    let mut names = Vec::new();
    for line in &lines {
        melodies.push(melody_from_notes(&line.notes, line.duration)?.value);
        names.push(line.name.clone());
    }
    Ok(MusicConversion {
        value: Counterpoint::new(melodies, names)?,
        preserved: staff.object_ids(),
        losses: vec![identity_sidecar_loss(ScoreFormKind::Counterpoint)],
    })
}

fn staff_to_piano_roll(staff: &Staff) -> Result<MusicConversion<PianoRoll>, ConversionError> {
    let mut losses = vec![identity_sidecar_loss(ScoreFormKind::PianoRoll)];
    let lanes = staff
        .voices
        .iter()
        .map(|voice| {
            let sounding_end = voice
                .notes
                .iter()
                .map(StaffNote::end)
                .max()
                .unwrap_or_else(|| Time::from_integer(0));
            if voice.name != voice.id.as_str() || voice.duration != sounding_end {
                losses.push(ConversionLoss::new(
                    ConversionLossKind::VoiceMetadata,
                    Some(voice.id.clone()),
                    "piano-roll lane retains the voice id but not its name or silent span",
                ));
            }
            PianoRollLane::new(
                LaneId::new(voice.id.as_str()),
                LaneKind::Note,
                voice
                    .notes
                    .iter()
                    .map(|item| {
                        PianoRollCell::Note(TimedNote {
                            onset: item.onset,
                            note: item.note.clone(),
                        })
                    })
                    .collect(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MusicConversion {
        value: PianoRoll::from_lanes_with_time(lanes, TimeGrid::default())?,
        preserved: staff.object_ids(),
        losses,
    })
}

fn melody_from_notes(
    notes: &[StaffNote],
    duration: Time,
) -> Result<MusicConversion<Melody>, ConversionError> {
    let mut ordered = notes.to_vec();
    ordered.sort_by(staff_note_order);
    let mut cursor = Time::from_integer(0);
    let mut items = Vec::new();
    for note in &ordered {
        if note.onset > cursor {
            items.push(MelodyItem::Rest(Rest::new(note.onset - cursor)?));
        }
        items.push(MelodyItem::Note(note.note.clone()));
        cursor = note.end();
    }
    if duration > cursor {
        items.push(MelodyItem::Rest(Rest::new(duration - cursor)?));
    }
    Ok(MusicConversion {
        value: Melody::new(items)?,
        preserved: ids_for_notes(&ordered),
        losses: Vec::new(),
    })
}

fn monophonic_lines(staff: &Staff) -> Vec<StaffVoice> {
    let mut lines = Vec::new();
    for voice in &staff.voices {
        if voice.notes.is_empty() {
            lines.push(voice.clone());
            continue;
        }
        let mut ordered = voice.notes.clone();
        ordered.sort_by(staff_note_order);
        let mut split = Vec::<StaffVoice>::new();
        for note in ordered {
            if let Some(line) = split.iter_mut().find(|line| {
                line.notes
                    .last()
                    .is_none_or(|last| last.end() <= note.onset)
            }) {
                let mut note = note;
                note.voice_id = line.id.clone();
                line.notes.push(note);
            } else {
                let index = split.len();
                let id = if index == 0 {
                    voice.id.clone()
                } else {
                    ObjectId::derived("voice", format!("{}/line-{index}", voice.id))
                };
                let mut note = note;
                note.voice_id = id.clone();
                split.push(StaffVoice {
                    id,
                    name: if index == 0 {
                        voice.name.clone()
                    } else {
                        format!("{} {}", voice.name, index + 1)
                    },
                    duration: voice.duration,
                    notes: vec![note],
                });
            }
        }
        lines.extend(split);
    }
    lines.sort_by(|left, right| left.id.cmp(&right.id));
    lines
}

fn choose_line<'a>(
    lines: &[&'a StaffVoice],
    policy: AmbiguousConversionPolicy,
) -> Option<&'a StaffVoice> {
    match policy {
        AmbiguousConversionPolicy::Reject | AmbiguousConversionPolicy::KeepFirst => {
            lines.first().copied()
        }
        AmbiguousConversionPolicy::KeepHighest => lines.iter().copied().max_by_key(|line| {
            line.notes
                .first()
                .map_or(i32::MIN, |note| note.note.pitch.semitone())
        }),
        AmbiguousConversionPolicy::KeepLowest => lines.iter().copied().min_by_key(|line| {
            line.notes
                .first()
                .map_or(i32::MAX, |note| note.note.pitch.semitone())
        }),
    }
}

pub(super) fn finish_staff(
    voices: Vec<StaffVoice>,
    losses: Vec<ConversionLoss>,
) -> Result<MusicConversion<Staff>, ConversionError> {
    let staff = Staff::new(voices)?;
    Ok(MusicConversion {
        preserved: staff.object_ids(),
        value: staff,
        losses,
    })
}

pub(super) fn clean_staff(staff: Staff) -> MusicConversion<Staff> {
    MusicConversion {
        preserved: staff.object_ids(),
        value: staff,
        losses: Vec::new(),
    }
}

pub(super) fn embedded_ids(form: &ScoreForm) -> Vec<ObjectId> {
    match form {
        ScoreForm::Staff(staff) => staff.object_ids(),
        ScoreForm::Snapshot(stream) => {
            let mut ids = stream
                .snapshots
                .iter()
                .flat_map(|snapshot| ids_for_notes(&snapshot.sounding))
                .collect::<Vec<_>>();
            ids.sort();
            ids.dedup();
            ids
        }
        ScoreForm::ChangeStream(stream) => {
            let mut ids = stream
                .changes
                .iter()
                .flat_map(|change| match change {
                    MusicChange::NoteStarted(note) => vec![
                        note.voice_id.clone(),
                        note.note_id.clone(),
                        note.event_id.clone(),
                    ],
                    MusicChange::NoteEnded {
                        voice_id,
                        note_id,
                        event_id,
                        ..
                    } => vec![voice_id.clone(), note_id.clone(), event_id.clone()],
                })
                .collect::<Vec<_>>();
            ids.sort();
            ids.dedup();
            ids
        }
        _ => Vec::new(),
    }
}

fn ids_for_notes(notes: &[StaffNote]) -> Vec<ObjectId> {
    let mut ids = notes
        .iter()
        .flat_map(|note| {
            [
                note.voice_id.clone(),
                note.note_id.clone(),
                note.event_id.clone(),
            ]
        })
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn ambiguous(from: ScoreFormKind, to: ScoreFormKind, detail: impl Into<String>) -> ConversionError {
    ConversionError::Ambiguous {
        from,
        to,
        detail: detail.into(),
    }
}

pub(super) fn identity_sidecar_loss(target: ScoreFormKind) -> ConversionLoss {
    ConversionLoss::new(
        ConversionLossKind::IdentityMetadata,
        None,
        format!("{target:?} carries note and event identities only in the conversion report"),
    )
}
