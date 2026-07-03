use std::collections::BTreeSet;

use sim_kernel::Symbol;

use crate::{
    ComponentCapability, ComponentRegistryCategory, InstrumentWrapperCategory,
    default_audio_synth_registry,
    ps3300::{
        PS3300_FOOTAGES, PS3300_KEY_COUNT, PS3300_MASTER_OSCILLATOR_COUNT, Ps3300AliasingPolicy,
        Ps3300Footage, Ps3300FootageLevels, Ps3300Noise, Ps3300NoiseColor, Ps3300NoiseSettings,
        Ps3300ToneSource, Ps3300ToneSourceSettings, Ps3300ToneWaveform, ps3_noise_component_id,
        ps3_tonegen_component_id, ps3300_pitch_coverage, ps3300_tone_divider_plan,
        ps3300_tone_source_fixture_names, ps3300_tone_source_module_ids,
    },
};

#[test]
fn ps3300_tone_source_records_ids_and_fixture_names() {
    assert_eq!(
        ps3300_tone_source_module_ids()
            .iter()
            .map(Symbol::as_qualified_str)
            .collect::<Vec<_>>(),
        vec![
            "audio-synth/module/ps3-tonegen",
            "audio-synth/module/ps3-noise",
        ]
    );
    assert_eq!(
        ps3300_tone_source_fixture_names(),
        [
            "ps3300-ps3-tonegen-pitch-coverage",
            "ps3300-ps3-tonegen-footage-transposition",
            "ps3300-ps3-tonegen-divider-determinism",
            "ps3300-ps3-tonegen-aliasing-policy",
            "ps3300-ps3-noise-white-colored-bands",
        ]
    );
}

#[test]
fn ps3300_tone_source_registry_entries_are_exact_components() {
    let registry = default_audio_synth_registry();
    for id in [ps3_tonegen_component_id(), ps3_noise_component_id()] {
        let entry = registry.get(&id).expect("PS-3300 source entry");
        assert_eq!(entry.category(), ComponentRegistryCategory::Exact);
        assert_eq!(entry.wrapper(), InstrumentWrapperCategory::FixedPolysynth);
        assert!(entry.is_implemented());
        assert!(entry.has_capability(ComponentCapability::RealtimeSafe));
        assert!(entry.has_capability(ComponentCapability::Traceable));
        assert_eq!(entry.instantiate().unwrap().component_id(), id);
    }
}

#[test]
fn ps3300_pitch_coverage_uses_twelve_masters_and_four_divider_stages() {
    let coverage = ps3300_pitch_coverage();
    assert_eq!(coverage.len(), PS3300_KEY_COUNT);

    let masters = coverage
        .iter()
        .map(|plan| plan.master_index)
        .collect::<BTreeSet<_>>();
    assert_eq!(masters.len(), PS3300_MASTER_OSCILLATOR_COUNT);

    let stages = coverage
        .iter()
        .map(|plan| plan.divider_stage)
        .collect::<BTreeSet<_>>();
    assert_eq!(stages, BTreeSet::from([0, 1, 2, 3]));

    for plan in coverage {
        assert_close(plan.divided_frequency_hz, midi_key_hz(plan.midi_key), 0.001);
    }
}

#[test]
fn ps3300_footages_transpose_by_octaves() {
    let mut tone = Ps3300ToneSource::new(Ps3300ToneSourceSettings {
        waveform: Ps3300ToneWaveform::Sine,
        aliasing_policy: Ps3300AliasingPolicy::ClampToNyquist,
        footage_levels: Ps3300FootageLevels {
            sixteen: 1.0,
            eight: 1.0,
            four: 1.0,
        },
        detune_cents: 0.0,
        level: 1.0,
    });
    tone.set_sample_rate(48_000.0);

    let midi_key = 60;
    let eight = tone.frequency_hz(midi_key, Ps3300Footage::Eight, 0.0);
    let sixteen = tone.frequency_hz(midi_key, Ps3300Footage::Sixteen, 0.0);
    let four = tone.frequency_hz(midi_key, Ps3300Footage::Four, 0.0);

    assert_close(sixteen.output_hz * 2.0, eight.output_hz, 0.001);
    assert_close(four.output_hz, eight.output_hz * 2.0, 0.001);

    for footage in PS3300_FOOTAGES {
        assert!(tone.frequency_hz(midi_key, footage, 0.0).output_hz > 0.0);
    }
}

#[test]
fn ps3300_divider_plan_is_deterministic() {
    let first = (36..84).map(ps3300_tone_divider_plan).collect::<Vec<_>>();
    let second = (36..84).map(ps3300_tone_divider_plan).collect::<Vec<_>>();
    assert_eq!(round_plans(&first), round_plans(&second));
    assert_eq!(first[0].master_index, 0);
    assert_eq!(first[0].divider_stage, 3);
    assert_eq!(first[47].divider_stage, 0);
}

#[test]
fn ps3300_aliasing_policy_is_recorded_and_applied() {
    let mut folded = Ps3300ToneSource::new(Ps3300ToneSourceSettings {
        aliasing_policy: Ps3300AliasingPolicy::Foldback,
        ..Ps3300ToneSourceSettings::default()
    });
    folded.set_sample_rate(2_000.0);

    let mut clamped = Ps3300ToneSource::new(Ps3300ToneSourceSettings {
        aliasing_policy: Ps3300AliasingPolicy::ClampToNyquist,
        ..Ps3300ToneSourceSettings::default()
    });
    clamped.set_sample_rate(2_000.0);

    let fold = folded.frequency_hz(108, Ps3300Footage::Four, 0.0);
    let clamp = clamped.frequency_hz(108, Ps3300Footage::Four, 0.0);
    assert!(fold.aliased);
    assert!(clamp.aliased);
    assert!(fold.output_hz < clamp.output_hz);
    assert_close(clamp.output_hz, 1_000.0, 0.001);
}

#[test]
fn ps3300_noise_is_deterministic_and_colored_band_is_smoother() {
    let settings = Ps3300NoiseSettings {
        color: Ps3300NoiseColor::White,
        level: 1.0,
        seed: 0x1234_5678,
        color_coefficient: 0.08,
    };
    let mut first = Ps3300Noise::new(settings);
    let mut second = Ps3300Noise::new(settings);
    let first_run = (0..64).map(|_| first.next_sample()).collect::<Vec<_>>();
    let second_run = (0..64).map(|_| second.next_sample()).collect::<Vec<_>>();
    assert_eq!(round4(&first_run), round4(&second_run));

    let mut noise = Ps3300Noise::new(settings);
    let frames = (0..1_024).map(|_| noise.next_frame()).collect::<Vec<_>>();
    let white = frames.iter().map(|frame| frame.white).collect::<Vec<_>>();
    let colored = frames.iter().map(|frame| frame.colored).collect::<Vec<_>>();
    assert!(white.iter().all(|sample| sample.abs() <= 1.0001));
    assert!(colored.iter().all(|sample| sample.abs() <= 1.0001));
    assert!(difference_energy(&colored) < difference_energy(&white) * 0.45);

    let mut selected = Ps3300Noise::new(Ps3300NoiseSettings {
        color: Ps3300NoiseColor::Colored,
        ..settings
    });
    let frame = selected.next_frame();
    assert_eq!(round4(&[frame.selected]), round4(&[frame.colored]));
}

fn midi_key_hz(midi_key: u8) -> f32 {
    440.0 * 2.0_f32.powf((f32::from(midi_key) - 69.0) / 12.0)
}

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {actual} within {tolerance} of {expected}"
    );
}

fn round_plans(plans: &[crate::ps3300::Ps3300DividerPlan]) -> Vec<(u8, u8, u8, i32, i32)> {
    plans
        .iter()
        .map(|plan| {
            (
                plan.midi_key,
                plan.master_index,
                plan.divider_stage,
                (plan.master_frequency_hz * 1_000.0).round() as i32,
                (plan.divided_frequency_hz * 1_000.0).round() as i32,
            )
        })
        .collect()
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

fn round4(samples: &[f32]) -> Vec<i32> {
    samples
        .iter()
        .map(|sample| (sample * 10_000.0).round() as i32)
        .collect()
}
