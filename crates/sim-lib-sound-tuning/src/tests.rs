use sim_lib_pitch_core::{Pitch, PitchClass};
use std::time::Duration;

use sim_lib_sound_core::{Amplitude, Frequency, Partial, PartialTag, Phase, Tone};

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
fn edo19_fixture_reuses_equal_temperament_for_harmonic_partials() {
    let edo19 = EqualTemperament::new(19, (Pitch::from_midi(69), Frequency(440.0))).unwrap();
    let fundamental = edo19.frequency_of(Pitch::from_midi(69));
    let partials = (1..=8)
        .map(|n| {
            Partial::tagged(
                Frequency(fundamental.0 * n as f64),
                Amplitude(1.0 / n as f64),
                Phase(0.0),
                PartialTag::harmonic(n).unwrap(),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let tone = Tone::from_partials(
        partials,
        sim_lib_sound_core::default_envelope(),
        Duration::from_secs(1),
    )
    .unwrap();

    assert_eq!(edo19.divisions(), 19);
    assert_eq!(tone.partials.len(), 8);
    assert_eq!(tone.partials[0].frequency, fundamental);
    assert_eq!(tone.partials[7].tag, PartialTag::Harmonic(8));
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

#[test]
fn descriptor_rejects_invalid_reference_frequency() {
    let descriptor = TuningDescriptor::EqualTemperament {
        divisions: 19,
        reference_midi: 69,
        reference_hz: f64::NAN,
    };
    assert_eq!(
        descriptor.to_tuning().err(),
        Some(crate::SoundTuningError::InvalidReferenceFrequency)
    );
}
