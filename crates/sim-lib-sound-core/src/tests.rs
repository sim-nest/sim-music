use std::time::Duration;

use super::*;
use sim_lib_pitch_core::Pitch;

#[test]
fn frequency_cents_ops_are_inverse() {
    let base = Frequency(440.0);
    let shifted = base.shift_cents(700.0);
    assert!((shifted.cents_above(base) - 700.0).abs() < 1e-6);
}

#[test]
fn amplitude_db_ops_are_inverse() {
    let amplitude = Amplitude::from_db(-6.0);
    assert!((amplitude.to_db() + 6.0).abs() < 1e-6);
}

#[test]
fn sine_has_one_partial() {
    assert_eq!(
        Tone::sine(Frequency(440.0), Duration::from_secs(1))
            .partials
            .len(),
        1
    );
}

#[test]
fn sawtooth_follows_inverse_harmonics() {
    let tone = Tone::sawtooth(Frequency(440.0), Duration::from_secs(1), 3);
    assert!((tone.partials[1].amplitude.0 - 0.5).abs() < 1e-6);
}

#[test]
fn pitch_to_equal_temperament_maps_a4_to_440() {
    assert!((equal_temperament_frequency(Pitch::from_midi(69)).0 - 440.0).abs() < 1e-6);
}

#[test]
fn partial_tags_and_phase_are_validated() {
    let partial = Partial::tagged(
        Frequency(440.0),
        Amplitude(0.5),
        Phase(-std::f64::consts::FRAC_PI_2),
        PartialTag::harmonic(2).unwrap(),
    )
    .unwrap();
    assert_eq!(partial.tag, PartialTag::Harmonic(2));
    assert!((partial.phase.0 - std::f64::consts::TAU * 0.75).abs() < 1e-9);

    assert_eq!(
        PartialTag::harmonic(0).unwrap_err(),
        SoundCoreError::InvalidPartialTag
    );
    assert_eq!(
        Partial::new(Frequency(f64::NAN), Amplitude(1.0), Phase(0.0)).unwrap_err(),
        SoundCoreError::InvalidFrequency
    );
    assert_eq!(
        Partial::new(Frequency(440.0), Amplitude(-0.1), Phase(0.0)).unwrap_err(),
        SoundCoreError::InvalidAmplitude
    );
    assert_eq!(
        Partial::new(Frequency(440.0), Amplitude(1.0), Phase(f64::INFINITY)).unwrap_err(),
        SoundCoreError::InvalidPhase
    );
}

#[test]
fn tone_from_partials_normalizes_partial_semantics() {
    let tone = Tone::from_partials(
        vec![Partial {
            frequency: Frequency(440.0),
            amplitude: Amplitude(1.0),
            phase: Phase(std::f64::consts::TAU * 3.0),
            tag: PartialTag::Source,
        }],
        default_envelope(),
        Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(tone.partials[0].phase, Phase(0.0));
    assert_eq!(tone.partials[0].tag, PartialTag::Source);
}
