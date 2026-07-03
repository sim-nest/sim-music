use num_rational::Ratio;
use sim_lib_music_analysis::{ChordWindowMode, chord_windows_from_piano_roll};
use sim_lib_music_core::{Chord, PianoRoll, Progression, Time, TimedNote};
use sim_lib_pitch_core::Pitch;
use sim_lib_pitch_namer::{LabelContext, NamerRegistry, NamingSchool};

use crate::collect::collect_midi;
use crate::{LabelStrategy, LiftError, LiftReport, ProgressionLiftOpts};

pub(crate) fn lift_progression_impl(
    file: &sim_lib_midi_smf::SmfFile,
    opts: &ProgressionLiftOpts,
) -> Result<LiftReport<Progression>, LiftError> {
    validate_progression_opts(opts)?;
    let collected = collect_midi(file);
    let roll = quantize_roll(&collected.to_piano_roll()?, opts.grid)?;
    let windows = chord_windows_from_piano_roll(&roll, opts.window_mode);
    let registry = NamerRegistry::new_with_builtins();
    let school = match opts.label_strategy {
        LabelStrategy::Functional => NamingSchool::FunctionalRoman,
        LabelStrategy::JazzChord => NamingSchool::Jazz,
        LabelStrategy::SetClass => NamingSchool::Forte,
    };

    let mut chords = Vec::new();
    for (index, window) in windows.iter().enumerate() {
        if window.pitch_class_mask.count_bits() < opts.min_notes as u32 {
            continue;
        }
        let next_onset = windows
            .iter()
            .skip(index + 1)
            .find(|next| next.pitch_class_mask.count_bits() >= opts.min_notes as u32)
            .map(|next| next.at)
            .unwrap_or(window.until);
        let duration = next_onset - window.at;
        if duration <= Time::from_integer(0) {
            continue;
        }
        let notes = notes_for_window(&roll, window.at, opts.window_mode);
        let context = LabelContext {
            root: window.bit_chord.root,
            key: opts.key_hint,
        };
        let label = registry
            .label_all(window.pitch_class_mask, &context)
            .into_iter()
            .find(|label| label.school == school)
            .expect("builtin naming school is installed");
        let velocity = notes
            .iter()
            .map(|item| item.note.velocity)
            .max()
            .unwrap_or(100);
        let channel = notes
            .first()
            .map(|item| item.note.channel)
            .unwrap_or_else(|| {
                sim_lib_music_core::Channel::new(0).expect("default MIDI channel is valid")
            });
        chords.push(Chord::new(
            duration,
            label.text,
            unique_pitches(&notes),
            velocity,
            channel,
        )?);
    }

    Ok(LiftReport {
        value: Progression::new(opts.key_hint.map(format_key), chords)?,
        diagnostics: collected.diagnostics,
    })
}

fn validate_progression_opts(opts: &ProgressionLiftOpts) -> Result<(), LiftError> {
    if opts.grid <= Time::from_integer(0) {
        return Err(LiftError::InvalidGrid);
    }
    if opts.min_notes == 0 {
        return Err(LiftError::InvalidMinNotes);
    }
    Ok(())
}

fn quantize_roll(roll: &PianoRoll, grid: Time) -> Result<PianoRoll, LiftError> {
    if grid <= Time::from_integer(0) {
        return Err(LiftError::InvalidGrid);
    }
    PianoRoll::new(
        roll.items
            .iter()
            .map(|item| {
                let onset = quantize_time(item.onset, grid);
                let end = quantize_time(item.onset + item.note.duration, grid);
                let duration = if end > onset { end - onset } else { grid };
                TimedNote {
                    onset,
                    note: sim_lib_music_core::Note {
                        duration,
                        ..item.note.clone()
                    },
                }
            })
            .collect(),
    )
    .map_err(LiftError::from)
}

fn quantize_time(time: Time, grid: Time) -> Time {
    let steps = time / grid;
    let numerator = *steps.numer();
    let denominator = *steps.denom();
    let rounded = (numerator * 2 + denominator) / (2 * denominator);
    grid * Ratio::from_integer(rounded)
}

fn notes_for_window(roll: &PianoRoll, at: Time, mode: ChordWindowMode) -> Vec<TimedNote> {
    roll.items
        .iter()
        .filter(|item| match mode {
            ChordWindowMode::SoundingNotes => {
                item.onset <= at && at < item.onset + item.note.duration
            }
            ChordWindowMode::StartingNotes => item.onset == at,
        })
        .cloned()
        .collect()
}

fn unique_pitches(notes: &[TimedNote]) -> Vec<Pitch> {
    let mut pitches = notes.iter().map(|item| item.note.pitch).collect::<Vec<_>>();
    pitches.sort();
    pitches.dedup();
    pitches
}

fn format_key(key: sim_lib_pitch_scale::Key) -> String {
    format!("{}-{}", key.tonic.canonical_name(), key.mode.name())
}
