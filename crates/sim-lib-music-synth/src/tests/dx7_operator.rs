use crate::{
    ComponentCapability, ComponentRegistryCategory, ComponentTraceValue, DiscreteComponent,
    Dx7Envelope, Dx7FmOperator, Dx7FmOperatorSettings, Dx7FrequencyMode, Dx7Lfo, Dx7OperatorInput,
    Dx7Patch, Dx7PatchOperator, Dx7PitchSettings, Dx7RawPatch, QLevel,
    default_audio_synth_registry, dx7_operator_component_id, dx7_operator_trace_fixture_names,
};

#[test]
fn dx7_operator_single_op_integer_trace_matches_fixture() {
    let mut operator = Dx7FmOperator::new(test_settings(0));
    operator.prepare(crate::ComponentPrepareConfig::new(8, 8, 1, 1));

    let output = render_raw(&mut operator, &[QLevel::ZERO; 8]);
    assert_eq!(
        output,
        [
            0,
            759_250_112,
            1_073_741_824,
            759_250_112,
            -94,
            -759_250_240,
            -1_073_741_824,
            -759_249_856,
        ]
    );
    assert_trace_output(&operator, output[7]);
}

#[test]
fn dx7_operator_two_op_modulation_integer_trace_matches_fixture() {
    let mut modulator = Dx7FmOperator::new(test_settings(0));
    let mut carrier = Dx7FmOperator::new(test_settings(0));
    modulator.prepare(crate::ComponentPrepareConfig::new(8, 8, 1, 1));
    carrier.prepare(crate::ComponentPrepareConfig::new(8, 8, 1, 1));

    let mut output = Vec::new();
    for _ in 0..8 {
        let modulation = modulator
            .next_sample(Dx7OperatorInput {
                gate: true,
                ..Dx7OperatorInput::default()
            })
            .sample;
        output.push(
            carrier
                .next_sample(Dx7OperatorInput {
                    gate: true,
                    modulation,
                    ..Dx7OperatorInput::default()
                })
                .sample
                .raw(),
        );
    }

    assert_eq!(
        output,
        vec![
            0,
            981_629_312,
            759_250_112,
            222_379_104,
            -94,
            -222_379_088,
            -759_250_240,
            -981_629_184,
        ]
    );
    assert_trace_output(&carrier, output[7]);
}

#[test]
fn dx7_operator_feedback_integer_trace_matches_fixture() {
    let mut operator = Dx7FmOperator::new(test_settings(7));
    operator.prepare(crate::ComponentPrepareConfig::new(8, 8, 1, 1));

    let output = render_raw(&mut operator, &[QLevel::ZERO; 8]);
    assert_eq!(
        output,
        [
            0,
            759_250_112,
            851_362_560,
            157_245_808,
            -111_189_776,
            -680_627_008,
            -874_390_784,
            -1_015_353_280,
        ]
    );
    assert_trace_output(&operator, output[7]);
}

#[test]
fn dx7_operator_registry_entry_is_implemented_and_traceable() {
    assert_eq!(
        dx7_operator_trace_fixture_names(),
        [
            "dx7-operator-single-op-i32",
            "dx7-operator-two-op-modulation-i32",
            "dx7-operator-feedback-i32",
        ]
    );

    let registry = default_audio_synth_registry();
    let entry = registry
        .get(&dx7_operator_component_id())
        .expect("dx7 operator entry");
    assert_eq!(entry.category(), ComponentRegistryCategory::Exact);
    assert!(entry.is_implemented());
    assert!(entry.has_capability(ComponentCapability::Traceable));
    assert_eq!(entry.ports().len(), 5);
    assert_eq!(entry.params().len(), 5);
    let instance = entry.instantiate().expect("dx7 operator instance");
    assert_eq!(instance.component_id(), dx7_operator_component_id());
}

#[test]
fn dx7_operator_settings_map_patch_fields() {
    let operator = Dx7PatchOperator {
        rates: [90, 80, 70, 60],
        levels: [99, 88, 77, 0],
        breakpoint: 45,
        left_depth: 12,
        right_depth: 34,
        left_curve: 1,
        right_curve: 3,
        rate_scale: 5,
        amp_mod_sens: 2,
        key_velocity_sens: 6,
        output_level: 82,
        oscillator_mode: 1,
        frequency_coarse: 7,
        frequency_fine: 25,
        detune: 10,
    };
    let patch = Dx7Patch {
        name: "settings".to_owned(),
        operators: vec![operator.clone()],
        pitch_envelope: Dx7Envelope {
            rates: [10, 20, 30, 40],
            levels: [50, 60, 70, 80],
        },
        algorithm: 1,
        feedback: 6,
        oscillator_sync: true,
        lfo: Dx7Lfo {
            speed: 44,
            delay: 3,
            pitch_mod_depth: 22,
            amp_mod_depth: 11,
            sync: true,
            waveform: 2,
            pitch_mod_sens: 5,
        },
        transpose: 24,
        raw: Dx7RawPatch {
            edit_buffer: Vec::new(),
            packed_voice: Vec::new(),
        },
    };

    let settings = Dx7FmOperatorSettings::from_patch_operator(&operator, &patch);
    assert_eq!(settings.pitch.mode, Dx7FrequencyMode::Fixed);
    assert_eq!(settings.pitch.coarse, 7);
    assert_eq!(settings.pitch.fine, 25);
    assert_eq!(settings.pitch.detune, 3);
    assert_eq!(settings.output_level, 82);
    assert_eq!(settings.amp_mod_sens, 2);
    assert_eq!(settings.feedback, 6);
    assert_eq!(settings.scaling.breakpoint, 45);
    assert_eq!(settings.scaling.rate_scale, 5);
    assert_eq!(settings.velocity.sensitivity, 6);
    assert_eq!(settings.envelope.rates, [90, 80, 70, 60]);
    assert_eq!(settings.envelope.levels, [99, 88, 77, 0]);
    assert_eq!(settings.lfo.waveform, 2);
}

fn test_settings(feedback: u8) -> Dx7FmOperatorSettings {
    Dx7FmOperatorSettings {
        pitch: Dx7PitchSettings {
            mode: Dx7FrequencyMode::Fixed,
            coarse: 0,
            fine: 0,
            detune: 0,
        },
        feedback,
        base_key: 60,
        sine_lut_len: 8,
        ..Dx7FmOperatorSettings::default()
    }
}

fn render_raw(operator: &mut Dx7FmOperator, modulation: &[QLevel; 8]) -> [i32; 8] {
    let mut output = [0; 8];
    for (index, modulation) in modulation.iter().enumerate() {
        output[index] = operator
            .next_sample(Dx7OperatorInput {
                gate: true,
                modulation: *modulation,
                ..Dx7OperatorInput::default()
            })
            .sample
            .raw();
    }
    output
}

fn assert_trace_output(operator: &Dx7FmOperator, expected: i32) {
    let trace = operator.trace().expect("trace frame");
    let output = trace
        .records()
        .iter()
        .find(|record| record.key().as_qualified_str().ends_with("/output-raw"))
        .expect("output trace");
    assert_eq!(
        output.value(),
        &ComponentTraceValue::Integer(i64::from(expected))
    );
}
