use num_rational::Ratio;

use sim_lib_music_core::{Articulation, Channel, Note, PianoRoll, TimedNote};
use sim_lib_pitch_core::{Pitch, PitchClass};

use crate::{
    ChordWindowMode, DiffRoll, chord_windows_from_diff_roll, chord_windows_from_piano_roll,
};

fn note(midi: u8, onset: Ratio<i64>, duration: Ratio<i64>) -> TimedNote {
    TimedNote {
        onset,
        note: Note::new(
            duration,
            Pitch::from_midi(midi),
            100,
            Channel::new(0).expect("channel"),
            Articulation::Normal,
        )
        .expect("note"),
    }
}

#[test]
fn diff_roll_marks_started_sounding_ended_and_slurred() {
    let roll = PianoRoll::new(vec![
        note(60, Ratio::new(0, 1), Ratio::new(1, 2)),
        note(64, Ratio::new(1, 4), Ratio::new(1, 2)),
    ])
    .expect("roll");
    let diff = DiffRoll::from_piano_roll(&roll);
    assert_eq!(
        diff.frames[0].started.to_pitches(),
        vec![Pitch::from_midi(60)]
    );
    assert_eq!(diff.frames[1].sounding.to_pitches().len(), 2);
    assert_eq!(
        diff.frames[1].slurred.to_pitches(),
        vec![Pitch::from_midi(60)]
    );
    assert_eq!(
        diff.frames[2].ended.to_pitches(),
        vec![Pitch::from_midi(60)]
    );
}

#[test]
fn sounding_and_starting_modes_differ_on_sustained_chord() {
    let roll = PianoRoll::new(vec![
        note(60, Ratio::new(0, 1), Ratio::new(1, 1)),
        note(64, Ratio::new(0, 1), Ratio::new(1, 1)),
        note(67, Ratio::new(1, 2), Ratio::new(1, 2)),
    ])
    .expect("roll");
    let sounding = chord_windows_from_piano_roll(&roll, ChordWindowMode::SoundingNotes);
    let starting = chord_windows_from_piano_roll(&roll, ChordWindowMode::StartingNotes);
    assert_ne!(sounding, starting);
    assert_eq!(
        starting[1].pitch_class_mask,
        sim_lib_pitch_set::PitchClassMask::from_pitch_classes(&[PitchClass::G])
    );
    assert_eq!(sounding[1].pitch_class_mask.count_bits(), 3);
}

#[test]
fn diff_roll_and_window_extraction_agree() {
    let roll = PianoRoll::new(vec![
        note(60, Ratio::new(0, 1), Ratio::new(1, 4)),
        note(67, Ratio::new(1, 4), Ratio::new(1, 4)),
    ])
    .expect("roll");
    let diff = DiffRoll::from_piano_roll(&roll);
    assert_eq!(
        chord_windows_from_piano_roll(&roll, ChordWindowMode::StartingNotes),
        chord_windows_from_diff_roll(&diff, ChordWindowMode::StartingNotes)
    );
}
