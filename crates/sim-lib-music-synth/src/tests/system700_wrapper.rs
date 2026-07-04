use crate::{
    ComponentCapability, ComponentRegistryCategory, InstrumentPatch, InstrumentWrapperCategory,
    System700, System700RenderFixtureKind, System700RenderMode, default_audio_synth_registry,
    render_system700_fixture, system_700_component_id, system700_audio_graph,
    system700_default_patch, system700_default_patch_id, system700_mode_tolerance_report,
    system700_patch_round_trip_patch, system700_recipe_path, system700_render_fixture_ids,
    system700_render_fixtures, system700_render_gate, system700_render_mode_symbols,
    system700_required_module_ids, system700_sequencer_patch, system700_user_patch_path,
};

#[test]
fn system700_wrapper_default_patch_and_fixture_ids_are_recorded() {
    assert_eq!(
        system700_default_patch_id().as_qualified_str(),
        "audio-synth/patch/roland-system-700-main-console"
    );
    assert_eq!(
        system700_user_patch_path(),
        "$HOME/.local/share/sim/system700/main-console.patch.siml"
    );
    assert_eq!(
        system700_recipe_path(),
        "crates/sim-lib-music-synth/recipes/system700/synthetic-main-console/recipe.toml"
    );
    assert_eq!(
        system700_render_fixture_ids(),
        vec![
            "system700-r700-single-module-render",
            "system700-r700-two-module-patch-render",
            "system700-default-main-console-voice",
            "system700-sequencer-driven-patch",
            "system700-default-patch-round-trip",
        ]
    );

    let patch = system700_default_patch();
    assert_eq!(patch.name, system700_default_patch_id());
    assert_eq!(patch.modules.len(), 8);
    assert_eq!(patch.cords.len(), 9);
}

#[test]
fn system700_registry_entry_is_implemented_wrapper() {
    let registry = default_audio_synth_registry();
    let entry = registry
        .get(&system_700_component_id())
        .expect("System 700 entry");

    assert_eq!(entry.category(), ComponentRegistryCategory::Exact);
    assert_eq!(entry.wrapper(), InstrumentWrapperCategory::ModularAnalog);
    assert!(entry.is_implemented());
    assert!(entry.has_capability(ComponentCapability::RealtimeSafe));
    assert!(entry.has_capability(ComponentCapability::Traceable));
    assert_eq!(
        entry.instantiate().unwrap().component_id(),
        system_700_component_id()
    );

    let system = System700::from_registry(
        &registry,
        system700_default_patch(),
        System700RenderMode::Modeled,
    )
    .expect("registry-backed wrapper");
    assert_eq!(system.patch().name, system700_default_patch_id());
    assert_eq!(system700_required_module_ids().len(), 15);
}

#[test]
fn system700_render_modes_and_fixture_tolerances_are_recorded() {
    assert_eq!(
        system700_render_mode_symbols()
            .iter()
            .map(|symbol| symbol.as_qualified_str())
            .collect::<Vec<_>>(),
        vec![
            "audio-synth/system700-render-mode/ideal",
            "audio-synth/system700-render-mode/modeled",
            "audio-synth/system700-render-mode/trace",
        ]
    );

    let gate = system700_render_gate();
    assert_eq!(
        gate.modes,
        vec![
            System700RenderMode::Ideal,
            System700RenderMode::Modeled,
            System700RenderMode::Trace,
        ]
    );
    assert_eq!(gate.required_fixture_ids, system700_render_fixture_ids());

    let fixtures = system700_render_fixtures();
    assert_eq!(fixtures.len(), 5);
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.kind == System700RenderFixtureKind::SingleModule)
    );
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.kind == System700RenderFixtureKind::TwoModulePatch)
    );
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.kind == System700RenderFixtureKind::SequencerDrivenPatch)
    );

    for fixture in fixtures {
        let output = render_system700_fixture(&fixture);
        assert_eq!(output.len(), fixture.channels);
        assert_eq!(output[0].len(), fixture.frames);
        assert!(output[0].iter().all(|sample| sample.is_finite()));
        assert!(output[0].iter().any(|sample| sample.abs() > 0.0));

        let report = system700_mode_tolerance_report(&fixture);
        assert_eq!(report.frames, fixture.frames * fixture.channels);
        assert!(
            report.passed,
            "{} max={} mean={}",
            report.fixture_id, report.max_abs_delta, report.mean_abs_delta
        );
    }
}

#[test]
fn system700_wrapper_renders_through_processor_and_graph() {
    let fixture = system700_render_fixtures()
        .into_iter()
        .find(|fixture| fixture.id == "system700-default-main-console-voice")
        .expect("default voice fixture");
    let processor_output = render_system700_fixture(&fixture);
    assert!(peak(&processor_output[0]) > 0.05);

    let mut graph = system700_audio_graph(system700_sequencer_patch(), System700RenderMode::Trace)
        .expect("System 700 graph");
    graph.prepare(48_000, 128).expect("prepared graph");
    let graph_output = graph.process_offline(&[], 128).expect("graph output");

    assert_eq!(graph_output.len(), 1);
    assert_eq!(graph_output[0].len(), 128);
    assert!(graph_output[0].iter().any(|sample| sample.abs() > 0.0));
}

#[test]
fn system700_patch_round_trip_fixture_serializes() {
    let patch = system700_patch_round_trip_patch();
    let expr = patch.to_expr();
    let decoded = InstrumentPatch::from_expr(&expr).expect("patch round trip");
    assert_eq!(decoded, patch);

    let fixture = system700_render_fixtures()
        .into_iter()
        .find(|fixture| fixture.kind == System700RenderFixtureKind::PatchRoundTrip)
        .expect("patch round trip fixture");
    assert_eq!(fixture.metadata.patch_hash.len(), 16);
    assert_eq!(
        fixture.metadata.default_patch_id,
        system700_default_patch_id().as_qualified_str()
    );
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().copied().map(f32::abs).fold(0.0, f32::max)
}
