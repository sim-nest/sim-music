use sim_kernel::Symbol;

use crate::{
    ComponentCapability, ComponentRegistryCategory, GateMode, InstrumentPatch,
    InstrumentWrapperCategory, default_audio_synth_registry,
    system55::{
        SYSTEM55_RECIPE_BOOK_PATH, SYSTEM55_RECIPE_CHAPTER_PATH, System55ModuleRole,
        system55_control_fixture_names, system55_gate_mode_symbols, system55_module_descriptors,
        system55_module_ids, system55_s_trigger_fit_evidence,
        system55_s_trigger_voltage_gate_frames, system55_scaffold_patch,
        system55_scaffold_patch_id,
    },
};

#[test]
fn system55_scaffold_records_stable_module_ids_and_paths() {
    assert_eq!(
        system55_module_ids()
            .iter()
            .map(Symbol::as_qualified_str)
            .collect::<Vec<_>>(),
        vec![
            "audio-synth/module/m55-921a-oscillator-driver",
            "audio-synth/module/m55-921b-oscillator",
            "audio-synth/module/m55-923-noise-filter",
            "audio-synth/module/m55-904a-low-pass-filter",
            "audio-synth/module/m55-904b-high-pass-filter",
            "audio-synth/module/m55-904c-filter-coupler",
            "audio-synth/module/m55-907-fixed-filter-bank",
            "audio-synth/module/m55-902-vca",
            "audio-synth/module/m55-911-envelope-generator",
            "audio-synth/module/m55-911a-dual-trigger-delay",
            "audio-synth/module/m55-912-envelope-follower",
            "audio-synth/module/m55-1630-frequency-shifter",
            "audio-synth/module/m55-ring-modulator",
            "audio-synth/module/m55-928-sample-hold",
            "audio-synth/module/m55-960-sequential-controller",
            "audio-synth/module/m55-961-interface",
            "audio-synth/module/m55-cp3a-mixer",
            "audio-synth/module/m55-multiple",
            "audio-synth/module/m55-attenuator",
            "audio-synth/module/m55-956-ribbon-controller",
            "audio-synth/module/m55-951-keyboard-controller",
        ]
    );
    assert_eq!(
        system55_control_fixture_names(),
        [
            "system55-m55-vca-gain-law",
            "system55-m55-envelope-s-trigger-timing",
            "system55-m55-trigger-delay-s-trigger-timing",
            "system55-m55-envelope-follower-gate",
            "system55-m55-fixed-filter-bank-centers",
            "system55-m55-frequency-shifter-sidebands",
            "system55-m55-ring-modulator-sidebands",
            "system55-m55-mixer-mix-behavior",
            "system55-m55-multiple-attenuator-utility",
            "system55-m55-sample-hold-s-trigger",
            "system55-m55-sequencer-advance",
            "system55-m55-ribbon-keyboard-mapping",
        ]
    );
    assert_eq!(
        SYSTEM55_RECIPE_BOOK_PATH,
        "crates/sim-lib-music-synth/recipes/system55/book.toml"
    );
    assert_eq!(
        SYSTEM55_RECIPE_CHAPTER_PATH,
        "crates/sim-lib-music-synth/recipes/system55/chapter.toml"
    );
}

#[test]
fn system55_scaffold_uses_shared_patch_model() {
    let patch = system55_scaffold_patch();
    assert_eq!(patch.name, system55_scaffold_patch_id());
    assert_eq!(patch.modules.len(), 6);
    assert_eq!(patch.cords.len(), 4);
    assert_eq!(
        patch.modules[0].kind.as_qualified_str(),
        "audio-synth/module/m55-921a-oscillator-driver"
    );
    assert!(
        patch
            .settings
            .iter()
            .any(|setting| setting.key.as_qualified_str() == "audio-synth/system55/gate-mode")
    );

    let expr = patch.to_expr();
    let decoded = InstrumentPatch::from_expr(&expr).expect("system55 patch round trip");
    assert_eq!(decoded, patch);
}

#[test]
fn system55_registry_records_exact_module_entries() {
    let registry = default_audio_synth_registry();
    for id in system55_module_ids() {
        let entry = registry.get(&id).expect("System 55 module entry");
        assert_eq!(entry.wrapper(), InstrumentWrapperCategory::ModularAnalog);
        assert!(entry.has_capability(ComponentCapability::Editable));
        assert!(entry.has_capability(ComponentCapability::SpecializedView));
        assert!(!entry.ports().is_empty());
        assert_eq!(entry.category(), ComponentRegistryCategory::Exact);
        assert!(entry.is_implemented());
        assert!(entry.has_capability(ComponentCapability::RealtimeSafe));
        assert!(entry.has_capability(ComponentCapability::Traceable));
    }

    let descriptors = system55_module_descriptors();
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.role == System55ModuleRole::Envelope
                && descriptor.gate.is_some())
    );
}

#[test]
fn system55_s_trigger_maps_to_voltage_gate_frames() {
    assert_eq!(
        system55_gate_mode_symbols()
            .iter()
            .map(Symbol::as_qualified_str)
            .collect::<Vec<_>>(),
        vec![
            "audio-synth/gate/s-trigger",
            "audio-synth/gate/voltage-gate"
        ]
    );

    let evidence = system55_s_trigger_fit_evidence();
    assert_eq!(evidence.gate_mode, GateMode::STrigger);
    assert_eq!(evidence.native_inactive_voltage_v, 5.0);
    assert_eq!(evidence.native_active_voltage_v, 0.0);
    assert_eq!(evidence.voltage_gate_inactive_v, 0.0);
    assert_eq!(evidence.voltage_gate_active_v, 5.0);

    let frames = system55_s_trigger_voltage_gate_frames(&[5.0, 0.0, 0.0, 5.0, 0.0]);
    assert_eq!(
        frames.iter().map(|frame| frame.active).collect::<Vec<_>>(),
        vec![false, true, true, false, true]
    );
    assert_eq!(
        frames
            .iter()
            .map(|frame| frame.triggered)
            .collect::<Vec<_>>(),
        vec![false, true, false, false, true]
    );
    assert_eq!(
        frames
            .iter()
            .map(|frame| frame.voltage_gate_volts)
            .collect::<Vec<_>>(),
        vec![0.0, 5.0, 5.0, 0.0, 5.0]
    );
}
