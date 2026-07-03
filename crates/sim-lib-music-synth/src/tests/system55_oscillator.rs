use sim_kernel::Symbol;

use crate::{
    ComponentCapability, ComponentRegistryCategory, System55Noise, System55NoiseColor,
    System55NoiseSettings, System55Vco, System55VcoDriver, System55VcoDriverSettings,
    System55VcoSettings, System55VcoWaveform, default_audio_synth_registry,
    system55_oscillator_fixture_names, system55_oscillator_module_ids,
};

#[test]
fn system55_oscillator_bank_records_ids_and_fixture_names() {
    assert_eq!(
        system55_oscillator_module_ids()
            .iter()
            .map(Symbol::as_qualified_str)
            .collect::<Vec<_>>(),
        vec![
            "audio-synth/module/m55-921a-oscillator-driver",
            "audio-synth/module/m55-921b-oscillator",
            "audio-synth/module/m55-923-noise-filter",
        ]
    );
    assert_eq!(
        system55_oscillator_fixture_names(),
        [
            "system55-m55-vco-driver-fanout",
            "system55-m55-vco-pitch-sync-pwm",
            "system55-m55-vco-shared-driver-tracking",
            "system55-m55-noise-white-pink-bands",
        ]
    );
}

#[test]
fn system55_oscillator_registry_entries_are_exact_components() {
    let registry = default_audio_synth_registry();
    for id in system55_oscillator_module_ids() {
        let entry = registry.get(&id).expect("System 55 oscillator entry");
        assert_eq!(entry.category(), ComponentRegistryCategory::Exact);
        assert!(entry.is_implemented());
        assert!(entry.has_capability(ComponentCapability::RealtimeSafe));
        assert!(entry.has_capability(ComponentCapability::Traceable));

        let component = entry.instantiate().expect("implemented component");
        assert_eq!(component.component_id(), id);
    }
}

#[test]
fn m55_vco_driver_fans_out_pitch_modulation_and_sync() {
    let mut driver = System55VcoDriver::new(System55VcoDriverSettings {
        transpose_octaves: 1.0,
        fine_tune_semitones: 12.0,
        modulation_depth_octaves: 2.0,
    });

    let frame = driver.next_frame(0.25, 0.5, false);
    assert_close(frame.pitch_cv_v, 2.25, 0.0001);
    assert_close(frame.modulation_cv_v, 1.0, 0.0001);
    assert_close(frame.tracking_cv_v(), 3.25, 0.0001);
    assert!(!frame.sync_high);
    assert!(!frame.sync_triggered);

    let first_sync = driver.next_frame(0.0, 0.0, true);
    let held_sync = driver.next_frame(0.0, 0.0, true);
    let released_sync = driver.next_frame(0.0, 0.0, false);
    assert!(first_sync.sync_triggered);
    assert!(!held_sync.sync_triggered);
    assert!(!released_sync.sync_triggered);
}

#[test]
fn m55_vco_tracks_driver_pitch_sync_and_pwm() {
    let mut vco = System55Vco::new(System55VcoSettings {
        waveform: System55VcoWaveform::Sine,
        base_frequency_hz: 100.0,
        pulse_width: 0.5,
        pwm_depth: 0.25,
        modulation_depth_octaves: 1.0,
        level: 1.0,
    });
    vco.set_sample_rate(1_000.0);

    assert_close(vco.effective_frequency_hz(0.0, 0.0), 100.0, 0.0001);
    assert_close(vco.effective_frequency_hz(1.0, 0.0), 200.0, 0.0001);
    assert_close(vco.effective_frequency_hz(0.0, 1.0), 200.0, 0.0001);
    assert_close(vco.effective_pulse_width(1.0), 0.75, 0.0001);
    assert_close(vco.effective_pulse_width(4.0), 0.95, 0.0001);
    assert_close(vco.effective_pulse_width(-4.0), 0.05, 0.0001);

    let first = vco.next_sample(0.0, 0.0, 0.0, false);
    let second = vco.next_sample(0.0, 0.0, 0.0, false);
    let synced = vco.next_sample(0.0, 0.0, 0.0, true);
    assert_close(first, 0.0, 0.0001);
    assert!(second > 0.5);
    assert_close(synced, 0.0, 0.0001);

    for waveform in [
        System55VcoWaveform::Saw,
        System55VcoWaveform::Triangle,
        System55VcoWaveform::Pulse,
        System55VcoWaveform::Sine,
    ] {
        let mut osc = System55Vco::new(System55VcoSettings {
            waveform,
            base_frequency_hz: 80.0,
            level: 1.0,
            ..System55VcoSettings::default()
        });
        osc.set_sample_rate(1_000.0);
        let samples = (0..32)
            .map(|_| osc.next_sample(0.0, 0.0, 0.2, false))
            .collect::<Vec<_>>();
        assert!(samples.iter().all(|sample| sample.abs() <= 1.0001));
    }
}

#[test]
fn m55_vco_tracks_shared_driver_frame() {
    let mut driver = System55VcoDriver::default();
    let vco = System55Vco::new(System55VcoSettings {
        base_frequency_hz: 55.0,
        modulation_depth_octaves: 1.0,
        ..System55VcoSettings::default()
    });

    let root = driver.next_frame(0.0, 0.0, false);
    let octave = driver.next_frame(1.0, 0.0, false);
    let modulated = driver.next_frame(0.0, 0.5, false);

    assert_close(vco.effective_frequency_from_driver(root), 55.0, 0.0001);
    assert_close(vco.effective_frequency_from_driver(octave), 110.0, 0.0001);
    assert_close(
        vco.effective_frequency_from_driver(modulated),
        55.0 * 2.0_f32.sqrt(),
        0.0001,
    );
}

#[test]
fn m55_noise_generates_deterministic_white_and_smoother_pink() {
    let settings = System55NoiseSettings {
        color: System55NoiseColor::White,
        level: 1.0,
        seed: 0x1234_5678,
    };
    let mut first = System55Noise::new(settings);
    let mut second = System55Noise::new(settings);
    let first_run = (0..32).map(|_| first.next_sample()).collect::<Vec<_>>();
    let second_run = (0..32).map(|_| second.next_sample()).collect::<Vec<_>>();
    assert_eq!(round4(&first_run), round4(&second_run));

    let mut noise = System55Noise::new(settings);
    let frames = (0..512).map(|_| noise.next_frame()).collect::<Vec<_>>();
    assert!(
        frames
            .iter()
            .all(|frame| frame.white.abs() <= 1.0001 && frame.pink.abs() <= 1.0001)
    );

    let white = frames.iter().map(|frame| frame.white).collect::<Vec<_>>();
    let pink = frames.iter().map(|frame| frame.pink).collect::<Vec<_>>();
    assert!(mean_abs_delta(&pink) < mean_abs_delta(&white) * 0.85);
    assert!(rms(&white) > 0.45 && rms(&white) < 0.70);
    assert!(rms(&pink) > 0.10 && rms(&pink) < 0.55);
}

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {actual} within {tolerance} of {expected}"
    );
}

fn mean_abs_delta(samples: &[f32]) -> f32 {
    samples
        .windows(2)
        .map(|window| (window[1] - window[0]).abs())
        .sum::<f32>()
        / (samples.len().saturating_sub(1).max(1) as f32)
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len().max(1) as f32).sqrt()
}

fn round4(samples: &[f32]) -> Vec<i32> {
    samples
        .iter()
        .map(|sample| (sample * 10_000.0).round() as i32)
        .collect()
}
