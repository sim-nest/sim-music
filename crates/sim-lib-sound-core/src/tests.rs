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
