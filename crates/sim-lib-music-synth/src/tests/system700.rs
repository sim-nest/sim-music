use sim_kernel::Symbol;

use crate::{
    ComponentCapability, ComponentRegistryCategory, DiscreteComponent, InstrumentWrapperCategory,
    default_audio_synth_registry,
    system700::{
        System700Lfo, System700LfoSettings, System700LfoWaveform, System700Noise,
        System700NoiseColor, System700NoiseSettings, System700Vco, System700VcoSettings,
        System700VcoWaveform, r700_lfo_component_id, r700_noise_component_id,
        r700_vco_component_id, system700_scaffold_patch, system700_source_fixture_names,
        system700_source_module_ids,
    },
};

#[test]
fn system700_scaffold_records_source_ids_and_fixture_names() {
    assert_eq!(
        system700_source_module_ids()
            .iter()
            .map(Symbol::as_qualified_str)
            .collect::<Vec<_>>(),
        vec![
            "audio-synth/module/r700-vco",
            "audio-synth/module/r700-lfo",
            "audio-synth/module/r700-noise",
        ]
    );
    assert_eq!(
        system700_source_fixture_names(),
        [
            "system700-r700-vco-pitch-sync-pwm",
            "system700-r700-vco-exponential-fm",
            "system700-r700-lfo-delay-rate-cv",
            "system700-r700-noise-color-bands",
        ]
    );

    let patch = system700_scaffold_patch();
    assert_eq!(patch.modules.len(), 3);
    assert_eq!(patch.modules[0].kind, r700_vco_component_id());
    assert_eq!(patch.modules[1].kind, r700_lfo_component_id());
    assert_eq!(patch.modules[2].kind, r700_noise_component_id());
}

#[test]
fn system700_source_registry_entries_are_implemented() {
    let registry = default_audio_synth_registry();
    for id in [
        r700_vco_component_id(),
        r700_lfo_component_id(),
        r700_noise_component_id(),
    ] {
        let entry = registry.get(&id).expect("source module entry");
        assert_eq!(entry.category(), ComponentRegistryCategory::Exact);
        assert_eq!(entry.wrapper(), InstrumentWrapperCategory::ModularAnalog);
        assert!(entry.is_implemented());
        assert!(entry.has_capability(ComponentCapability::RealtimeSafe));
        assert!(entry.has_capability(ComponentCapability::Traceable));
        assert_eq!(entry.instantiate().unwrap().component_id(), id);
    }
}

#[test]
fn r700_vco_tracks_pitch_sync_and_pwm() {
    let mut vco = System700Vco::new(System700VcoSettings {
        waveform: System700VcoWaveform::Pulse,
        base_frequency_hz: 110.0,
        pulse_width: 0.5,
        pwm_depth: 0.5,
        exp_fm_depth_octaves: 2.0,
        level: 1.0,
    });
    vco.set_sample_rate(48_000.0);

    assert_eq!(round3(vco.effective_frequency_hz(0.0, 0.0)), 110.0);
    assert_eq!(round3(vco.effective_frequency_hz(1.0, 0.0)), 220.0);
    assert_eq!(round3(vco.effective_frequency_hz(0.0, 0.5)), 220.0);
    assert_eq!(round3(vco.effective_pulse_width(-4.0)), 0.05);
    assert_eq!(round3(vco.effective_pulse_width(4.0)), 0.95);

    for _ in 0..32 {
        vco.next_sample(0.0, 0.0, 0.0, false);
    }
    assert!(vco.phase() > 0.0);
    assert_eq!(vco.next_sample(0.0, 0.0, 0.0, true), 1.0);
    assert!(vco.phase() > 0.0);
}

#[test]
fn r700_vco_waveforms_are_bounded() {
    for waveform in [
        System700VcoWaveform::Saw,
        System700VcoWaveform::Triangle,
        System700VcoWaveform::Pulse,
        System700VcoWaveform::Sine,
    ] {
        let mut vco = System700Vco::new(System700VcoSettings {
            waveform,
            base_frequency_hz: 20.0,
            level: 1.0,
            ..System700VcoSettings::default()
        });
        vco.set_sample_rate(1_000.0);
        let samples = (0..128)
            .map(|_| vco.next_sample(0.0, 0.0, 0.0, false))
            .collect::<Vec<_>>();
        assert!(
            samples
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() <= 1.0001),
            "{waveform:?}"
        );
    }
}

#[test]
fn r700_lfo_delays_and_tracks_rate_cv() {
    let mut lfo = System700Lfo::new(System700LfoSettings {
        waveform: System700LfoWaveform::Square,
        rate_hz: 2.0,
        delay_s: 0.25,
        rate_cv_depth_octaves: 1.0,
        level: 1.0,
    });
    lfo.set_sample_rate(8.0);

    assert_eq!(round3(lfo.effective_rate_hz(0.0)), 2.0);
    assert_eq!(round3(lfo.effective_rate_hz(1.0)), 4.0);
    assert_eq!(round3(lfo.next_sample(0.0)), 0.0);
    assert_eq!(round3(lfo.next_sample(0.0)), 0.5);
    assert_eq!(round3(lfo.next_sample(0.0)), -1.0);
}

#[test]
fn r700_lfo_waveforms_are_bounded() {
    for waveform in [
        System700LfoWaveform::Sine,
        System700LfoWaveform::Triangle,
        System700LfoWaveform::SawUp,
        System700LfoWaveform::SawDown,
        System700LfoWaveform::Square,
    ] {
        let mut lfo = System700Lfo::new(System700LfoSettings {
            waveform,
            rate_hz: 10.0,
            delay_s: 0.0,
            rate_cv_depth_octaves: 1.0,
            level: 1.0,
        });
        lfo.set_sample_rate(100.0);
        let samples = (0..64).map(|_| lfo.next_sample(0.0)).collect::<Vec<_>>();
        assert!(
            samples
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() <= 1.0001),
            "{waveform:?}"
        );
    }
}

#[test]
fn r700_noise_is_deterministic_and_has_color_bands() {
    let mut white = System700Noise::new(System700NoiseSettings {
        color: System700NoiseColor::White,
        seed: 1234,
        level: 1.0,
    });
    let mut white_again = white.clone();
    let first = render_noise(&mut white, 512);
    let second = render_noise(&mut white_again, 512);
    assert_eq!(round6(&first[..32]), round6(&second[..32]));

    let mut pink = System700Noise::new(System700NoiseSettings {
        color: System700NoiseColor::Pink,
        seed: 1234,
        level: 1.0,
    });
    let mut red = System700Noise::new(System700NoiseSettings {
        color: System700NoiseColor::Red,
        seed: 1234,
        level: 1.0,
    });
    let pink = render_noise(&mut pink, 512);
    let red = render_noise(&mut red, 512);

    assert!(difference_energy(&pink) < difference_energy(&first));
    assert!(difference_energy(&red) < difference_energy(&pink));
    assert!(first.iter().all(|sample| sample.abs() <= 1.0));
    assert!(red.iter().all(|sample| sample.abs() <= 1.0));
}

#[test]
fn system700_modules_implement_discrete_component() {
    fn assert_component<T: DiscreteComponent>() {}
    assert_component::<System700Vco>();
    assert_component::<System700Lfo>();
    assert_component::<System700Noise>();
}

fn render_noise(noise: &mut System700Noise, frames: usize) -> Vec<f32> {
    (0..frames).map(|_| noise.next_sample()).collect()
}

fn difference_energy(samples: &[f32]) -> f32 {
    samples
        .windows(2)
        .map(|window| {
            let diff = window[1] - window[0];
            diff * diff
        })
        .sum::<f32>()
        / samples.len().max(1) as f32
}

fn round3(value: f32) -> f32 {
    (value * 1_000.0).round() / 1_000.0
}

fn round6(values: &[f32]) -> Vec<f32> {
    values
        .iter()
        .map(|value| (value * 1_000_000.0).round() / 1_000_000.0)
        .collect()
}
