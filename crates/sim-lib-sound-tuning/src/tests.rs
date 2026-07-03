use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_sound_core::Frequency;

use crate::{
    EqualTemperament, PitchClassN, ScalaScl, Tuning, TuningDescriptor, cents_between,
    default_just_intonation, render_pitch_with_tuning,
};

#[test]
fn a4_maps_to_440_in_default_equal_temperament() {
    let tuning = EqualTemperament::default();
    assert!((render_pitch_with_tuning(Pitch::from_midi(69), &tuning).0 - 440.0).abs() < 1e-9);
}

#[test]
fn octave_ratio_is_two_in_twelve_tet() {
    let tuning = EqualTemperament::default();
    let a4 = tuning.frequency_of(Pitch::from_midi(69));
    let a5 = tuning.frequency_of(Pitch::from_midi(81));
    assert!((a5.ratio(a4) - 2.0).abs() < 1e-9);
}

#[test]
fn pitch_class_n_works_for_non_twelve_equal_temperament() {
    let tuning = EqualTemperament::new(19, (Pitch::from_midi(69), Frequency(440.0))).unwrap();
    let degree = PitchClassN::new(19, 14).unwrap();
    let frequency = tuning.frequency_of_degree(degree, 4).unwrap();
    assert!((frequency.0 - 440.0).abs() < 2.0);
}

#[test]
fn scala_scl_parses_small_fixture() {
    let scala = ScalaScl::parse(
        "! comment\nsmall-fixture\n5\n!\n100.0\n200.0\n3/2\n700.0\n2/1\n",
        (Pitch::from_midi(69), Frequency(440.0)),
    )
    .unwrap();
    assert_eq!(scala.cents.len(), 5);
    assert!((scala.cents[2] - 701.955).abs() < 0.1);
}

#[test]
fn ji_fifth_differs_from_twelve_tet_by_expected_tolerance() {
    let ji = default_just_intonation();
    let tet = EqualTemperament::default();
    let c4 = Pitch {
        class: PitchClass::C,
        octave: 4,
    };
    let g4 = Pitch {
        class: PitchClass::G,
        octave: 4,
    };
    let delta = cents_between(c4, g4, &ji) - cents_between(c4, g4, &tet);
    assert!((delta.abs() - 1.955).abs() < 0.25);
}

#[test]
fn twelve_chroma_only_diagnostic_mentions_pitch_class_n() {
    let error = PitchClassN::new(19, 3)
        .unwrap()
        .to_pitch_class()
        .unwrap_err();
    assert!(format!("{error}").contains("12-chroma-only"));
}

#[test]
fn descriptor_round_trips_to_tuning() {
    let descriptor = TuningDescriptor::PythagoreanTuning {
        reference_midi: 69,
        reference_hz: 440.0,
    };
    let tuning = descriptor.to_tuning().unwrap();
    let a4 = tuning.frequency_of(Pitch::from_midi(69));
    assert!((a4.0 - 440.0).abs() < 1e-9);
}
