use super::*;

// conformance: pitch and sound vocabulary exposes stable pitch descriptors.

#[test]
fn semitone_round_trip() {
    for semitone in -512..=512 {
        assert_eq!(Pitch::from_semitone(semitone).semitone(), semitone);
    }
}

#[test]
fn midi_round_trip() {
    for midi in 0..=127u8 {
        assert_eq!(Pitch::from_midi(midi).to_midi(), Some(midi));
    }
}

#[test]
fn transposition_composition_holds() {
    let pitch = Pitch::from_midi(60);
    assert_eq!(pitch.transpose(7).transpose(-7), pitch);
}

#[test]
fn inversion_twice_is_identity() {
    let axis = Pitch::from_midi(60);
    let pitch = Pitch::from_midi(67);
    assert_eq!(pitch.invert(axis).invert(axis), pitch);
}

#[test]
fn reader_sugar_uses_canonical_sharps() {
    assert_eq!(parse_pitch("Eb5").unwrap(), Pitch::from_semitone(75));
    assert_eq!(parse_pitch("Cs4").unwrap().class, PitchClass::CS);
    assert_eq!(parse_interval("TT").unwrap(), Interval::TRITONE);
}

#[test]
fn pitch_class_constructor_rejects_invalid_values() {
    assert_eq!(PitchClass::new(12), Err(PitchError::InvalidPitchClass(12)));
    assert_eq!(PitchClass::B.value(), 11);
}

#[test]
fn octave_space_rejects_zero_divisions() {
    assert_eq!(OctaveSpace::new(0), Err(PitchError::InvalidOctaveSpace(0)));
}

#[test]
fn floor_decomposition_folds_negative_and_large_inputs() {
    let twelve = OctaveSpace::twelve_tone();
    assert_eq!(split_floor(-1, twelve), (-1, 11));
    assert_eq!(split_floor(-12, twelve), (-1, 0));
    assert_eq!(split_floor(-13, twelve), (-2, 11));
    assert_eq!(split_floor(25, twelve), (2, 1));

    let spaces = [1, 2, 5, 12, 19, 31, u16::MAX];
    let values = [
        i64::MIN + 1,
        -1_000_000_003,
        -2048,
        -13,
        -12,
        -1,
        0,
        1,
        11,
        12,
        13,
        2048,
        1_000_000_003,
        i64::MAX,
    ];
    for divisions in spaces {
        let space = OctaveSpace::new(divisions).unwrap();
        let width = i128::from(divisions);
        for value in values {
            let (octave, class) = split_floor(value, space);
            assert!(class < divisions);
            assert_eq!(
                i128::from(octave) * width + i128::from(class),
                i128::from(value)
            );
            assert_eq!(fold(value, space), class);
        }
    }
}

#[test]
fn folded_distances_handle_negative_and_large_inputs() {
    let pairs = [
        (-25, -13),
        (-13, -1),
        (-13, 14),
        (-1, -13),
        (0, 7),
        (7, 0),
        (1_000_000_003, -1_000_000_003),
    ];
    for divisions in [1, 5, 12, 19, 31, u16::MAX] {
        let space = OctaveSpace::new(divisions).unwrap();
        for (a, b) in pairs {
            let signed = folded_distance(a, b, space, TieDirection::Ascending);
            let unsigned = folded_unsigned_distance(a, b, space);
            assert_eq!(fold(a + i64::from(signed), space), fold(b, space));
            assert_eq!(signed.unsigned_abs() as u16, unsigned);
            assert!(i32::from(unsigned) <= i32::from(divisions / 2 + divisions % 2));
        }
    }
}

#[test]
fn folded_distance_uses_explicit_tie_policy() {
    let twelve = OctaveSpace::twelve_tone();
    assert_eq!(folded_distance(0, 6, twelve, TieDirection::Ascending), 6);
    assert_eq!(folded_distance(0, 6, twelve, TieDirection::Descending), -6);
    assert_eq!(folded_unsigned_distance(0, 6, twelve), 6);

    let even_space = OctaveSpace::new(20).unwrap();
    assert_eq!(
        folded_distance(-45, 25, even_space, TieDirection::Ascending),
        10
    );
    assert_eq!(
        folded_distance(-45, 25, even_space, TieDirection::Descending),
        -10
    );
}
