use std::f32::consts::TAU;

use sim_kernel::Symbol;

use crate::{
    ComponentCapability, ComponentRegistryCategory, DiscreteComponent, InstrumentWrapperCategory,
    default_audio_synth_registry,
    system700::{
        System700RingModulator, System700RingSettings, System700Vca, System700VcaResponse,
        System700VcaSettings, System700Vcf, System700VcfMode, System700VcfSettings,
        r700_ring_component_id, r700_vca_component_id, r700_vcf_component_id,
        system700_shaper_fixture_names, system700_shaper_module_ids, system700_vcf_mode_symbols,
    },
};

#[test]
fn system700_shaper_ids_modes_and_fixtures_are_recorded() {
    assert_eq!(
        system700_shaper_module_ids()
            .iter()
            .map(Symbol::as_qualified_str)
            .collect::<Vec<_>>(),
        vec![
            "audio-synth/module/r700-vcf",
            "audio-synth/module/r700-vca",
            "audio-synth/module/r700-ring",
        ]
    );
    assert_eq!(
        system700_vcf_mode_symbols()
            .iter()
            .map(Symbol::as_qualified_str)
            .collect::<Vec<_>>(),
        vec![
            "audio-synth/r700-vcf-mode/lowpass",
            "audio-synth/r700-vcf-mode/bandpass",
            "audio-synth/r700-vcf-mode/highpass",
            "audio-synth/r700-vcf-mode/notch",
        ]
    );
    assert_eq!(
        system700_shaper_fixture_names(),
        [
            "system700-r700-vcf-cutoff-tracking",
            "system700-r700-vcf-resonance-self-oscillation",
            "system700-r700-vca-gain-law",
            "system700-r700-vca-saturation",
            "system700-r700-ring-sidebands",
        ]
    );
}

#[test]
fn system700_shaper_registry_entries_are_implemented() {
    let registry = default_audio_synth_registry();
    for id in [
        r700_vcf_component_id(),
        r700_vca_component_id(),
        r700_ring_component_id(),
    ] {
        let entry = registry.get(&id).expect("shaper module entry");
        assert_eq!(entry.category(), ComponentRegistryCategory::Exact);
        assert_eq!(entry.wrapper(), InstrumentWrapperCategory::ModularAnalog);
        assert!(entry.is_implemented());
        assert!(entry.has_capability(ComponentCapability::RealtimeSafe));
        assert!(entry.has_capability(ComponentCapability::Traceable));
        assert_eq!(entry.instantiate().unwrap().component_id(), id);
    }
}

#[test]
fn r700_vcf_tracks_cutoff_and_resonance_peak() {
    let mut calm = vcf(System700VcfMode::BandPass, 0.1);
    let mut resonant = vcf(System700VcfMode::BandPass, 0.9);
    calm.set_sample_rate(4_800.0);
    resonant.set_sample_rate(4_800.0);

    assert_eq!(round3(calm.effective_cutoff_hz(0.0)), 240.0);
    assert_eq!(round3(calm.effective_cutoff_hz(1.0)), 480.0);

    let input = sine(240.0, 4_800.0, 512);
    let calm_out = render_vcf(&mut calm, &input, 0.0);
    let resonant_out = render_vcf(&mut resonant, &input, 0.0);
    assert!(rms(&resonant_out[128..]) > rms(&calm_out[128..]) * 1.2);
}

#[test]
fn r700_vcf_modes_are_bounded_and_self_oscillate() {
    for mode in [
        System700VcfMode::LowPass,
        System700VcfMode::BandPass,
        System700VcfMode::HighPass,
        System700VcfMode::Notch,
    ] {
        let mut filter = vcf(mode, 0.5);
        filter.set_sample_rate(4_800.0);
        let input = impulse(128);
        let output = render_vcf(&mut filter, &input, 0.0);
        assert!(
            output
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() <= 4.0001),
            "{mode:?}"
        );
    }

    let mut oscillator = vcf(System700VcfMode::LowPass, 1.2);
    oscillator.set_sample_rate(4_800.0);
    let output = render_vcf(&mut oscillator, &[0.0; 256], 0.0);
    assert!(peak(&output[32..]) > 0.05);
}

#[test]
fn r700_vca_gain_law_and_saturation_are_bounded() {
    let linear = System700Vca::new(System700VcaSettings {
        response: System700VcaResponse::Linear,
        gain: 1.0,
        saturation_drive: 2.0,
    });
    let exponential = System700Vca::new(System700VcaSettings {
        response: System700VcaResponse::Exponential,
        gain: 1.0,
        saturation_drive: 2.0,
    });
    assert_eq!(round3(linear.gain_for_cv(0.5)), 0.5);
    assert_eq!(round3(exponential.gain_for_cv(0.5)), 0.25);

    let mut saturated = System700Vca::new(System700VcaSettings {
        response: System700VcaResponse::Saturated,
        gain: 2.0,
        saturation_drive: 4.0,
    });
    assert_eq!(round3(saturated.next_sample(0.5, 1.0)), 1.0);
    assert!(saturated.next_sample(4.0, 1.0).abs() <= 1.0001);
}

#[test]
fn r700_ring_modulator_emits_sum_and_difference_sidebands() {
    let sample_rate = 4_800.0;
    let carrier = sine(330.0, sample_rate, 4_800);
    let modulator = sine(220.0, sample_rate, 4_800);
    let mut ring = System700RingModulator::new(System700RingSettings { level: 1.0 });
    let output = carrier
        .iter()
        .zip(modulator.iter())
        .map(|(carrier, modulator)| ring.next_sample(*carrier, *modulator))
        .collect::<Vec<_>>();

    let difference = tone_energy(&output, 110.0, sample_rate);
    let sum = tone_energy(&output, 550.0, sample_rate);
    let carrier_leak = tone_energy(&output, 330.0, sample_rate);
    assert!(difference > carrier_leak * 20.0);
    assert!(sum > carrier_leak * 20.0);
}

#[test]
fn system700_shaper_modules_implement_discrete_component() {
    fn assert_component<T: DiscreteComponent>() {}
    assert_component::<System700Vcf>();
    assert_component::<System700Vca>();
    assert_component::<System700RingModulator>();
}

fn vcf(mode: System700VcfMode, resonance: f32) -> System700Vcf {
    System700Vcf::new(System700VcfSettings {
        mode,
        cutoff_hz: 240.0,
        resonance,
        cutoff_cv_depth_octaves: 1.0,
        level: 1.0,
    })
}

fn render_vcf(filter: &mut System700Vcf, input: &[f32], cutoff_cv: f32) -> Vec<f32> {
    input
        .iter()
        .map(|sample| filter.next_sample(*sample, cutoff_cv))
        .collect()
}

fn sine(frequency_hz: f32, sample_rate_hz: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|frame| (TAU * frequency_hz * frame as f32 / sample_rate_hz).sin())
        .collect()
}

fn impulse(frames: usize) -> Vec<f32> {
    let mut samples = vec![0.0; frames];
    samples[0] = 1.0;
    samples
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len().max(1) as f32).sqrt()
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().copied().map(f32::abs).fold(0.0, f32::max)
}

fn tone_energy(samples: &[f32], frequency_hz: f32, sample_rate_hz: f32) -> f32 {
    let (sin_sum, cos_sum) =
        samples
            .iter()
            .enumerate()
            .fold((0.0, 0.0), |(sin_sum, cos_sum), (frame, sample)| {
                let phase = TAU * frequency_hz * frame as f32 / sample_rate_hz;
                (
                    sin_sum + sample * phase.sin(),
                    cos_sum + sample * phase.cos(),
                )
            });
    (sin_sum * sin_sum + cos_sum * cos_sum) / samples.len().max(1) as f32
}

fn round3(value: f32) -> f32 {
    (value * 1_000.0).round() / 1_000.0
}
