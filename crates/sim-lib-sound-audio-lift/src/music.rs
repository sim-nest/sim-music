use num_rational::Ratio;
use sim_lib_music_analysis::DiffRoll;
use sim_lib_music_core::{
    Articulation, Channel, Counterpoint, Melody, MelodyItem, MusicError, Note, PianoRoll, Rest,
    TimedNote,
};

use crate::{AudioLiftResult, AudioNoteCandidate};

/// Converts lifted note candidates into a [`PianoRoll`], mapping amplitudes to
/// velocities and tracks to channels.
pub fn lifted_notes_to_piano_roll(notes: &[AudioNoteCandidate]) -> Result<PianoRoll, MusicError> {
    let items = notes
        .iter()
        .map(|note| {
            Ok(TimedNote {
                onset: sample_to_time(note.onset_sample, note.sample_rate),
                note: Note::new(
                    sample_to_time(note.duration_samples, note.sample_rate),
                    note.pitch,
                    velocity_from_amplitude(note.mean_amplitude.0),
                    Channel::new((note.track % 16) as u8).expect("track modulo 16 is a channel"),
                    Articulation::Legato,
                )?,
            })
        })
        .collect::<Result<Vec<_>, MusicError>>()?;
    PianoRoll::new(items)
}

/// Converts lifted note candidates into a [`DiffRoll`] via their piano roll.
pub fn lifted_notes_to_diff_roll(notes: &[AudioNoteCandidate]) -> Result<DiffRoll, MusicError> {
    Ok(DiffRoll::from_piano_roll(&lifted_notes_to_piano_roll(
        notes,
    )?))
}

/// Converts lifted note candidates into a [`Counterpoint`], grouping notes by
/// track into voices with interspersed rests.
pub fn lifted_notes_to_counterpoint(
    notes: &[AudioNoteCandidate],
) -> Result<Counterpoint, MusicError> {
    let mut by_track = std::collections::BTreeMap::<usize, Vec<&AudioNoteCandidate>>::new();
    for note in notes {
        by_track.entry(note.track).or_default().push(note);
    }

    let mut voices = Vec::new();
    let mut names = Vec::new();
    for (track, mut track_notes) in by_track {
        track_notes.sort_by_key(|left| left.onset_sample);
        let sample_rate = track_notes
            .first()
            .map(|note| note.sample_rate)
            .unwrap_or(1);
        let mut items = Vec::new();
        let mut cursor = Ratio::from_integer(0);
        for note in track_notes {
            let onset = sample_to_time(note.onset_sample, note.sample_rate);
            if onset > cursor {
                items.push(MelodyItem::Rest(Rest::new(onset - cursor)?));
            }
            let duration = sample_to_time(note.duration_samples, note.sample_rate.max(sample_rate));
            items.push(MelodyItem::Note(Note::new(
                duration,
                note.pitch,
                velocity_from_amplitude(note.mean_amplitude.0),
                Channel::new((track % 16) as u8).expect("track modulo 16 is a channel"),
                Articulation::Legato,
            )?));
            cursor = onset + duration;
        }
        voices.push(Melody::new(items)?);
        names.push(format!("audio-voice-{}", track + 1));
    }
    Counterpoint::new(voices, names)
}

impl AudioLiftResult {
    /// Converts this result's notes into a [`PianoRoll`].
    pub fn to_piano_roll(&self) -> Result<PianoRoll, MusicError> {
        lifted_notes_to_piano_roll(&self.notes)
    }

    /// Converts this result's notes into a [`DiffRoll`].
    pub fn to_diff_roll(&self) -> Result<DiffRoll, MusicError> {
        lifted_notes_to_diff_roll(&self.notes)
    }

    /// Converts this result's notes into a [`Counterpoint`].
    pub fn to_counterpoint(&self) -> Result<Counterpoint, MusicError> {
        lifted_notes_to_counterpoint(&self.notes)
    }
}

fn sample_to_time(samples: usize, sample_rate: u32) -> Ratio<i64> {
    Ratio::new(samples as i64, i64::from(sample_rate.max(1)) * 4)
}

fn velocity_from_amplitude(amplitude: f64) -> u8 {
    (amplitude.clamp(0.0, 1.0) * 127.0)
        .round()
        .clamp(1.0, 127.0) as u8
}
