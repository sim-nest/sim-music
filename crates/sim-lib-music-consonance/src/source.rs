use std::collections::BTreeMap;

use sim_lib_music_core::{
    AmbiguousConversionPolicy, AtomRef, ConversionLoss, Music, MusicObject, ObjectId, Score,
    ScoreForm, ScoreFormKind, Staff, StaffNote, StaffVoice, Time, convert_score,
};
use sim_lib_music_lift::{MidiTimelineId, MidiTimelineRealization};

use crate::{ConsonanceError, Provenance, ProvenanceKind, SoundingNote};

pub(crate) struct SourceMaterial {
    pub duration: Time,
    pub notes: Vec<SoundingNote>,
    pub provenance: Provenance,
}

pub(crate) fn from_score(score: &Score) -> Result<SourceMaterial, ConsonanceError> {
    if matches!(score.body, Music::MidiTrack(_) | Music::MidiFile(_)) {
        return Err(ConsonanceError::MidiRequiresRealization);
    }
    let (staff, losses, identity_policy) = match score_form(&score.body) {
        Some(form) => {
            let converted = convert_score(
                &form,
                ScoreFormKind::Staff,
                AmbiguousConversionPolicy::Reject,
            )
            .map_err(|error| ConsonanceError::ScoreConversion(error.to_string()))?;
            let ScoreForm::Staff(staff) = converted.value else {
                unreachable!("staff target always returns a staff");
            };
            (
                staff,
                converted.losses,
                "canonical score-form conversion through identity-bearing Staff".to_owned(),
            )
        }
        None => (
            flattened_staff(score)?,
            Vec::new(),
            "canonical MusicObject flatten order with channel-stable derived voices".to_owned(),
        ),
    };
    let mut facts = score_facts(score);
    facts.extend(losses.iter().map(loss_fact));
    from_staff_with_provenance(
        &staff,
        Provenance {
            kind: ProvenanceKind::Score,
            source: "music/Score".to_owned(),
            identity_policy,
            facts,
        },
    )
}

pub(crate) fn from_staff(staff: &Staff) -> Result<SourceMaterial, ConsonanceError> {
    from_staff_with_provenance(
        staff,
        Provenance {
            kind: ProvenanceKind::Staff,
            source: "music/Staff".to_owned(),
            identity_policy: "source voice/note/event ObjectId values retained verbatim".to_owned(),
            facts: vec![
                format!("voices={}", staff.voices.len()),
                format!("notes={}", staff.notes().count()),
            ],
        },
    )
}

fn from_staff_with_provenance(
    staff: &Staff,
    provenance: Provenance,
) -> Result<SourceMaterial, ConsonanceError> {
    let notes = staff
        .notes()
        .map(|note| SoundingNote {
            voice_id: note.voice_id.clone(),
            note_id: note.note_id.clone(),
            event_id: note.event_id.clone(),
            pitch: note.note.pitch,
            onset: note.onset,
            release: note.end(),
            velocity: note.note.velocity,
            channel: note.note.channel,
            articulation: note.note.articulation,
            provenance: vec![
                "source=identity-bearing-staff".to_owned(),
                format!("event={}", note.event_id),
            ],
        })
        .collect();
    Ok(SourceMaterial {
        duration: staff.duration(),
        notes,
        provenance,
    })
}

pub(crate) fn from_midi_timeline(
    timeline: &MidiTimelineRealization,
) -> Result<SourceMaterial, ConsonanceError> {
    let timeline_name = timeline_id(timeline.id);
    let notes = timeline
        .notes
        .iter()
        .map(|note| {
            let stem = format!(
                "{timeline_name}/track-{}/event-{}",
                note.id.track, note.id.event_index
            );
            let voice_id = object_id(format!(
                "midi/voice/{timeline_name}/track-{}/channel-{}",
                note.id.track, note.note.channel.0
            ))?;
            Ok(SoundingNote {
                voice_id,
                note_id: object_id(format!("midi/note/{stem}"))?,
                event_id: object_id(format!("midi/event/{stem}"))?,
                pitch: note.note.pitch,
                onset: note.onset,
                release: note.sounding_until,
                velocity: note.note.velocity,
                channel: note.note.channel,
                articulation: note.note.articulation,
                provenance: vec![
                    format!("midi-note-id={:?}", note.id),
                    format!("key-release={:?}", note.key_release),
                    format!("ended-by={:?}", note.ended_by),
                ],
            })
        })
        .collect::<Result<Vec<_>, ConsonanceError>>()?;
    let duration = notes
        .iter()
        .map(|note| note.release)
        .max()
        .unwrap_or_else(|| Time::from_integer(0));
    Ok(SourceMaterial {
        duration,
        notes,
        provenance: Provenance {
            kind: ProvenanceKind::MidiTimeline,
            source: format!("music/MidiTimelineRealization/{timeline_name}"),
            identity_policy:
                "source track/event note-on identity plus track/channel voice identity".to_owned(),
            facts: vec![
                format!("source-tracks={:?}", timeline.source_tracks),
                format!("notes={}", timeline.notes.len()),
                format!("diagnostics={}", timeline.diagnostics.len()),
                "pedal-and-overlap-realization=complete".to_owned(),
            ],
        },
    })
}

fn score_form(music: &Music) -> Option<ScoreForm> {
    match music {
        Music::Chord(value) => Some(ScoreForm::Chord(value.clone())),
        Music::Melody(value) => Some(ScoreForm::Melody(value.clone())),
        Music::Progression(value) => Some(ScoreForm::Progression(value.clone())),
        Music::Counterpoint(value) => Some(ScoreForm::Counterpoint(value.clone())),
        Music::PianoRoll(value) => Some(ScoreForm::PianoRoll(value.clone())),
        Music::Note(_)
        | Music::Rest(_)
        | Music::Par(_)
        | Music::Seq(_)
        | Music::Arranger(_)
        | Music::MidiTrack(_)
        | Music::MidiFile(_) => None,
    }
}

fn flattened_staff(score: &Score) -> Result<Staff, ConsonanceError> {
    let duration = score.body.duration();
    let mut atoms = Vec::new();
    score.body.voices(Time::from_integer(0), &mut atoms);
    let mut voices = BTreeMap::<u8, StaffVoice>::new();
    for (index, atom) in atoms.into_iter().enumerate() {
        let AtomRef::Note(note) = atom.atom else {
            continue;
        };
        let channel = note.channel.0;
        let voice_id = object_id(format!("score/voice/channel-{channel}"))?;
        let entry = voices.entry(channel).or_insert_with(|| StaffVoice {
            id: voice_id.clone(),
            name: format!("Derived channel {channel}"),
            duration,
            notes: Vec::new(),
        });
        entry.notes.push(StaffNote {
            voice_id,
            note_id: object_id(format!("score/note/{index}"))?,
            event_id: object_id(format!("score/event/{index}"))?,
            onset: atom.onset,
            note,
        });
    }
    if voices.is_empty() {
        let voice_id = object_id("score/voice/silence")?;
        voices.insert(
            0,
            StaffVoice {
                id: voice_id,
                name: "Silence".to_owned(),
                duration,
                notes: Vec::new(),
            },
        );
    }
    Staff::new(voices.into_values().collect())
        .map_err(|error| ConsonanceError::ScoreConversion(error.to_string()))
}

fn object_id(value: impl Into<String>) -> Result<ObjectId, ConsonanceError> {
    ObjectId::new(value).map_err(|error| ConsonanceError::Identity(error.to_string()))
}

fn score_facts(score: &Score) -> Vec<String> {
    vec![
        format!("tempo-bpm={}", score.tempo_bpm),
        format!(
            "time-signature={}/{}",
            score.time_signature.0, score.time_signature.1
        ),
        format!("key={}", score.key.as_deref().unwrap_or("none")),
        format!("body-kind={}", score.body.kind()),
    ]
}

fn loss_fact(loss: &ConversionLoss) -> String {
    format!(
        "conversion-loss={:?};object={};detail={}",
        loss.kind,
        loss.object
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "none".to_owned()),
        loss.detail
    )
}

fn timeline_id(id: MidiTimelineId) -> String {
    match id {
        MidiTimelineId::Shared => "shared".to_owned(),
        MidiTimelineId::Pattern(index) => format!("pattern-{index}"),
    }
}
