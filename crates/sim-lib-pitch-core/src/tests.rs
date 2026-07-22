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
