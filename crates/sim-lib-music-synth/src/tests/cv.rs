use crate::{CvConvention, CvPolarity, GateConvention, GateConverter, GateMode, VoltsPerOctave};

#[test]
fn bipolar_cv_and_volts_per_octave_scale_predictably() {
    let bipolar = CvConvention::bipolar(5.0);
    assert_eq!(bipolar.polarity(), CvPolarity::Bipolar);
    assert_eq!(round4(bipolar.normalize(-5.0)), 0.0);
    assert_eq!(round4(bipolar.normalize(0.0)), 0.5);
    assert_eq!(round4(bipolar.normalize(5.0)), 1.0);
    assert_eq!(round4(bipolar.scale(0.25)), -2.5);

    let pitch = VoltsPerOctave::new(60, 1.0);
    assert_eq!(round4(pitch.midi_key_to_volts(60)), 0.0);
    assert_eq!(round4(pitch.midi_key_to_volts(72)), 1.0);
    assert_eq!(round4(pitch.volts_to_midi_key(0.5)), 66.0);
    assert_eq!(round4(pitch.frequency_hz_to_volts(440.0)), 0.75);
}

#[test]
fn gate_converter_handles_voltage_trigger_and_s_trigger() {
    let mut trigger = GateConverter::new(GateConvention::voltage_trigger());
    assert_eq!(trigger.convention().mode(), GateMode::VoltageTrigger);
    assert!(!trigger.convert(0.0).active);
    let rising = trigger.convert(5.0);
    assert!(rising.active);
    assert!(rising.triggered);
    assert_eq!(round4(rising.voltage_gate_volts), 5.0);
    assert!(!trigger.convert(5.0).triggered);

    let mut s_trigger = GateConverter::new(GateConvention::s_trigger());
    let open = s_trigger.convert(5.0);
    assert!(!open.active);
    assert_eq!(round4(open.voltage_gate_volts), 0.0);
    let shorted = s_trigger.convert(0.0);
    assert!(shorted.active);
    assert!(shorted.triggered);
    assert_eq!(round4(shorted.voltage_gate_volts), 5.0);
}

#[test]
fn gate_converter_reset_makes_edge_detection_deterministic() {
    let mut converter = GateConverter::new(GateConvention::voltage_gate());
    let input = [0.0, 5.0, 5.0, 0.0, 5.0];
    let first = input
        .into_iter()
        .map(|volts| converter.convert(volts))
        .collect::<Vec<_>>();

    converter.reset();
    let second = input
        .into_iter()
        .map(|volts| converter.convert(volts))
        .collect::<Vec<_>>();

    assert_eq!(first, second);
    assert_eq!(
        first
            .iter()
            .map(|frame| frame.triggered)
            .collect::<Vec<_>>(),
        vec![false, true, false, false, true]
    );
}

fn round4(value: f32) -> f32 {
    (value * 10_000.0).round() / 10_000.0
}
