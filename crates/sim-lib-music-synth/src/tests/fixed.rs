use sim_kernel::Symbol;

use crate::{
    ComponentBackend, ComponentTraceFrame, ComponentTraceRole, ComponentTraceValue, FixedRounding,
    GeneratedLut, GeneratedLutKind, QLevel, QPhase,
};

#[test]
fn q_phase_wraps_and_reports_table_position() {
    assert_eq!(QPhase::FORMAT.fractional_bits(), 32);
    assert!(!QPhase::FORMAT.signed());

    let phase = QPhase::from_turns(1.25);
    assert_eq!(phase, QPhase::from_turns(0.25));
    assert_eq!(phase.table_position(4), Some((1, 2, 0.0)));

    let mut accumulator = QPhase::ZERO;
    accumulator.advance_wrapping(QPhase::from_turns(0.75));
    accumulator.advance_wrapping(QPhase::from_turns(0.5));
    assert_eq!(accumulator, QPhase::from_turns(0.25));
    accumulator = QPhase::ZERO;
    assert_eq!(accumulator.raw(), 0);
}

#[test]
fn q_level_saturates_wraps_rounds_truncates_and_applies_bias() {
    assert_eq!(QLevel::FORMAT.integer_bits(), 1);
    assert_eq!(QLevel::FORMAT.fractional_bits(), 30);

    assert_eq!(QLevel::MAX.saturating_add(QLevel::ONE), QLevel::MAX);
    assert_eq!(QLevel::ONE.wrapping_add(QLevel::ONE).raw(), i32::MIN);
    assert_eq!(QLevel::ONE.saturating_mul(QLevel::ONE), QLevel::ONE);

    let half_lsb = 0.5 / (1_i64 << QLevel::FRACTIONAL_BITS) as f64;
    assert_eq!(QLevel::from_f64(half_lsb, FixedRounding::Truncate).raw(), 0);
    assert_eq!(
        QLevel::from_f64(half_lsb, FixedRounding::RoundNearest).raw(),
        1
    );
    assert_eq!(
        QLevel::from_f64(-half_lsb, FixedRounding::BiasAwayFromZero).raw(),
        -1
    );
    assert_eq!(
        QLevel::from_f64_with_raw_bias(0.0, 3, FixedRounding::Truncate).raw(),
        3
    );
    assert_eq!(QLevel::from_raw(7).truncated_shift_right(1), 3);
    assert_eq!(QLevel::from_raw(-7).truncated_shift_right(1), -3);
    assert_eq!(QLevel::from_raw(7).rounded_shift_right(1), 4);
}

#[test]
fn generated_luts_interpolate_and_hold_boundaries() {
    let sine = GeneratedLut::sine(4);
    assert_eq!(sine.kind(), GeneratedLutKind::Sine);
    assert!(sine.wraps());
    assert_close(sine.sample_phase(QPhase::ZERO), 0.0);
    assert_close(sine.sample_phase(QPhase::from_turns(0.25)), 1.0);
    assert_close(sine.sample(1.0), sine.sample(0.0));

    let exp = GeneratedLut::exp(8, -1.0, 1.0);
    assert_eq!(exp.kind(), GeneratedLutKind::Exp);
    assert_close(exp.sample(-10.0), (-1.0_f32).exp());
    assert_close(exp.sample(10.0), 1.0_f32.exp());

    let log = GeneratedLut::log(8, 1.0, 16.0);
    assert_eq!(log.kind(), GeneratedLutKind::Log);
    assert_close(log.sample(1.0), 0.0);
    assert_close(log.sample(16.0), 16.0_f32.ln());
}

#[test]
fn component_trace_records_cover_signal_state_clock_and_integer_values() {
    let frame = ComponentTraceFrame::new(
        Symbol::qualified("audio-synth", "trace-test"),
        ComponentBackend::Modeled,
        42,
    )
    .with_input(
        Symbol::qualified("audio-synth/trace", "input"),
        ComponentTraceValue::Float(0.25),
    )
    .with_state(
        Symbol::qualified("audio-synth/trace", "state"),
        ComponentTraceValue::Text("running".to_owned()),
    )
    .with_output(
        Symbol::qualified("audio-synth/trace", "output"),
        ComponentTraceValue::Float(-0.5),
    )
    .with_clock_position(Symbol::qualified("audio-synth/trace", "clock"), 42)
    .with_integer(Symbol::qualified("audio-synth/trace", "raw"), 17);

    assert_eq!(frame.clock(), 42);
    assert_eq!(
        frame
            .records()
            .iter()
            .map(|record| record.role())
            .collect::<Vec<_>>(),
        vec![
            ComponentTraceRole::Input,
            ComponentTraceRole::State,
            ComponentTraceRole::Output,
            ComponentTraceRole::Clock,
            ComponentTraceRole::Integer,
        ]
    );
    assert_eq!(
        frame.records()[4].value(),
        &ComponentTraceValue::Integer(17)
    );
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 0.0001,
        "{actual} != {expected}"
    );
}
