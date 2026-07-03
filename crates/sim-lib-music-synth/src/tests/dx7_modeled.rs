use crate::{
    ComponentBackend, ComponentCapability, ComponentRegistryCategory, ComponentTraceValue,
    DiscreteComponent, Dx7DacWordWidths, Dx7EgsWordWidths, Dx7FmOperator, Dx7FmOperatorSettings,
    Dx7FrequencyMode, Dx7ModeledOperator, Dx7OperatorInput, Dx7OpsWordWidths, Dx7PitchSettings,
    QLevel, assert_backend_surface_identity, default_audio_synth_registry,
    dx7_modeled_divergence_report, dx7_modeled_operator_component_id,
    dx7_modeled_trace_fixture_names, dx7_operator_backend_surfaces,
};

#[test]
fn dx7_modeled_operator_shares_algorithmic_contract() {
    let [algorithmic, modeled] = dx7_operator_backend_surfaces();
    assert_eq!(algorithmic.backend(), ComponentBackend::Algorithmic);
    assert_eq!(modeled.backend(), ComponentBackend::Modeled);
    assert_backend_surface_identity(&algorithmic, &modeled).unwrap();
    assert_eq!(algorithmic.ports(), modeled.ports());
    assert_eq!(algorithmic.params(), modeled.params());
}

#[test]
fn dx7_modeled_word_widths_are_declared() {
    assert_eq!(Dx7OpsWordWidths::declared().phase_bits, 32);
    assert_eq!(Dx7OpsWordWidths::declared().log_bits, 15);
    assert_eq!(Dx7OpsWordWidths::declared().output_bits, 14);
    assert_eq!(Dx7EgsWordWidths::declared().level_bits, 14);
    assert_eq!(Dx7EgsWordWidths::declared().rate_bits, 7);
    assert_eq!(Dx7EgsWordWidths::declared().pitch_bits, 16);
    assert_eq!(Dx7DacWordWidths::declared().input_bits, 14);
    assert_eq!(Dx7DacWordWidths::declared().held_sample_bits, 24);
}

#[test]
fn dx7_modeled_operator_is_deterministic_after_reset() {
    let mut operator = Dx7ModeledOperator::new(test_settings(7));
    operator.prepare(crate::ComponentPrepareConfig::new(8, 8, 1, 1));

    let first = render_modeled_raw(&mut operator, &[QLevel::ZERO; 8]);
    DiscreteComponent::reset(&mut operator);
    operator.prepare(crate::ComponentPrepareConfig::new(8, 8, 1, 1));
    let second = render_modeled_raw(&mut operator, &[QLevel::ZERO; 8]);
    assert_eq!(first, second);
    assert_trace_output(&operator, second[7]);
}

#[test]
fn dx7_modeled_vs_algorithmic_divergence_report_is_stable() {
    let settings = test_settings(7);
    let mut algorithmic = Dx7FmOperator::new(settings.clone());
    let mut modeled = Dx7ModeledOperator::new(settings);
    algorithmic.prepare(crate::ComponentPrepareConfig::new(8, 8, 1, 1));
    modeled.prepare(crate::ComponentPrepareConfig::new(8, 8, 1, 1));

    let algorithmic = render_algorithmic_raw(&mut algorithmic, &[QLevel::ZERO; 8]);
    let modeled = render_modeled_raw(&mut modeled, &[QLevel::ZERO; 8]);
    let report = dx7_modeled_divergence_report(&algorithmic, &modeled);

    assert_eq!(report.frames, 8);
    assert_eq!(report.max_abs_delta, 648_027_792);
    assert_eq!(report.sum_abs_delta, 1_557_502_816);
}

#[test]
fn dx7_modeled_integer_trace_matches_fixture() {
    assert_eq!(
        dx7_modeled_trace_fixture_names(),
        [
            "dx7-modeled-single-op-i32",
            "dx7-modeled-divergence-report",
            "dx7-modeled-integer-trace",
        ]
    );

    let mut operator = Dx7ModeledOperator::new(test_settings(7));
    operator.prepare(crate::ComponentPrepareConfig::new(8, 8, 1, 1));
    let output = render_modeled_raw(&mut operator, &[QLevel::ZERO; 8]);

    assert_eq!(
        output,
        [
            0,
            805_273_600,
            1_073_741_824,
            805_273_600,
            0,
            -805_273_600,
            -1_073_741_824,
            -809_468_416,
        ]
    );
    assert_trace_value(&operator, "ops-output-raw", -6175);
    assert_trace_value(&operator, "output-raw", -809_468_416);
}

#[test]
fn dx7_modeled_registry_entry_is_implemented_and_traceable() {
    let registry = default_audio_synth_registry();
    let entry = registry
        .get(&dx7_modeled_operator_component_id())
        .expect("dx7 modeled operator entry");
    assert_eq!(entry.category(), ComponentRegistryCategory::Exact);
    assert!(entry.is_implemented());
    assert!(entry.has_capability(ComponentCapability::Traceable));
    assert_eq!(entry.ports().len(), 5);
    assert_eq!(entry.params().len(), 5);
    let instance = entry.instantiate().expect("dx7 modeled operator instance");
    assert_eq!(instance.component_id(), dx7_modeled_operator_component_id());
    assert_eq!(instance.backend(), ComponentBackend::Modeled);
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

fn render_algorithmic_raw(operator: &mut Dx7FmOperator, modulation: &[QLevel; 8]) -> [i32; 8] {
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

fn render_modeled_raw(operator: &mut Dx7ModeledOperator, modulation: &[QLevel; 8]) -> [i32; 8] {
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

fn assert_trace_output(operator: &Dx7ModeledOperator, expected: i32) {
    assert_trace_value(operator, "output-raw", expected);
}

fn assert_trace_value(operator: &Dx7ModeledOperator, suffix: &str, expected: i32) {
    let trace = operator.trace().expect("trace frame");
    let expected_key = format!("audio-synth/dx7-modeled-trace/{suffix}");
    let output = trace
        .records()
        .iter()
        .find(|record| record.key().as_qualified_str() == expected_key)
        .expect("trace record");
    assert_eq!(
        output.value(),
        &ComponentTraceValue::Integer(i64::from(expected))
    );
}
