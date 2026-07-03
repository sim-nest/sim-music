use sim_kernel::Symbol;

use crate::{
    ComponentCapability, ComponentRegistryCategory, DiscreteComponent, InstrumentWrapperCategory,
    default_audio_synth_registry,
    system700::{
        System700Clock, System700ClockSettings, System700Envelope, System700EnvelopeSettings,
        System700ExternalInput, System700ExternalInputSettings, System700Keyboard,
        System700KeyboardSettings, System700Mixer, System700MixerSettings, System700Multiple,
        System700MultipleSettings, System700SampleHold, System700SampleHoldSettings,
        System700Sequencer, System700SequencerSettings, System700VoltageProcessor,
        System700VoltageProcessorSettings, r700_clock_component_id, r700_envelope_component_id,
        r700_external_input_component_id, r700_keyboard_component_id, r700_mixer_component_id,
        r700_multiple_component_id, r700_sample_hold_component_id, r700_sequencer_component_id,
        r700_voltage_processor_component_id, system700_control_fixture_names,
        system700_control_module_ids,
    },
};

#[test]
fn system700_control_ids_and_fixtures_are_recorded() {
    assert_eq!(
        system700_control_module_ids()
            .iter()
            .map(Symbol::as_qualified_str)
            .collect::<Vec<_>>(),
        vec![
            "audio-synth/module/r700-envelope",
            "audio-synth/module/r700-sample-hold",
            "audio-synth/module/r700-voltage-processor",
            "audio-synth/module/r700-mixer",
            "audio-synth/module/r700-multiple",
            "audio-synth/module/r700-external-input",
            "audio-synth/module/r700-keyboard",
            "audio-synth/module/r700-clock",
            "audio-synth/module/r700-sequencer",
        ]
    );
    assert_eq!(
        system700_control_fixture_names(),
        [
            "system700-r700-envelope-timing",
            "system700-r700-sample-hold-capture",
            "system700-r700-voltage-processor-transfer",
            "system700-r700-mixer-multiple",
            "system700-r700-external-input-map",
            "system700-r700-keyboard-cv-gate",
            "system700-r700-clock-pulses",
            "system700-r700-sequencer-steps",
        ]
    );
}

#[test]
fn system700_control_registry_entries_are_implemented() {
    let registry = default_audio_synth_registry();
    for id in system700_control_module_ids() {
        let entry = registry.get(&id).expect("control module entry");
        assert_eq!(entry.category(), ComponentRegistryCategory::Exact);
        assert_eq!(entry.wrapper(), InstrumentWrapperCategory::ModularAnalog);
        assert!(entry.is_implemented());
        assert!(entry.has_capability(ComponentCapability::RealtimeSafe));
        assert!(entry.has_capability(ComponentCapability::Traceable));
        assert_eq!(entry.instantiate().unwrap().component_id(), id);
    }
}

#[test]
fn r700_envelope_runs_timed_adsr_segments() {
    let mut envelope = System700Envelope::new(System700EnvelopeSettings {
        attack_s: 0.2,
        decay_s: 0.2,
        sustain_level: 0.5,
        release_s: 0.2,
        level: 1.0,
    });
    envelope.set_sample_rate(10.0);

    assert_eq!(round3(envelope.next_sample(true)), 0.5);
    assert_eq!(round3(envelope.next_sample(true)), 1.0);
    assert_eq!(round3(envelope.next_sample(true)), 0.5);
    assert_eq!(round3(envelope.next_sample(true)), 0.5);
    assert_eq!(round3(envelope.next_sample(false)), 0.0);
}

#[test]
fn r700_sample_hold_captures_only_on_rising_trigger() {
    let mut sample_hold = System700SampleHold::new(System700SampleHoldSettings {
        initial_value: 0.0,
        trigger_threshold: 0.5,
    });

    assert_eq!(sample_hold.next_sample(1.0, 0.0), 0.0);
    assert_eq!(sample_hold.next_sample(1.25, 1.0), 1.25);
    assert_eq!(sample_hold.next_sample(2.0, 1.0), 1.25);
    assert_eq!(sample_hold.next_sample(3.0, 0.0), 1.25);
    assert_eq!(sample_hold.next_sample(-2.0, 1.0), -2.0);
}

#[test]
fn r700_voltage_processor_mixer_and_multiple_map_values() {
    let processor = System700VoltageProcessor::new(System700VoltageProcessorSettings {
        gain: 2.0,
        offset_v: 1.0,
        invert: true,
    });
    assert_eq!(round3(processor.transfer(2.0)), -3.0);

    let mixer = System700Mixer::new(System700MixerSettings {
        gains: [0.5, 1.0, 1.5, 0.0],
        output_gain: 0.5,
    });
    assert_eq!(round3(mixer.mix([1.0, 1.0, 1.0, 1.0])), 1.5);

    let multiple = System700Multiple::new(System700MultipleSettings { output_count: 4 });
    assert_eq!(multiple.fanout(2.25), [2.25; 4]);
}

#[test]
fn r700_external_input_and_keyboard_map_interface_signals() {
    let external = System700ExternalInput::new(System700ExternalInputSettings {
        gain: 2.0,
        cv_bias_v: 1.0,
        gate_threshold_v: 3.0,
    });
    let mapped = external.map_input(1.25);
    assert_eq!(round3(mapped.audio), 2.5);
    assert_eq!(round3(mapped.cv), 3.5);
    assert!(mapped.gate);

    let mut keyboard = System700Keyboard::new(System700KeyboardSettings {
        reference_key: 60,
        bend_depth_octaves: 1.0,
    });
    let first = keyboard.next_frame(Some(60), true, 0.0);
    assert_eq!(round3(first.pitch_cv), 0.0);
    assert!(first.gate);
    assert!(first.trigger);
    assert!(!keyboard.next_frame(Some(60), true, 0.0).trigger);
    let octave = keyboard.next_frame(Some(72), true, 0.0);
    assert_eq!(round3(octave.pitch_cv), 1.0);
    assert!(octave.trigger);
    assert!(!keyboard.next_frame(Some(72), false, 0.0).gate);
}

#[test]
fn r700_clock_and_sequencer_advance_deterministically() {
    let mut clock = System700Clock::new(System700ClockSettings {
        rate_hz: 2.0,
        pulse_width: 0.5,
    });
    clock.set_sample_rate(10.0);
    let frames = (0..6).map(|_| clock.next_frame(true)).collect::<Vec<_>>();
    assert!(frames[0].gate);
    assert!(frames[0].trigger);
    assert!(frames[1].gate);
    assert!(!frames[1].trigger);
    assert!(frames[2].gate);
    assert!(!frames[3].gate);
    assert!(frames[5].gate);
    assert!(frames[5].trigger);

    let mut sequencer = System700Sequencer::new(System700SequencerSettings {
        steps: [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0],
        step_count: 4,
        gate_mask: 0b1010,
    });
    assert_eq!(sequencer.next_frame(false, false).step, 0);
    let one = sequencer.next_frame(true, false);
    assert_eq!(one.step, 1);
    assert_eq!(one.cv, 1.0);
    assert!(one.gate);
    assert!(one.trigger);
    assert!(!sequencer.next_frame(true, false).trigger);
    sequencer.next_frame(false, false);
    let two = sequencer.next_frame(true, false);
    assert_eq!(two.step, 2);
    assert_eq!(two.cv, 2.0);
    assert!(!two.gate);
    assert_eq!(sequencer.next_frame(false, true).step, 0);
}

#[test]
fn system700_control_modules_implement_discrete_component() {
    fn assert_component<T: DiscreteComponent>() {}
    assert_component::<System700Envelope>();
    assert_component::<System700SampleHold>();
    assert_component::<System700VoltageProcessor>();
    assert_component::<System700Mixer>();
    assert_component::<System700Multiple>();
    assert_component::<System700ExternalInput>();
    assert_component::<System700Keyboard>();
    assert_component::<System700Clock>();
    assert_component::<System700Sequencer>();

    let ids = [
        r700_envelope_component_id(),
        r700_sample_hold_component_id(),
        r700_voltage_processor_component_id(),
        r700_mixer_component_id(),
        r700_multiple_component_id(),
        r700_external_input_component_id(),
        r700_keyboard_component_id(),
        r700_clock_component_id(),
        r700_sequencer_component_id(),
    ];
    assert_eq!(ids, system700_control_module_ids());
}

fn round3(value: f32) -> f32 {
    (value * 1_000.0).round() / 1_000.0
}
