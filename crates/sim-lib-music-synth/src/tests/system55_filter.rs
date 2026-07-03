use std::f32::consts::TAU;

use sim_kernel::Symbol;

use crate::{
    ComponentCapability, ComponentRegistryCategory, DiscreteComponent, System55FilterCoupler,
    System55FilterCouplerSettings, System55HighPassFilter, System55HighPassFilterSettings,
    System55LadderLpf, System55LadderLpfSettings, default_audio_synth_registry,
    m55_coupler_component_id, m55_hpf_component_id, m55_ladder_lpf_component_id,
    system55_filter_fixture_names, system55_filter_model_notes, system55_filter_module_ids,
};

const SAMPLE_RATE: f32 = 9_600.0;

#[test]
fn system55_filter_records_ids_model_notes_and_fixture_names() {
    assert_eq!(
        system55_filter_module_ids()
            .iter()
            .map(Symbol::as_qualified_str)
            .collect::<Vec<_>>(),
        vec![
            "audio-synth/module/m55-904a-low-pass-filter",
            "audio-synth/module/m55-904b-high-pass-filter",
            "audio-synth/module/m55-904c-filter-coupler",
        ]
    );
    assert_eq!(
        system55_filter_fixture_names(),
        [
            "system55-m55-ladder-lpf-slope",
            "system55-m55-ladder-lpf-resonance-peak",
            "system55-m55-ladder-lpf-self-oscillation",
            "system55-m55-ladder-lpf-saturation",
            "system55-m55-hpf-slope",
            "system55-m55-coupler-bandpass",
        ]
    );
    assert!(
        system55_filter_model_notes()
            .iter()
            .any(|note| note.contains("4x oversampling"))
    );
}

#[test]
fn system55_filter_registry_entries_are_exact_components() {
    let registry = default_audio_synth_registry();
    for id in system55_filter_module_ids() {
        let entry = registry.get(&id).expect("System 55 filter entry");
        assert_eq!(entry.category(), ComponentRegistryCategory::Exact);
        assert!(entry.is_implemented());
        assert!(entry.has_capability(ComponentCapability::RealtimeSafe));
        assert!(entry.has_capability(ComponentCapability::Traceable));
        assert_eq!(entry.instantiate().unwrap().component_id(), id);
    }
}

#[test]
fn m55_ladder_lpf_tracks_cutoff_and_24db_slope() {
    let mut filter = ladder(System55LadderLpfSettings {
        cutoff_hz: 800.0,
        resonance: 0.1,
        drive: 0.8,
        ..System55LadderLpfSettings::default()
    });
    assert_close(filter.effective_cutoff_hz(0.0), 800.0, 0.001);
    assert_close(filter.effective_cutoff_hz(1.0), 1_600.0, 0.001);

    let low = render_ladder(&mut filter, &sine(200.0, 2_048), 0.0, 0.0);
    filter.reset();
    filter.set_sample_rate(SAMPLE_RATE);
    let high = render_ladder(&mut filter, &sine(3_200.0, 2_048), 0.0, 0.0);
    assert!(rms(&low[512..]) > rms(&high[512..]) * 5.0);
}

#[test]
fn m55_ladder_lpf_resonates_self_oscillates_and_saturates() {
    let input = sine(800.0, 2_048);
    let mut calm = ladder(System55LadderLpfSettings {
        cutoff_hz: 800.0,
        resonance: 0.1,
        drive: 0.8,
        ..System55LadderLpfSettings::default()
    });
    let mut resonant = ladder(System55LadderLpfSettings {
        cutoff_hz: 800.0,
        resonance: 0.95,
        drive: 0.8,
        ..System55LadderLpfSettings::default()
    });
    let calm_out = render_ladder(&mut calm, &input, 0.0, 0.0);
    let resonant_out = render_ladder(&mut resonant, &input, 0.0, 0.0);
    assert!(rms(&resonant_out[512..]) > rms(&calm_out[512..]) * 1.15);

    let mut oscillator = ladder(System55LadderLpfSettings {
        cutoff_hz: 440.0,
        resonance: 1.25,
        drive: 1.0,
        ..System55LadderLpfSettings::default()
    });
    let output = render_ladder(&mut oscillator, &[0.0; 2_048], 0.0, 0.0);
    assert!(peak(&output[256..]) > 0.12);
    assert_close(
        estimated_frequency_hz(&output[256..], SAMPLE_RATE),
        440.0,
        40.0,
    );

    let mut saturated = ladder(System55LadderLpfSettings {
        cutoff_hz: 2_000.0,
        resonance: 0.2,
        drive: 8.0,
        level: 2.0,
        ..System55LadderLpfSettings::default()
    });
    let driven = (0..128)
        .map(|_| saturated.next_sample(20.0, 0.0, 0.0))
        .collect::<Vec<_>>();
    assert!(driven.iter().all(|sample| sample.abs() <= 1.2501));
}

#[test]
fn m55_hpf_rejects_low_frequencies_and_tracks_cutoff_cv() {
    let mut filter = hpf(System55HighPassFilterSettings {
        cutoff_hz: 600.0,
        drive: 0.8,
        ..System55HighPassFilterSettings::default()
    });
    assert_close(filter.effective_cutoff_hz(0.0), 600.0, 0.001);
    assert_close(filter.effective_cutoff_hz(1.0), 1_200.0, 0.001);

    let low = render_hpf(&mut filter, &sine(100.0, 2_048), 0.0);
    filter.reset();
    filter.set_sample_rate(SAMPLE_RATE);
    let high = render_hpf(&mut filter, &sine(2_000.0, 2_048), 0.0);
    assert!(rms(&high[512..]) > rms(&low[512..]) * 3.0);
}

#[test]
fn m55_coupler_creates_bandpass_window() {
    let mut coupler = System55FilterCoupler::new(System55FilterCouplerSettings {
        low_cutoff_hz: 300.0,
        high_cutoff_hz: 1_200.0,
        drive: 0.8,
        ..System55FilterCouplerSettings::default()
    });
    coupler.set_sample_rate(SAMPLE_RATE);
    assert_eq!(coupler.effective_band_hz(0.0, 0.0), (300.0, 1_200.0));

    let mid = render_coupler(&mut coupler, &sine(700.0, 2_048), 0.0, 0.0);
    coupler.reset();
    coupler.set_sample_rate(SAMPLE_RATE);
    let low = render_coupler(&mut coupler, &sine(80.0, 2_048), 0.0, 0.0);
    coupler.reset();
    coupler.set_sample_rate(SAMPLE_RATE);
    let high = render_coupler(&mut coupler, &sine(3_000.0, 2_048), 0.0, 0.0);
    assert!(rms(&mid[512..]) > rms(&low[512..]) * 2.0);
    assert!(rms(&mid[512..]) > rms(&high[512..]) * 2.0);
}

#[test]
fn system55_filter_modules_implement_discrete_component() {
    fn assert_component<T: DiscreteComponent>() {}
    assert_component::<System55LadderLpf>();
    assert_component::<System55HighPassFilter>();
    assert_component::<System55FilterCoupler>();
    assert_eq!(
        [
            m55_ladder_lpf_component_id(),
            m55_hpf_component_id(),
            m55_coupler_component_id(),
        ],
        system55_filter_module_ids()
    );
}

fn ladder(settings: System55LadderLpfSettings) -> System55LadderLpf {
    let mut filter = System55LadderLpf::new(settings);
    filter.set_sample_rate(SAMPLE_RATE);
    filter
}

fn hpf(settings: System55HighPassFilterSettings) -> System55HighPassFilter {
    let mut filter = System55HighPassFilter::new(settings);
    filter.set_sample_rate(SAMPLE_RATE);
    filter
}

fn render_ladder(
    filter: &mut System55LadderLpf,
    input: &[f32],
    cutoff_cv_v: f32,
    resonance_cv_v: f32,
) -> Vec<f32> {
    input
        .iter()
        .map(|sample| filter.next_sample(*sample, cutoff_cv_v, resonance_cv_v))
        .collect()
}

fn render_hpf(filter: &mut System55HighPassFilter, input: &[f32], cutoff_cv_v: f32) -> Vec<f32> {
    input
        .iter()
        .map(|sample| filter.next_sample(*sample, cutoff_cv_v))
        .collect()
}

fn render_coupler(
    coupler: &mut System55FilterCoupler,
    input: &[f32],
    low_cv_v: f32,
    high_cv_v: f32,
) -> Vec<f32> {
    input
        .iter()
        .map(|sample| coupler.next_sample(*sample, low_cv_v, high_cv_v))
        .collect()
}

fn sine(frequency_hz: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|frame| (TAU * frequency_hz * frame as f32 / SAMPLE_RATE).sin())
        .collect()
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len().max(1) as f32).sqrt()
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().copied().map(f32::abs).fold(0.0, f32::max)
}

fn estimated_frequency_hz(samples: &[f32], sample_rate_hz: f32) -> f32 {
    let crossings = samples
        .windows(2)
        .filter(|window| window[0] <= 0.0 && window[1] > 0.0)
        .count();
    crossings as f32 * sample_rate_hz / samples.len().max(1) as f32
}

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {actual} within {tolerance} of {expected}"
    );
}
