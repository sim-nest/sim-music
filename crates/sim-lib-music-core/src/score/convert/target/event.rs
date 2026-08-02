use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AmbiguousConversionPolicy, Chord, ConversionError, ConversionLoss, ConversionLossKind,
    MusicChange, MusicChangeStream, MusicConversion, MusicSnapshot, ObjectId, Progression,
    ScoreFormKind, ScoreVoice, SnapshotStream, Staff, StaffNote, Time,
};

use super::{ambiguous, identity_sidecar_loss, ids_for_notes};

pub(super) fn staff_to_snapshots(staff: &Staff) -> MusicConversion<SnapshotStream> {
    let zero = Time::from_integer(0);
    let mut times = vec![zero, staff.duration()];
    let mut losses = Vec::new();
    let mut lost = BTreeSet::new();
    for note in staff.notes() {
        if note.note.duration == zero {
            lost.extend([note.note_id.clone(), note.event_id.clone()]);
            losses.push(ConversionLoss::new(
                ConversionLossKind::ZeroDurationSnapshot,
                Some(note.event_id.clone()),
                "zero-duration event is never sounding in a half-open snapshot",
            ));
        } else {
            times.extend([note.onset, note.end()]);
        }
    }
    times.sort();
    times.dedup();
    let snapshots = times
        .into_iter()
        .map(|at| MusicSnapshot {
            at,
            sounding: staff
                .notes()
                .filter(|note| note.onset <= at && at < note.end())
                .cloned()
                .collect(),
        })
        .collect();
    MusicConversion {
        value: SnapshotStream {
            duration: staff.duration(),
            voices: score_voices(staff),
            snapshots,
        },
        preserved: staff
            .object_ids()
            .into_iter()
            .filter(|id| !lost.contains(id))
            .collect(),
        losses,
    }
}

pub(super) fn staff_to_changes(staff: &Staff) -> MusicConversion<MusicChangeStream> {
    let mut changes = staff
        .notes()
        .flat_map(|note| {
            [
                MusicChange::NoteStarted(note.clone()),
                MusicChange::NoteEnded {
                    at: note.end(),
                    voice_id: note.voice_id.clone(),
                    note_id: note.note_id.clone(),
                    event_id: note.event_id.clone(),
                },
            ]
        })
        .collect::<Vec<_>>();
    changes.sort_by(|left, right| {
        left.at().cmp(&right.at()).then_with(|| {
            let left_key = change_order(left);
            let right_key = change_order(right);
            left_key.cmp(&right_key)
        })
    });
    MusicConversion {
        value: MusicChangeStream {
            duration: staff.duration(),
            voices: score_voices(staff),
            changes,
        },
        preserved: staff.object_ids(),
        losses: Vec::new(),
    }
}

pub(super) fn staff_to_progression(
    staff: &Staff,
    source: ScoreFormKind,
    policy: AmbiguousConversionPolicy,
) -> Result<MusicConversion<Progression>, ConversionError> {
    let mut groups = BTreeMap::<Time, Vec<StaffNote>>::new();
    for note in staff.notes() {
        groups.entry(note.onset).or_default().push(note.clone());
    }
    if groups.is_empty() {
        if staff.duration() > Time::from_integer(0) && policy == AmbiguousConversionPolicy::Reject {
            return Err(ambiguous(
                source,
                ScoreFormKind::Progression,
                "silent staff has no chord representation",
            ));
        }
        return Ok(MusicConversion {
            value: Progression::new(None, Vec::new())?,
            preserved: Vec::new(),
            losses: std::iter::once(identity_sidecar_loss(ScoreFormKind::Progression))
                .chain((staff.duration() > Time::from_integer(0)).then(|| {
                    ConversionLoss::new(
                        ConversionLossKind::Silence,
                        None,
                        "progression cannot represent a silent span",
                    )
                }))
                .collect(),
        });
    }

    let onsets = groups.keys().copied().collect::<Vec<_>>();
    let mut chords = Vec::new();
    let mut losses = Vec::new();
    let mut preserved = Vec::new();
    losses.push(identity_sidecar_loss(ScoreFormKind::Progression));
    if onsets[0] > Time::from_integer(0) {
        if policy == AmbiguousConversionPolicy::Reject {
            return Err(ambiguous(
                source,
                ScoreFormKind::Progression,
                "progression cannot represent leading silence",
            ));
        }
        losses.push(ConversionLoss::new(
            ConversionLossKind::Silence,
            None,
            "leading staff silence was removed",
        ));
    }
    for (index, onset) in onsets.iter().enumerate() {
        let notes = &groups[onset];
        let until = onsets
            .get(index + 1)
            .copied()
            .unwrap_or_else(|| staff.duration());
        let duration = until - *onset;
        let first = &notes[0];
        let exact = duration > Time::from_integer(0)
            && notes.iter().all(|note| {
                note.end() == until
                    && note.note.velocity == first.note.velocity
                    && note.note.channel == first.note.channel
            });
        if !exact && policy == AmbiguousConversionPolicy::Reject {
            return Err(ambiguous(
                source,
                ScoreFormKind::Progression,
                format!("notes at {onset} do not form one exact chord window"),
            ));
        }
        chords.push(Chord::new(
            duration.max(Time::from_integer(0)),
            "",
            notes.iter().map(|note| note.note.pitch).collect(),
            first.note.velocity,
            first.note.channel,
        )?);
        preserved.extend(ids_for_notes(notes));
        losses.push(ConversionLoss::new(
            ConversionLossKind::SynthesizedLabel,
            None,
            format!("progression chord {index} has an empty synthesized label"),
        ));
        if !exact {
            for note in notes {
                losses.push(ConversionLoss::new(
                    ConversionLossKind::DiscardedVoice,
                    Some(note.event_id.clone()),
                    "note duration, velocity, or channel was normalized to its chord window",
                ));
            }
        }
    }
    preserved.sort();
    preserved.dedup();
    Ok(MusicConversion {
        value: Progression::new(None, chords)?,
        preserved,
        losses,
    })
}

fn change_order(change: &MusicChange) -> (u8, &ObjectId) {
    match change {
        MusicChange::NoteEnded { event_id, .. } => (0, event_id),
        MusicChange::NoteStarted(note) => (1, &note.event_id),
    }
}

fn score_voices(staff: &Staff) -> Vec<ScoreVoice> {
    staff
        .voices
        .iter()
        .map(|voice| ScoreVoice {
            id: voice.id.clone(),
            name: voice.name.clone(),
            duration: voice.duration,
        })
        .collect()
}
