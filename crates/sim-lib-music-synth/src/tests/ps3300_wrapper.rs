use crate::{
    ComponentCapability, ComponentRegistryCategory, InstrumentPatch, InstrumentWrapperCategory,
    PS3300_FIXTURE_REGENERATE_COMMAND, Ps3300, Ps3300PatchProfile, Ps3300RenderFixtureKind,
    Ps3300RenderMode, default_audio_synth_registry, ps_3300_component_id, ps3300_audio_graph,
    ps3300_default_patch, ps3300_default_patch_id, ps3300_fixture_profiles,
    ps3300_fixture_regeneration_command, ps3300_mode_tolerance_report,
    ps3300_patch_round_trip_patch, ps3300_polyphony_summary, ps3300_recipe_path,
    ps3300_render_fixture_ids, ps3300_render_fixture_manifest, ps3300_render_fixtures,
    ps3300_render_gate, ps3300_render_mode_symbols, ps3300_required_module_ids,
    ps3300_section_graph, ps3300_three_section_stack_patch, ps3300_user_patch_path,
    render_ps3300_fixture,
};

#[test]
fn ps3300_wrapper_default_patch_and_fixture_ids_are_recorded() {
    assert_eq!(
        ps3300_default_patch_id().as_qualified_str(),
        "audio-synth/patch/korg-ps-3300-synthetic-polyphonic"
    );
    assert_eq!(
        ps3300_user_patch_path(),
        "$HOME/.local/share/sim/ps3300/synthetic-polyphonic.patch.siml"
    );
    assert_eq!(
        ps3300_recipe_path(),
        "crates/sim-lib-music-synth/recipes/ps3300/synthetic-polyphonic-patch/recipe.toml"
    );
    assert_eq!(
        ps3300_render_fixture_ids(),
        vec![
            "ps3300-ps3-one-cell-render",
            "ps3300-ps3-one-section-chord-render",
            "ps3300-ps3-resonator-sweep-render",
            "ps3300-ps3-three-section-stack-render",
            "ps3300-default-patch-round-trip",
        ]
    );
    assert_eq!(
        ps3300_fixture_profiles(),
        [
            Ps3300PatchProfile::OneCell,
            Ps3300PatchProfile::OneSectionChord,
            Ps3300PatchProfile::ResonatorSweep,
            Ps3300PatchProfile::ThreeSectionStack,
            Ps3300PatchProfile::PatchRoundTrip,
        ]
    );
    assert_eq!(
        ps3300_section_graph(),
        vec![
            "keyboard->pin-matrix",
            "modulation-generator->pin-matrix",
            "sample-hold->pin-matrix",
            "external-processor->pin-matrix",
            "pin-matrix->section-a",
            "pin-matrix->section-b",
            "pin-matrix->section-c",
            "sections->triple-resonator",
            "sections+resonator->output-mixer",
        ]
    );
    assert_eq!(ps3300_polyphony_summary().section_count, 3);
    assert_eq!(ps3300_polyphony_summary().keys_per_section, 48);
    assert_eq!(ps3300_polyphony_summary().total_key_cells, 144);

    let patch = ps3300_default_patch();
    assert_eq!(patch.name, ps3300_default_patch_id());
    assert_eq!(patch.modules.len(), 10);
    assert_eq!(patch.cords.len(), 17);
}

#[test]
fn ps3300_registry_entry_is_implemented_wrapper() {
    let registry = default_audio_synth_registry();
    let entry = registry
        .get(&ps_3300_component_id())
        .expect("PS-3300 entry");

    assert_eq!(entry.category(), ComponentRegistryCategory::Exact);
    assert_eq!(entry.wrapper(), InstrumentWrapperCategory::FixedPolysynth);
    assert!(entry.is_implemented());
    assert!(entry.has_capability(ComponentCapability::RealtimeSafe));
    assert!(entry.has_capability(ComponentCapability::Traceable));
    assert_eq!(
        entry.instantiate().unwrap().component_id(),
        ps_3300_component_id()
    );

    let ps3300 =
        Ps3300::from_registry(&registry, ps3300_default_patch(), Ps3300RenderMode::Modeled)
            .expect("registry-backed wrapper");
    assert_eq!(ps3300.patch().name, ps3300_default_patch_id());
    assert_eq!(ps3300_required_module_ids().len(), 11);
}

#[test]
fn ps3300_render_modes_and_fixture_tolerances_are_recorded() {
    assert_eq!(
        ps3300_render_mode_symbols()
            .iter()
            .map(|symbol| symbol.as_qualified_str())
            .collect::<Vec<_>>(),
        vec![
            "audio-synth/ps3300-render-mode/ideal",
            "audio-synth/ps3300-render-mode/modeled",
            "audio-synth/ps3300-render-mode/trace",
        ]
    );

    let gate = ps3300_render_gate();
    assert_eq!(
        gate.modes,
        vec![
            Ps3300RenderMode::Ideal,
            Ps3300RenderMode::Modeled,
            Ps3300RenderMode::Trace,
        ]
    );
    assert_eq!(gate.required_fixture_ids, ps3300_render_fixture_ids());

    let fixtures = ps3300_render_fixtures();
    assert_eq!(fixtures.len(), 5);
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.kind == Ps3300RenderFixtureKind::OneCell)
    );
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.kind == Ps3300RenderFixtureKind::OneSectionChord)
    );
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.kind == Ps3300RenderFixtureKind::ResonatorSweep)
    );
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.kind == Ps3300RenderFixtureKind::ThreeSectionStack)
    );

    for fixture in fixtures {
        let output = render_ps3300_fixture(&fixture);
        assert_eq!(output.len(), fixture.channels);
        assert_eq!(output[0].len(), fixture.frames);
        assert!(output[0].iter().all(|sample| sample.is_finite()));
        assert!(output[0].iter().any(|sample| sample.abs() > 0.0));

        let report = ps3300_mode_tolerance_report(&fixture);
        assert_eq!(report.frames, fixture.frames * fixture.channels);
        assert!(
            report.passed,
            "{} max={} mean={}",
            report.fixture_id, report.max_abs_delta, report.mean_abs_delta
        );
    }
}

#[test]
fn ps3300_wrapper_renders_through_processor_and_graph() {
    let fixture = ps3300_render_fixtures()
        .into_iter()
        .find(|fixture| fixture.kind == Ps3300RenderFixtureKind::ThreeSectionStack)
        .expect("three-section stack fixture");
    let processor_output = render_ps3300_fixture(&fixture);
    assert!(peak(&processor_output[0]) > 0.01);

    let mut graph = ps3300_audio_graph(ps3300_three_section_stack_patch(), Ps3300RenderMode::Trace)
        .expect("PS-3300 graph");
    graph.prepare(48_000, 128).expect("prepared graph");
    let graph_output = graph.process_offline(&[], 128).expect("graph output");

    assert_eq!(graph_output.len(), 1);
    assert_eq!(graph_output[0].len(), 128);
    assert!(graph_output[0].iter().any(|sample| sample.abs() > 0.0));
}

#[test]
fn ps3300_patch_round_trip_fixture_serializes() {
    let patch = ps3300_patch_round_trip_patch();
    let expr = patch.to_expr();
    let decoded = InstrumentPatch::from_expr(&expr).expect("patch round trip");
    assert_eq!(decoded, patch);

    let fixture = ps3300_render_fixtures()
        .into_iter()
        .find(|fixture| fixture.kind == Ps3300RenderFixtureKind::PatchRoundTrip)
        .expect("patch round trip fixture");
    assert_eq!(fixture.metadata.patch_hash.len(), 16);
    assert_eq!(fixture.metadata.trace_hash.len(), 16);
    assert_eq!(
        fixture.metadata.default_patch_id,
        ps3300_default_patch_id().as_qualified_str()
    );
}

#[test]
fn ps3300_fixture_manifest_matches_regenerated_output() {
    assert_eq!(
        ps3300_fixture_regeneration_command(),
        PS3300_FIXTURE_REGENERATE_COMMAND
    );
    assert_eq!(
        include_str!("../../fixtures/ps3300/render-fixtures.toml"),
        ps3300_render_fixture_manifest()
    );
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().copied().map(f32::abs).fold(0.0, f32::max)
}
