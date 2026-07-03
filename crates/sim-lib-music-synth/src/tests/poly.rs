use sim_kernel::{Expr, Symbol};

use crate::{
    ComponentPortMedia, GateConvention, PerKeyGateBus, PerKeyGateInput, PolyphonicArray,
    PolyphonicSectionSetting, VoltsPerOctave,
};

#[test]
fn polyphonic_array_fans_out_per_key_pitch_and_gate_bus() {
    let mut array = PolyphonicArray::new(
        Symbol::qualified("audio-synth", "poly-array-test"),
        2,
        VoltsPerOctave::new(60, 1.0),
        GateConvention::s_trigger(),
    );
    let bus = PerKeyGateBus::default()
        .with_key(PerKeyGateInput::new(60, 0.0, 0.8))
        .with_key(PerKeyGateInput::new(72, 5.0, 0.4))
        .with_key(PerKeyGateInput::new(84, 0.0, 1.0));

    let first = array.fan_out(&bus);
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].voice_index, 0);
    assert_eq!(round4(first[0].pitch.volts()), 0.0);
    assert_eq!(round4(first[0].pitch.normalized()), 0.5);
    assert!(first[0].gate.active);
    assert!(first[0].gate.triggered);
    assert_eq!(round4(first[0].gate.voltage_gate_volts), 5.0);
    assert_eq!(first[1].voice_index, 1);
    assert_eq!(round4(first[1].pitch.volts()), 1.0);
    assert!(!first[1].gate.active);
    assert_eq!(round4(first[1].velocity), 0.4);

    let held = array.fan_out(&bus);
    assert!(!held[0].gate.triggered);
    array.reset();
    assert_eq!(array.fan_out(&bus), first);
}

#[test]
fn polyphonic_array_exposes_per_note_topology_modules() {
    let array = PolyphonicArray::new(
        Symbol::qualified("audio-synth", "poly-array-test"),
        2,
        VoltsPerOctave::new(60, 1.0),
        GateConvention::voltage_gate(),
    )
    .with_section_setting(PolyphonicSectionSetting::new(
        Symbol::new("filter"),
        Symbol::new("cutoff-hz"),
        Expr::String("8000".to_owned()),
    ));

    let patch = array.per_note_patch();
    assert_eq!(patch.modules.len(), 2);
    for (voice_index, module) in patch.modules.iter().enumerate() {
        assert_eq!(module.id.name.as_ref(), format!("voice-{voice_index}"));
        assert!(
            module
                .inputs
                .iter()
                .any(|jack| jack.media == ComponentPortMedia::ControlVoltage)
        );
        assert!(
            module
                .inputs
                .iter()
                .any(|jack| jack.media == ComponentPortMedia::Gate)
        );
        assert!(
            module
                .settings
                .iter()
                .any(|setting| setting.key.name.as_ref() == "cutoff-hz")
        );
    }
}

fn round4(value: f32) -> f32 {
    (value * 10_000.0).round() / 10_000.0
}
