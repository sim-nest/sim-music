use num_rational::Ratio;

// conformance: exact music analysis and transform exposes checked analysis descriptors.

use sim_lib_music_core::{Articulation, Channel, Note, PianoRoll, TimedNote};
use sim_lib_pitch_chord::ChordSymbol;
use sim_lib_pitch_core::{Pitch, PitchClass};

use crate::{
    ChordWindowMode, DiffRoll, chord_windows_from_diff_roll, chord_windows_from_piano_roll,
    tonnetz::{
        CanonicalTriad, TonnetzError, TonnetzMove, TriadQuality, analyze_tonnetz,
        tonnetz_riemann_label, verify_tonnetz_path,
    },
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

#[test]
fn plr_generators_are_involutions_and_commute_with_transposition() {
    let operations = [
        TonnetzMove::Parallel,
        TonnetzMove::LeadingToneExchange,
        TonnetzMove::Relative,
    ];
    for root in 0..12 {
        let root = PitchClass::new(root).expect("pitch class");
        for quality in [TriadQuality::Major, TriadQuality::Minor] {
            let triad = CanonicalTriad::new(root, quality);
            for operation in operations {
                assert_eq!(triad.apply(operation).apply(operation), triad);
                let shifted = CanonicalTriad::new(root.transpose(5), quality);
                let transformed_then_shifted = triad.apply(operation);
                assert_eq!(
                    shifted.apply(operation),
                    CanonicalTriad::new(
                        transformed_then_shifted.root.transpose(5),
                        transformed_then_shifted.quality,
                    )
                );
            }
        }
    }

    let triad = CanonicalTriad::new(PitchClass::F, TriadQuality::Minor);
    let left = [TonnetzMove::Parallel, TonnetzMove::LeadingToneExchange];
    let right = [TonnetzMove::Relative, TonnetzMove::Parallel];
    let concatenated: Vec<_> = left.into_iter().chain(right).collect();
    assert_eq!(triad.apply_moves(&[]), triad);
    assert_eq!(
        triad.apply_moves(&concatenated),
        triad.apply_moves(&left).apply_moves(&right)
    );
}

#[test]
fn c_major_to_a_minor_is_a_reproducible_relative_path() {
    let from = ChordSymbol::parse("C").unwrap().to_chord(4);
    let to = ChordSymbol::parse("Am").unwrap().to_chord(4);
    let moves = [
        TonnetzMove::Parallel,
        TonnetzMove::LeadingToneExchange,
        TonnetzMove::Relative,
    ];
    let path = analyze_tonnetz(&from, &to, &moves, 8).expect("Tonnetz path");

    assert_eq!(path.distance, 1);
    assert_eq!(path.steps[0].operation, TonnetzMove::Relative);
    assert_eq!(path.riemann_labels(), vec!["T(C)", "t(A)"]);
    verify_tonnetz_path(&path, &moves, 8).expect("verified graph path");

    let repeated = analyze_tonnetz(&from, &to, &moves, 8).expect("repeated path");
    assert_eq!(repeated, path);
}

#[test]
fn identity_drives_transform_while_labels_remain_projection() {
    let triad = CanonicalTriad::new(PitchClass::CS, TriadQuality::Major);
    let parallel = triad.apply(TonnetzMove::Parallel);
    assert_eq!(parallel.root, PitchClass::CS);
    assert_eq!(parallel.quality, TriadQuality::Minor);
    assert_eq!(tonnetz_riemann_label(triad), "T(C#)");
    assert_eq!(tonnetz_riemann_label(parallel), "t(C#)");
}

#[test]
fn tonnetz_bounds_and_induced_graph_reachability_fail_closed() {
    let c_major = ChordSymbol::parse("C").unwrap().to_chord(4);
    let a_minor = ChordSymbol::parse("Am").unwrap().to_chord(4);
    assert_eq!(
        analyze_tonnetz(&c_major, &a_minor, &[TonnetzMove::Relative], 0),
        Err(TonnetzError::LimitExceeded {
            limit: 0,
            required: 1,
        })
    );
    assert_eq!(
        analyze_tonnetz(&c_major, &a_minor, &[TonnetzMove::Parallel], 8),
        Err(TonnetzError::Unreachable)
    );
}
