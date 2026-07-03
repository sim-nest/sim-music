use std::f32::consts::TAU;

use sim_kernel::Symbol;

use crate::{
    ComponentCapability, ComponentRegistryCategory, DiscreteComponent,
    SYSTEM55_FIXED_FILTER_BANK_CENTERS_HZ, System55Attenuator, System55AttenuatorSettings,
    System55Envelope, System55EnvelopeFollower, System55EnvelopeFollowerSettings,
    System55EnvelopeSettings, System55EnvelopeStage, System55FixedFilterBank,
    System55FixedFilterBankSettings, System55FrequencyShifter, System55FrequencyShifterSettings,
    System55Interface, System55Keyboard, System55KeyboardSettings, System55Mixer,
    System55MixerSettings, System55Multiple, System55MultipleSettings, System55Ribbon,
    System55RibbonSettings, System55RingModulator, System55RingModulatorSettings,
    System55SampleHold, System55SampleHoldSettings, System55Sequencer, System55SequencerSettings,
    System55TriggerDelay, System55TriggerDelaySettings, System55Vca, System55VcaResponse,
    System55VcaSettings, default_audio_synth_registry, m55_attenuator_component_id,
    m55_env_follower_component_id, m55_envelope_component_id, m55_fixed_filter_bank_component_id,
    m55_frequency_shifter_component_id, m55_interface_component_id, m55_keyboard_component_id,
    m55_mixer_component_id, m55_multiple_component_id, m55_ribbon_component_id,
    m55_ring_component_id, m55_sample_hold_component_id, m55_sequencer_component_id,
    m55_trigger_delay_component_id, m55_vca_component_id, system55_control_module_ids,
};

const SAMPLE_RATE: f32 = 1_000.0;

#[test]
fn system55_control_module_ids_are_recorded() {
    assert_eq!(
        system55_control_module_ids()
            .iter()
            .map(Symbol::as_qualified_str)
            .collect::<Vec<_>>(),
        vec![
            "audio-synth/module/m55-902-vca",
            "audio-synth/module/m55-911-envelope-generator",
            "audio-synth/module/m55-911a-dual-trigger-delay",
            "audio-synth/module/m55-912-envelope-follower",
            "audio-synth/module/m55-907-fixed-filter-bank",
            "audio-synth/module/m55-1630-frequency-shifter",
            "audio-synth/module/m55-ring-modulator",
            "audio-synth/module/m55-cp3a-mixer",
            "audio-synth/module/m55-multiple",
            "audio-synth/module/m55-attenuator",
            "audio-synth/module/m55-928-sample-hold",
            "audio-synth/module/m55-960-sequential-controller",
            "audio-synth/module/m55-961-interface",
            "audio-synth/module/m55-956-ribbon-controller",
            "audio-synth/module/m55-951-keyboard-controller",
        ]
    );
}

#[test]
fn system55_control_registry_entries_are_exact_components() {
    let registry = default_audio_synth_registry();
    for id in system55_control_module_ids() {
        let entry = registry.get(&id).expect("System 55 control module entry");
        assert_eq!(entry.category(), ComponentRegistryCategory::Exact);
        assert!(entry.is_implemented());
        assert!(entry.has_capability(ComponentCapability::RealtimeSafe));
        assert!(entry.has_capability(ComponentCapability::Traceable));
        assert_eq!(entry.instantiate().unwrap().component_id(), id);
    }
}

#[test]
fn m55_vca_tracks_gain_law_and_saturation() {
    let linear = System55Vca::new(System55VcaSettings {
        response: System55VcaResponse::Linear,
        gain: 2.0,
        saturation_drive: 2.0,
    });
    assert_close(linear.gain_for_cv(0.5), 1.0, 0.0001);

    let mut exponential = System55Vca::new(System55VcaSettings {
        response: System55VcaResponse::Exponential,
        gain: 2.0,
        saturation_drive: 2.0,
    });
    assert_close(exponential.gain_for_cv(0.5), 0.5, 0.0001);
    assert_close(exponential.next_sample(2.0, 0.5), 1.0, 0.0001);

    let mut saturated = System55Vca::new(System55VcaSettings {
        response: System55VcaResponse::Saturated,
        gain: 4.0,
        saturation_drive: 8.0,
    });
    assert!(saturated.next_sample(10.0, 1.0).abs() <= 1.0001);
}

#[test]
fn m55_envelope_and_trigger_delay_use_s_trigger_timing() {
    let mut envelope = System55Envelope::new(System55EnvelopeSettings {
        attack_s: 0.01,
        decay_s: 0.01,
        sustain_level: 0.5,
        release_s: 0.01,
        level: 1.0,
    });
    envelope.set_sample_rate(100.0);
    assert_eq!(envelope.stage(), System55EnvelopeStage::Idle);
    assert_close(envelope.next_sample(5.0), 0.0, 0.0001);
    assert_close(envelope.next_sample(0.0), 1.0, 0.0001);
    assert_eq!(envelope.stage(), System55EnvelopeStage::Decay);
    assert_close(envelope.next_sample(0.0), 0.5, 0.0001);
    assert_eq!(envelope.stage(), System55EnvelopeStage::Sustain);
    assert_close(envelope.next_sample(5.0), 0.0, 0.0001);
    assert_eq!(envelope.stage(), System55EnvelopeStage::Idle);

    let mut delay = System55TriggerDelay::new(System55TriggerDelaySettings {
        delay_s: 0.02,
        pulse_s: 0.02,
    });
    delay.set_sample_rate(100.0);
    assert!(!delay.next_frame(5.0).active);
    assert!(!delay.next_frame(0.0).active);
    assert!(delay.next_frame(0.0).active);
    assert!(delay.next_frame(0.0).active);
    assert!(!delay.next_frame(0.0).active);
}

#[test]
fn m55_envelope_follower_tracks_level_and_gate() {
    let mut follower = System55EnvelopeFollower::new(System55EnvelopeFollowerSettings {
        attack_s: 0.0,
        release_s: 0.05,
        gate_threshold: 0.4,
    });
    follower.set_sample_rate(SAMPLE_RATE);
    let first = follower.next_frame(0.0);
    assert!(!first.gate);
    let hit = follower.next_frame(1.0);
    assert!(hit.envelope >= 1.0);
    assert!(hit.gate);
    assert!(hit.trigger);
    let held = follower.next_frame(0.2);
    assert!(held.envelope > 0.4);
    assert!(held.gate);
    assert!(!held.trigger);
}

#[test]
fn m55_fixed_filter_bank_records_centers() {
    let mut bank = System55FixedFilterBank::new(System55FixedFilterBankSettings::default());
    bank.set_sample_rate(16_000.0);
    assert_eq!(
        bank.band_centers_hz(),
        SYSTEM55_FIXED_FILTER_BANK_CENTERS_HZ
    );
    let frame = bank.next_frame(1.0);
    assert_eq!(
        frame.bands.len(),
        SYSTEM55_FIXED_FILTER_BANK_CENTERS_HZ.len()
    );
    assert!(frame.output.abs() > 0.0);
}

#[test]
fn m55_frequency_shifter_and_ring_modulator_create_sidebands() {
    let mut shifter = System55FrequencyShifter::new(System55FrequencyShifterSettings {
        shift_hz: 100.0,
        level: 1.0,
    });
    shifter.set_sample_rate(SAMPLE_RATE);
    let mut upper = Vec::new();
    let mut lower = Vec::new();
    for frame in 0..1_000 {
        let phase = TAU * 200.0 * frame as f32 / SAMPLE_RATE;
        let shifted = shifter.next_frame(phase.sin(), -phase.cos());
        upper.push(shifted.upper_sideband);
        lower.push(shifted.lower_sideband);
    }
    assert!(tone_energy(&upper, 300.0) > tone_energy(&upper, 100.0) * 8.0);
    assert!(tone_energy(&lower, 100.0) > tone_energy(&lower, 300.0) * 8.0);

    let mut ring = System55RingModulator::new(System55RingModulatorSettings { level: 1.0 });
    let ringed = (0..1_000)
        .map(|frame| {
            let carrier = (TAU * 200.0 * frame as f32 / SAMPLE_RATE).sin();
            let modulator = (TAU * 100.0 * frame as f32 / SAMPLE_RATE).sin();
            ring.next_sample(carrier, modulator)
        })
        .collect::<Vec<_>>();
    assert!(tone_energy(&ringed, 100.0) > 0.2);
    assert!(tone_energy(&ringed, 300.0) > 0.2);
    assert!(tone_energy(&ringed, 200.0) < 0.05);
}

#[test]
fn m55_mixer_multiple_and_attenuator_handle_utility_signals() {
    let mixer = System55Mixer::new(System55MixerSettings {
        gains: [0.5, 0.5, 0.5, 0.5],
        output_gain: 1.0,
        drive: 0.1,
    });
    let mixed = mixer.mix([0.25, 0.25, 0.25, 0.25]);
    assert!(mixed > 0.49 && mixed < 0.51);

    let mut driven = System55Mixer::new(System55MixerSettings {
        gains: [2.0; 4],
        output_gain: 2.0,
        drive: 8.0,
    });
    assert!(driven.next_sample([4.0; 4]).abs() <= 4.0001);

    let mut multiple = System55Multiple::new(System55MultipleSettings { output_count: 3 });
    assert_eq!(multiple.next_frame(2.5).outputs, [2.5, 2.5, 2.5, 0.0]);

    let mut attenuator = System55Attenuator::new(System55AttenuatorSettings {
        gain: 0.5,
        offset: 1.0,
    });
    assert_close(attenuator.next_sample(4.0), 3.0, 0.0001);
}

#[test]
fn m55_sample_hold_and_sequencer_advance_on_s_trigger() {
    let mut sample_hold = System55SampleHold::new(System55SampleHoldSettings::default());
    assert_close(sample_hold.next_sample(1.0, 5.0), 0.0, 0.0001);
    assert_close(sample_hold.next_sample(2.0, 0.0), 2.0, 0.0001);
    assert_close(sample_hold.next_sample(3.0, 0.0), 2.0, 0.0001);
    assert_close(sample_hold.next_sample(4.0, 5.0), 2.0, 0.0001);
    assert_close(sample_hold.next_sample(5.0, 0.0), 5.0, 0.0001);

    let mut sequencer = System55Sequencer::new(System55SequencerSettings {
        steps: [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7],
        step_count: 4,
        gate_mask: 0b0101,
    });
    assert_eq!(sequencer.next_frame(5.0, 5.0).step, 0);
    let first = sequencer.next_frame(0.0, 5.0);
    assert_eq!(first.step, 1);
    assert_close(first.cv, 0.1, 0.0001);
    assert!(!first.gate);
    assert_eq!(sequencer.next_frame(0.0, 5.0).step, 1);
    assert_eq!(sequencer.next_frame(5.0, 5.0).step, 1);
    let second = sequencer.next_frame(0.0, 5.0);
    assert_eq!(second.step, 2);
    assert!(second.gate);
    assert_eq!(sequencer.next_frame(5.0, 0.0).step, 0);
}

#[test]
fn m55_ribbon_keyboard_and_interface_map_control_signals() {
    let mut ribbon = System55Ribbon::new(System55RibbonSettings {
        range_octaves: 4.0,
        center_cv_v: 0.0,
    });
    let left = ribbon.next_frame(0.0, 0.0);
    assert_close(left.pitch_cv, -2.0, 0.0001);
    assert!(!left.gate);
    let right = ribbon.next_frame(1.0, 0.5);
    assert_close(right.pitch_cv, 2.0, 0.0001);
    assert_close(right.pressure_cv, 2.5, 0.0001);
    assert!(right.gate);
    assert!(right.trigger);

    let mut keyboard = System55Keyboard::new(System55KeyboardSettings {
        reference_key: 60,
        bend_depth_octaves: 1.0,
    });
    let note = keyboard.next_frame(Some(72), true, 0.0);
    assert_close(note.pitch_cv, 1.0, 0.0001);
    assert_close(note.s_trigger_v, 0.0, 0.0001);
    assert!(note.trigger);
    assert!(!keyboard.next_frame(Some(72), true, 0.0).trigger);
    assert!(keyboard.next_frame(Some(74), true, 0.0).trigger);
    assert_close(
        keyboard.next_frame(Some(74), false, 0.0).s_trigger_v,
        5.0,
        0.0001,
    );

    let mut interface = System55Interface::new();
    let (voltage_gate, s_trigger, triggered) = interface.next_frame(0.0, 0.0);
    assert_close(voltage_gate, 5.0, 0.0001);
    assert_close(s_trigger, 5.0, 0.0001);
    assert!(triggered);
    let (_, s_trigger_from_gate, _) = interface.next_frame(5.0, 5.0);
    assert_close(s_trigger_from_gate, 0.0, 0.0001);
}

#[test]
fn m55_control_modules_implement_discrete_component() {
    fn assert_component<T: DiscreteComponent>() {}
    assert_component::<System55Vca>();
    assert_component::<System55Envelope>();
    assert_component::<System55TriggerDelay>();
    assert_component::<System55EnvelopeFollower>();
    assert_component::<System55FixedFilterBank>();
    assert_component::<System55FrequencyShifter>();
    assert_component::<System55RingModulator>();
    assert_component::<System55Mixer>();
    assert_component::<System55Multiple>();
    assert_component::<System55Attenuator>();
    assert_component::<System55SampleHold>();
    assert_component::<System55Sequencer>();
    assert_component::<System55Interface>();
    assert_component::<System55Ribbon>();
    assert_component::<System55Keyboard>();
    assert_eq!(
        [
            m55_vca_component_id(),
            m55_envelope_component_id(),
            m55_trigger_delay_component_id(),
            m55_env_follower_component_id(),
            m55_fixed_filter_bank_component_id(),
            m55_frequency_shifter_component_id(),
            m55_ring_component_id(),
            m55_mixer_component_id(),
            m55_multiple_component_id(),
            m55_attenuator_component_id(),
            m55_sample_hold_component_id(),
            m55_sequencer_component_id(),
            m55_interface_component_id(),
            m55_ribbon_component_id(),
            m55_keyboard_component_id(),
        ],
        system55_control_module_ids()
    );
}

fn tone_energy(samples: &[f32], frequency_hz: f32) -> f32 {
    let mut re = 0.0;
    let mut im = 0.0;
    for (frame, sample) in samples.iter().enumerate() {
        let phase = TAU * frequency_hz * frame as f32 / SAMPLE_RATE;
        re += sample * phase.cos();
        im -= sample * phase.sin();
    }
    (re.mul_add(re, im * im)).sqrt() / samples.len().max(1) as f32
}

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {actual} within {tolerance} of {expected}"
    );
}
