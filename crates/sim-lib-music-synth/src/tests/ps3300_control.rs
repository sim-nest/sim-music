use crate::{
    ComponentCapability, ComponentRegistryCategory, DiscreteComponent, InstrumentWrapperCategory,
    default_audio_synth_registry,
    modules::{
        ps3_keyboard::{
            Ps3300KeyboardController, Ps3300KeyboardSettings, ps3_keyboard_component_id,
            ps3300_keyboard_gate_mapping,
        },
        ps3_matrix::{
            Ps3300PinMatrix, Ps3300PinMatrixInputs, ps3_pin_matrix_component_id,
            ps3300_pin_matrix_format,
        },
        ps3_modulation::{
            Ps3300ExternalProcessor, Ps3300ExternalProcessorSettings, Ps3300ModulationGenerator,
            Ps3300ModulationGeneratorSettings, Ps3300ModulationWaveform, Ps3300SampleHold,
            Ps3300SampleHoldSettings, ps3_external_processor_component_id,
            ps3_modulation_generator_component_id, ps3_sample_hold_component_id,
            ps3300_modulation_fixture_names, ps3300_modulation_module_ids,
        },
        ps3_section::{
            Ps3300SectionGenerator, Ps3300SectionGeneratorSettings, Ps3300ThreeSectionSummer,
            Ps3300ThreeSectionSummerSettings, ps3_output_mixer_component_id,
            ps3_section_generator_component_id, ps3300_section_fixture_names,
            ps3300_section_module_ids,
        },
    },
    ps3300::{
        PS3300_KEY_COUNT, Ps3300PinMatrixRoute, Ps3300PolyArraySettings, Ps3300Section,
        ps3300_validate_pin_matrix_routes,
    },
};

#[test]
fn ps3300_control_ids_matrix_format_and_keyboard_mapping_are_recorded() {
    let ids = ps3300_modulation_module_ids()
        .into_iter()
        .chain(ps3300_section_module_ids())
        .chain([ps3_keyboard_component_id(), ps3_pin_matrix_component_id()])
        .map(|id| id.as_qualified_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            "audio-synth/module/ps3-modulation-generator",
            "audio-synth/module/ps3-sample-hold",
            "audio-synth/module/ps3-external-processor",
            "audio-synth/module/ps3-section-generator",
            "audio-synth/module/ps3-output-mixer",
            "audio-synth/module/ps3-keyboard-controller",
            "audio-synth/module/ps3-pin-matrix",
        ]
    );
    assert_eq!(
        ps3300_modulation_fixture_names(),
        [
            "ps3300-ps3-modulation-generator-shapes",
            "ps3300-ps3-sample-hold-edge-capture",
            "ps3300-ps3-external-processor-tracking",
        ]
    );
    assert_eq!(
        ps3300_section_fixture_names(),
        [
            "ps3300-ps3-section-chord-render",
            "ps3300-ps3-three-section-summer-stack",
        ]
    );

    let format = ps3300_pin_matrix_format();
    assert!(format.sources.contains(&"modulation-cv"));
    assert!(format.targets.contains(&"resonator-formant-cv"));
    assert!(
        format
            .legal_pairs
            .contains(&("keyboard-pitch-cv", "section-a-pitch-cv"))
    );

    let mapping = ps3300_keyboard_gate_mapping();
    assert_eq!(mapping.first_midi_key, 36);
    assert_eq!(mapping.key_count, PS3300_KEY_COUNT);
    assert_eq!(mapping.gate_bus_width, PS3300_KEY_COUNT);
}

#[test]
fn ps3300_control_registry_entries_are_exact_components() {
    let registry = default_audio_synth_registry();
    for id in [
        ps3_modulation_generator_component_id(),
        ps3_sample_hold_component_id(),
        ps3_external_processor_component_id(),
        ps3_keyboard_component_id(),
        ps3_pin_matrix_component_id(),
        ps3_section_generator_component_id(),
        ps3_output_mixer_component_id(),
    ] {
        let entry = registry.get(&id).expect("PS-3300 control entry");
        assert_eq!(entry.category(), ComponentRegistryCategory::Exact);
        assert_eq!(entry.wrapper(), InstrumentWrapperCategory::FixedPolysynth);
        assert!(entry.is_implemented());
        assert!(entry.has_capability(ComponentCapability::RealtimeSafe));
        assert!(entry.has_capability(ComponentCapability::Traceable));
        assert_eq!(entry.instantiate().unwrap().component_id(), id);
    }
}

#[test]
fn ps3300_modulation_generator_shapes_are_deterministic() {
    let mut samples = Vec::new();
    for waveform in [
        Ps3300ModulationWaveform::Sine,
        Ps3300ModulationWaveform::Triangle,
        Ps3300ModulationWaveform::Saw,
        Ps3300ModulationWaveform::Square,
    ] {
        let mut generator = Ps3300ModulationGenerator::new(Ps3300ModulationGeneratorSettings {
            waveform,
            rate_hz: 1.0,
            depth: 1.0,
            offset: 0.0,
            rate_cv_depth_octaves: 0.0,
        });
        generator.set_sample_rate(4.0);
        samples.push(round3(generator.next_sample(0.0).bipolar));
        samples.push(round3(generator.next_sample(0.0).bipolar));
    }
    assert_eq!(samples, vec![0.0, 1.0, -1.0, 0.0, -1.0, -0.5, 1.0, 1.0]);
}

#[test]
fn ps3300_sample_hold_captures_only_on_rising_edge() {
    let mut sample_hold = Ps3300SampleHold::new(Ps3300SampleHoldSettings {
        initial_value: 0.0,
        trigger_threshold_v: 0.5,
    });

    assert_eq!(sample_hold.next_sample(1.0, 0.0).held, 0.0);
    assert_eq!(sample_hold.next_sample(1.25, 1.0).held, 1.25);
    assert_eq!(sample_hold.next_sample(2.0, 1.0).held, 1.25);
    assert_eq!(sample_hold.next_sample(3.0, 0.0).held, 1.25);
    assert_eq!(sample_hold.next_sample(-2.0, 1.0).held, -2.0);
}

#[test]
fn ps3300_external_processor_tracks_audio_cv_and_gate() {
    let mut external = Ps3300ExternalProcessor::new(Ps3300ExternalProcessorSettings {
        audio_gain: 2.0,
        cv_gain: 1.5,
        cv_bias_v: 0.25,
        gate_threshold_v: 2.0,
        follower_smoothing: 1.0,
    });
    let frame = external.next_sample(0.75, 0.5);
    assert_eq!(round3(frame.audio), 1.5);
    assert_eq!(round3(frame.follower), 1.5);
    assert_eq!(round3(frame.cv), 2.5);
    assert!(frame.gate);
}

#[test]
fn ps3300_section_chord_render_and_three_section_stack_are_bounded() {
    let mut section = Ps3300SectionGenerator::new(Ps3300SectionGeneratorSettings {
        section: Ps3300Section::B,
        level: 1.0,
        poly: Ps3300PolyArraySettings {
            section_level: 1.0,
            first_midi_key: 36,
            key_count: PS3300_KEY_COUNT,
        },
        ..Ps3300SectionGeneratorSettings::default()
    });
    section.set_sample_rate(1_000.0);
    let chord = [48, 52, 55];
    let mut frame = section.next_chord(&chord, 0.0, true, 0.0);
    for _ in 0..24 {
        frame = section.next_chord(&chord, 0.0, true, 0.0);
    }
    assert_eq!(frame.active_count, chord.len());
    assert!(frame.output.abs() > 0.0);

    let mut summer = Ps3300ThreeSectionSummer::new(Ps3300ThreeSectionSummerSettings {
        section_gains: [0.5, 0.5, 0.5],
        resonator_gain: 1.0,
        output_gain: 1.0,
    });
    let mixed = summer.next_sample([0.2, 0.3, 0.4], 0.1);
    assert_eq!(round3(mixed.dry_sum), 0.45);
    assert_eq!(round3(mixed.output), 0.55);
}

#[test]
fn ps3300_pin_matrix_rejects_illegal_pairs_and_routes_legal_pairs() {
    let err = ps3300_validate_pin_matrix_routes(&[Ps3300PinMatrixRoute::new(
        "keyboard-gate",
        "resonator-audio-in",
        1.0,
    )])
    .expect_err("gate to audio should fail");
    assert!(format!("{err}").contains("illegal PS-3300 pin route"));

    let routes = vec![
        Ps3300PinMatrixRoute::new("keyboard-pitch-cv", "section-a-pitch-cv", 1.0),
        Ps3300PinMatrixRoute::new("keyboard-gate", "section-a-gate", 1.0),
        Ps3300PinMatrixRoute::new("modulation-cv", "resonator-formant-cv", 0.5),
        Ps3300PinMatrixRoute::new("section-a-audio", "resonator-audio-in", 0.75),
    ];
    let mut matrix = Ps3300PinMatrix::new(routes).expect("legal routes");
    let frame = matrix.route(Ps3300PinMatrixInputs {
        keyboard_pitch_cv: 0.25,
        keyboard_gate: true,
        modulation_cv: 0.4,
        section_audio: [0.8, 0.0, 0.0],
        ..Ps3300PinMatrixInputs::default()
    });
    assert_eq!(round3(frame.section_pitch_cv[0]), 0.25);
    assert!(frame.section_gate[0]);
    assert_eq!(round3(frame.resonator_formant_cv), 0.2);
    assert_eq!(round3(frame.resonator_audio), 0.6);
}

#[test]
fn ps3300_keyboard_maps_per_key_gate_timing() {
    let mut keyboard = Ps3300KeyboardController::new(Ps3300KeyboardSettings {
        first_midi_key: 36,
        key_count: PS3300_KEY_COUNT,
        gate_voltage: 1.0,
    });

    let first = keyboard.next_key(Some(48), true);
    assert!(first.gate);
    assert!(first.trigger);
    assert!(keyboard.gate_for(48));
    assert_eq!(round3(first.pitch_cv), 1.0);

    assert!(!keyboard.next_key(Some(48), true).trigger);
    let switched = keyboard.next_key(Some(52), true);
    assert!(switched.trigger);
    assert!(!keyboard.gate_for(48));
    assert!(keyboard.gate_for(52));

    let released = keyboard.next_key(Some(52), false);
    assert!(!released.gate);
    assert!(!released.trigger);
    assert!(!keyboard.gate_for(52));
}

#[test]
fn ps3300_control_modules_implement_discrete_component() {
    fn assert_component<T: DiscreteComponent>() {}
    assert_component::<Ps3300ModulationGenerator>();
    assert_component::<Ps3300SampleHold>();
    assert_component::<Ps3300ExternalProcessor>();
    assert_component::<Ps3300KeyboardController>();
    assert_component::<Ps3300PinMatrix>();
    assert_component::<Ps3300SectionGenerator>();
    assert_component::<Ps3300ThreeSectionSummer>();

    let ids = [
        ps3_modulation_generator_component_id(),
        ps3_sample_hold_component_id(),
        ps3_external_processor_component_id(),
        ps3_keyboard_component_id(),
        ps3_pin_matrix_component_id(),
        ps3_section_generator_component_id(),
        ps3_output_mixer_component_id(),
    ];
    let symbols = ids
        .into_iter()
        .map(|id| id.as_qualified_str())
        .collect::<Vec<_>>();
    assert!(
        symbols
            .iter()
            .all(|id| id.starts_with("audio-synth/module/ps3-"))
    );
}

fn round3(value: f32) -> f32 {
    (value * 1_000.0).round() / 1_000.0
}
