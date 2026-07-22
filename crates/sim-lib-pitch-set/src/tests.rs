use super::*;
use sim_lib_pitch_core::PitchClass;

#[test]
fn mask_rotation_identity_after_twelve() {
    let mask = PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::E, PitchClass::G]);
    assert_eq!(mask.rotate(12), mask);
}

#[test]
fn inversion_twice_is_identity() {
    let mask = PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::DS, PitchClass::G]);
    assert_eq!(mask.invert(PitchClass::C).invert(PitchClass::C), mask);
}

#[test]
fn interval_vector_is_invariant_under_transposition() {
    let mask = PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::E, PitchClass::G]);
    assert_eq!(mask.interval_vector(), mask.rotate(5).interval_vector());
}

#[test]
fn pitch_class_mask_rejects_high_bits() {
    assert_eq!(
        PitchClassMask::new(0x1000),
        Err(PitchSetError::InvalidPitchClassMask(0x1000))
    );
    let mask = PitchClassMask::new(0x0fff).unwrap();
    assert_eq!(mask.count_bits(), 12);
    assert_eq!(mask.bits(), 0x0fff);
}

#[test]
fn pitch_range_represents_full_midi_space() {
    let mut mask = PitchRangeMask::default();
    for key in 0..=127u8 {
        mask.set(key);
    }
    assert_eq!(mask.to_pitches().len(), 128);
}

#[test]
fn third_stack_round_trip() {
    let signature = ThirdStackSignature {
        root: PitchClass::C,
        steps: vec![ThirdStep::Major, ThirdStep::Minor],
        guard: true,
    };
    let encoded = signature.encode().unwrap();
    let decoded = ThirdStackSignature::decode(encoded).unwrap();
    assert_eq!(decoded.root, signature.root);
}

#[test]
fn third_stack_decode_rejects_invalid_root_nibble() {
    for root in 12..=15 {
        assert_eq!(
            ThirdStackSignature::decode(root).unwrap_err(),
            PitchSetError::InvalidThirdStackEncoding
        );
    }
}
