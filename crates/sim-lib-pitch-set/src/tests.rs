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
fn conventional_classification_keeps_numeric_normalization_separate() {
    let major = PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::E, PitchClass::G]);
    assert_eq!(major.normalize().bits(), major.bits());

    let class = classify_set(major, SetEquivalence::Transposition);
    assert_eq!(
        class.normal,
        vec![PitchClass::C, PitchClass::E, PitchClass::G]
    );
    assert_eq!(class.prime, class.normal);
    assert_eq!(class.equivalence, SetEquivalence::Transposition);
}

#[test]
fn transposition_inversion_policy_types_prime_form_explicitly() {
    let major = PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::E, PitchClass::G]);
    let minor = PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::DS, PitchClass::G]);

    let major_under_t = classify_set(major, SetEquivalence::Transposition);
    let major_under_ti = classify_set(major, SetEquivalence::TranspositionInversion);
    let minor_under_ti = classify_set(minor, SetEquivalence::TranspositionInversion);

    assert_ne!(major_under_t, major_under_ti);
    assert_eq!(major_under_ti.prime, minor_under_ti.prime);
    assert_eq!(
        major_under_ti.prime,
        vec![PitchClass::C, PitchClass::DS, PitchClass::G]
    );
}

#[test]
fn normal_order_is_deterministic_under_rotation_and_transposition() {
    let source = PitchClassMask::from_pitch_classes(&[
        PitchClass::CS,
        PitchClass::D,
        PitchClass::E,
        PitchClass::FS,
    ]);
    let transposed = source.rotate(8);

    assert_eq!(source.normal_order(), transposed.normal_order());
    assert_eq!(
        source.normal_order(),
        vec![PitchClass::C, PitchClass::CS, PitchClass::DS, PitchClass::F]
    );
}

#[test]
fn complements_and_subset_relations_use_pitch_class_identity() {
    let trichord =
        PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::D, PitchClass::E]);
    let tetrachord = PitchClassMask::from_pitch_classes(&[
        PitchClass::C,
        PitchClass::D,
        PitchClass::E,
        PitchClass::FS,
    ]);

    assert!(trichord.is_subset_of(tetrachord));
    assert!(tetrachord.is_superset_of(trichord));
    assert_eq!(trichord.complement().count_bits(), 9);
    assert!(!trichord.complement().is_subset_of(tetrachord));
}

#[test]
fn roots_and_symmetries_are_derived_from_the_mask() {
    let dominant_seventh = PitchClassMask::from_pitch_classes(&[
        PitchClass::C,
        PitchClass::E,
        PitchClass::G,
        PitchClass::AS,
    ]);
    assert_eq!(dominant_seventh.roots(), vec![PitchClass::C]);

    let diminished_seventh = PitchClassMask::from_pitch_classes(&[
        PitchClass::C,
        PitchClass::DS,
        PitchClass::FS,
        PitchClass::A,
    ]);
    assert_eq!(
        diminished_seventh.transpositional_symmetries(),
        vec![0, 3, 6, 9]
    );
    assert_eq!(diminished_seventh.inversional_symmetries().len(), 4);
}

#[test]
fn z_relation_and_forte_examples_are_derived_facts() {
    let z15 = PitchClassMask::from_pitch_classes(&[
        PitchClass::C,
        PitchClass::CS,
        PitchClass::E,
        PitchClass::FS,
    ]);
    let z29 = PitchClassMask::from_pitch_classes(&[
        PitchClass::C,
        PitchClass::CS,
        PitchClass::DS,
        PitchClass::G,
    ]);

    assert!(z15.is_z_related_to(z29));
    assert_eq!(z15.interval_vector(), z29.interval_vector());
    assert_eq!(forte_fact_for(z15).unwrap().label, "4-Z15");
    assert_eq!(forte_fact_for(z29).unwrap().label, "4-Z29");
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
