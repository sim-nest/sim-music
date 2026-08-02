use sim_kernel::Diagnostic;
use sim_lib_midi_core::{MetaEvent, MidiPayload, meta_view};
use sim_lib_midi_smf::{SmfFile, SmfFormat};
use sim_lib_music_core::{Note, PianoRoll, Time};

use crate::realize::realize_midi_impl;
use crate::{LiftError, MidiRealizationPolicy};

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
    piano_roll: PianoRoll,
    pub diagnostics: Vec<Diagnostic>,
}

pub(crate) fn collect_midi(file: &SmfFile) -> Result<CollectedMidi, LiftError> {
    if file.format == SmfFormat::Independent {
        return Err(LiftError::IndependentPatternsRequireSelection);
    }
    let mut realization = realize_midi_impl(file, MidiRealizationPolicy::default())?;
    let timeline = realization
        .timelines
        .pop()
        .expect("formats 0 and 1 always realize one shared timeline");
    let notes = timeline
        .notes
        .iter()
        .map(|note| CollectedNote {
            onset: note.onset,
            duration: note.note.duration,
            note: note.note.clone(),
            track: note.id.track,
            track_name: file.tracks.get(note.id.track).and_then(track_name),
            order: note.id.event_index,
        })
        .collect();
    Ok(CollectedMidi {
        notes,
        piano_roll: timeline.piano_roll,
        diagnostics: timeline.diagnostics,
    })
}

impl CollectedMidi {
    pub(crate) fn to_piano_roll(&self) -> PianoRoll {
        self.piano_roll.clone()
    }
}

fn track_name(track: &sim_lib_midi_smf::SmfTrack) -> Option<String> {
    track.events.iter().find_map(|event| match &event.payload {
        MidiPayload::Meta(MetaEvent::Other(bucket)) => {
            meta_view::as_track_name(bucket).map(str::to_owned)
        }
        _ => None,
    })
}
