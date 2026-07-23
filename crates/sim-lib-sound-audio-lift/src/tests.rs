use std::sync::Arc;

// conformance: audio lift and render workflows expose checked audio analysis descriptors.

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, ExportKind, Symbol};
use sim_lib_sound_tuning::EqualTemperament;

use crate::{
    AudioLiftOptions, AudioLifter, FftPeakLifter, HarmonicCombLifter, install_sound_audio_lift_lib,
};

#[cfg(feature = "sound-music")]
use crate::{lifted_notes_to_counterpoint, lifted_notes_to_diff_roll, lifted_notes_to_piano_roll};

#[test]
fn synthetic_a4_sine_lifts_with_high_confidence() {
    let samples = sine_mix(&[(440.0, 1.0)], 48_000, 0.25, 0.0);
    let tuning = EqualTemperament::default();
    let report = HarmonicCombLifter::default()
        .lift_report(&samples, 48_000, &tuning)
        .unwrap();
    let note = report
        .value
        .notes
        .iter()
        .max_by(|left, right| left.confidence.total_cmp(&right.confidence))
        .expect("lifted note");
    assert_eq!(note.pitch.to_midi(), Some(69));
    assert!(note.confidence >= 0.75, "{note:?}");
}

#[test]
fn simultaneous_sines_lift_to_two_pitch_candidates() {
    let samples = sine_mix(&[(440.0, 1.0), (659.25, 0.8)], 48_000, 0.25, 0.0);
    let tuning = EqualTemperament::default();
    let report = FftPeakLifter::default()
        .lift_report(&samples, 48_000, &tuning)
        .unwrap();
    let frame = report
        .value
        .frames
        .iter()
        .max_by_key(|frame| frame.pitch_candidates.len())
        .expect("frame");
    let lifted = frame
        .pitch_candidates
        .iter()
        .filter_map(|candidate| candidate.pitch.to_midi())
        .collect::<Vec<_>>();
    assert!(lifted.contains(&69), "{lifted:?}");
    assert!(lifted.contains(&76), "{lifted:?}");
}

#[test]
fn noisy_audio_has_lower_confidence() {
    let tuning = EqualTemperament::default();
    let clean = sine_mix(&[(440.0, 1.0)], 48_000, 0.25, 0.0);
    let noisy = sine_mix(&[(440.0, 1.0)], 48_000, 0.25, 0.35);
    let lifter = HarmonicCombLifter::default();
    let clean_note = lifter
        .lift_report(&clean, 48_000, &tuning)
        .unwrap()
        .value
        .notes
        .into_iter()
        .max_by(|left, right| left.confidence.total_cmp(&right.confidence))
        .unwrap();
    let noisy_note = lifter
        .lift_report(&noisy, 48_000, &tuning)
        .unwrap()
        .value
        .notes
        .into_iter()
        .max_by(|left, right| left.confidence.total_cmp(&right.confidence))
        .unwrap();
    assert!(noisy_note.confidence < clean_note.confidence);
}

#[test]
fn note_candidates_include_onset_and_duration_estimates() {
    let samples = stepped_sine(440.0, 48_000, 0.10, 0.10, 0.10);
    let tuning = EqualTemperament::default();
    let report = HarmonicCombLifter {
        opts: AudioLiftOptions {
            min_note_windows: 1,
            ..AudioLiftOptions::default()
        },
    }
    .lift_report(&samples, 48_000, &tuning)
    .unwrap();
    assert!(!report.value.notes.is_empty());
    let note = &report.value.notes[0];
    assert!(note.onset_sample > 0);
    assert!(note.duration_samples > 0);
}

#[cfg(feature = "sound-music")]
#[test]
fn lifted_notes_convert_to_music_views() {
    let samples = sine_mix(&[(440.0, 1.0)], 48_000, 0.20, 0.0);
    let tuning = EqualTemperament::default();
    let report = HarmonicCombLifter::default()
        .lift_report(&samples, 48_000, &tuning)
        .unwrap();
    let roll = lifted_notes_to_piano_roll(&report.value.notes).unwrap();
    assert!(!roll.items.is_empty());
    let diff = lifted_notes_to_diff_roll(&report.value.notes).unwrap();
    assert!(!diff.frames.is_empty());
    let counterpoint = lifted_notes_to_counterpoint(&report.value.notes).unwrap();
    assert!(!counterpoint.voices.is_empty());
}

#[test]
fn empty_audio_returns_diagnostics_and_no_notes() {
    let tuning = EqualTemperament::default();
    let report = FftPeakLifter::default()
        .lift_report(&[], 48_000, &tuning)
        .unwrap();
    assert!(report.value.notes.is_empty());
    assert!(!report.diagnostics.is_empty());
}

#[test]
fn install_runtime_is_idempotent_and_registers_audio_lifters() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_sound_audio_lift_lib(&mut cx).unwrap();
    install_sound_audio_lift_lib(&mut cx).unwrap();

    let loaded = cx
        .registry()
        .lib(&Symbol::new("sound-audio-lift"))
        .expect("sound-audio-lift lib");
    assert!(loaded.exports.iter().any(|record| {
        record.kind == ExportKind::named("AudioLifter")
            && record.symbol == Symbol::qualified("sound", "FftPeakLifter")
    }));
    assert!(loaded.exports.iter().any(|record| {
        record.kind == ExportKind::named("AudioLifter")
            && record.symbol == Symbol::qualified("sound", "HarmonicCombLifter")
    }));
}

fn sine_mix(tones: &[(f64, f64)], sample_rate: u32, seconds: f64, noise: f64) -> Vec<f32> {
    let samples = (seconds * f64::from(sample_rate)).round() as usize;
    (0..samples)
        .map(|index| {
            let t = index as f64 / f64::from(sample_rate);
            let signal = tones
                .iter()
                .map(|(hz, gain)| (std::f64::consts::TAU * hz * t).sin() * gain)
                .sum::<f64>();
            let hiss = if noise > 0.0 {
                noise * pseudo_noise(index)
            } else {
                0.0
            };
            (signal + hiss).clamp(-1.0, 1.0) as f32
        })
        .collect()
}

fn stepped_sine(hz: f64, sample_rate: u32, lead_silence: f64, tone: f64, tail: f64) -> Vec<f32> {
    let silence = vec![0.0_f32; (lead_silence * f64::from(sample_rate)).round() as usize];
    let mut out = silence;
    out.extend(sine_mix(&[(hz, 1.0)], sample_rate, tone, 0.0));
    out.extend(vec![
        0.0_f32;
        (tail * f64::from(sample_rate)).round() as usize
    ]);
    out
}

fn pseudo_noise(index: usize) -> f64 {
    let x = ((index as u64).wrapping_mul(1103515245).wrapping_add(12345) % 65536) as f64;
    (x / 32768.0) - 1.0
}
