use std::collections::BTreeSet;

use sim_kernel::{Expr, Symbol};
use sim_lib_audio_graph_core::{
    BlockArena, BlockEvent, Graph, NullEventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};

use crate::{
    ComponentBackend, DX7_ALGORITHM_COUNT, DX7_ALGORITHM_TOPOLOGIES,
    DX7_FIXTURE_REGENERATE_COMMAND, DX7_OPERATOR_COUNT, DiscreteComponent, Dx7Lfo, Dx7Patch,
    Dx7PatchOperator, Dx7RawPatch, Dx7RenderFixtureKind, Dx7RendererAccuracyStatus, Dx7Voice,
    default_audio_synth_registry, dx7_accurate_renderer_gate, dx7_algorithm_topology,
    dx7_compatible_renderer_tolerance_report, dx7_component_id, dx7_fixture_regeneration_command,
    dx7_patch_algorithm_id, dx7_render_fixture_ids, dx7_render_fixture_manifest,
    dx7_render_fixtures, dx7_synthetic_bank_fixture, dx7_voice_audio_graph, render_dx7_fixture,
};

#[test]
fn dx7_algorithm_table_covers_all_32_topologies() {
    assert_eq!(DX7_ALGORITHM_TOPOLOGIES.len(), DX7_ALGORITHM_COUNT);
    for id in 1..=DX7_ALGORITHM_COUNT as u8 {
        let topology = dx7_algorithm_topology(id).expect("algorithm topology");
        assert_eq!(topology.id, id);
        assert_eq!(topology.operator_order, [1, 2, 3, 4, 5, 6]);
        assert!(topology.carrier_count() >= 1, "algorithm {id}");
        assert_eq!(topology.gain_points.len(), DX7_OPERATOR_COUNT);
        assert!(topology.feedback_edge.is_some(), "algorithm {id}");

        let graph = topology.to_topology_graph(ComponentBackend::Algorithmic);
        assert_eq!(graph.nodes.len(), DX7_OPERATOR_COUNT + 1, "algorithm {id}");
        assert_eq!(
            graph.edges.len(),
            topology.modulation_edges().count() + topology.carrier_count(),
            "algorithm {id}"
        );
    }

    assert_eq!(dx7_patch_algorithm_id(0), 1);
    assert_eq!(dx7_patch_algorithm_id(33), 32);
}

#[test]
fn dx7_voice_processor_handles_midi_controls() {
    let mut voice = Dx7Voice::new(probe_patch(14), ComponentBackend::Algorithmic);
    Processor::prepare(&mut voice, PrepareConfig::new(48_000, 8, 0, 1));

    let events = [
        BlockEvent::NoteOn {
            offset: 0,
            channel: 0,
            key: 60,
            velocity: 1.0,
        },
        BlockEvent::Midi {
            offset: 1,
            bytes: [0xe0, 0x7f, 0x7f],
            len: 3,
        },
        BlockEvent::Midi {
            offset: 2,
            bytes: [0xb0, 1, 100],
            len: 3,
        },
        BlockEvent::Midi {
            offset: 3,
            bytes: [0xd0, 80, 0],
            len: 2,
        },
        BlockEvent::Midi {
            offset: 4,
            bytes: [0xb0, 64, 127],
            len: 3,
        },
        BlockEvent::NoteOff {
            offset: 5,
            channel: 0,
            key: 60,
            velocity: 0.0,
        },
        BlockEvent::Midi {
            offset: 7,
            bytes: [0xb0, 64, 0],
            len: 3,
        },
    ];
    let output = render_voice(&mut voice, &events, 8);

    assert!(output.iter().all(|sample| sample.is_finite()));
    assert!(output.iter().any(|sample| sample.abs() > 0.0));
    let control = voice.control();
    assert!(!control.gate);
    assert!(!control.sustain);
    assert!(control.pitch_bend_semitones > 1.99);
    assert!((control.mod_wheel - 100.0 / 127.0).abs() < 0.0001);
    assert!((control.aftertouch - 80.0 / 127.0).abs() < 0.0001);
    assert!(voice.trace().is_some());
}

#[test]
fn dx7_voice_renders_through_audio_graph() {
    let mut voice = Dx7Voice::new(probe_patch(8), ComponentBackend::Algorithmic);
    voice.note_on(0, 60, 1.0);

    let mut graph = Graph::new();
    graph
        .add_node("dx7-voice", Box::new(voice), 0, 1)
        .expect("voice node");
    graph.prepare(48_000, 16).expect("prepared graph");
    let output = graph.process_offline(&[], 16).expect("graph output");

    assert_eq!(output.len(), 1);
    assert_eq!(output[0].len(), 16);
    assert!(output[0].iter().any(|sample| sample.abs() > 0.0));

    let mut graph = dx7_voice_audio_graph(probe_patch(1), ComponentBackend::Modeled)
        .expect("voice graph factory");
    graph.prepare(48_000, 4).expect("factory graph prepared");
    assert_eq!(graph.process_offline(&[], 4).unwrap()[0].len(), 4);
}

#[test]
fn dx7_probe_patch_renders_deterministically_for_every_algorithm() {
    for id in 1..=DX7_ALGORITHM_COUNT as u8 {
        let mut first = Dx7Voice::new(probe_patch(id), ComponentBackend::Algorithmic);
        let mut second = Dx7Voice::new(probe_patch(id), ComponentBackend::Algorithmic);
        Processor::prepare(&mut first, PrepareConfig::new(48_000, 16, 0, 1));
        Processor::prepare(&mut second, PrepareConfig::new(48_000, 16, 0, 1));
        first.note_on(0, 60, 1.0);
        second.note_on(0, 60, 1.0);

        let left = round6(&render_voice(&mut first, &[], 16));
        let right = round6(&render_voice(&mut second, &[], 16));
        assert_eq!(left, right, "algorithm {id}");
    }
}

#[test]
fn dx7_graph_inspection_serializes_for_web_ui() {
    let algorithmic = Dx7Voice::new(probe_patch(32), ComponentBackend::Algorithmic);
    let modeled = Dx7Voice::new(probe_patch(32), ComponentBackend::Modeled);
    let inspection = algorithmic.graph_inspection();

    assert_eq!(inspection.algorithm_id, 32);
    assert_eq!(inspection.node_count, DX7_OPERATOR_COUNT + 1);
    assert_eq!(inspection.carrier_count, 6);
    assert_eq!(
        algorithmic.topology_graph().edges.len(),
        modeled.topology_graph().edges.len()
    );

    let Expr::Map(entries) = inspection.to_expr() else {
        panic!("inspection should serialize as a map");
    };
    assert!(entries.iter().any(|(key, value)| {
        key == &field("tag")
            && value == &Expr::Symbol(Symbol::qualified("audio-synth", "dx7-graph-inspection"))
    }));
    assert!(entries.iter().any(|(key, _)| key == &field("nodes")));
    assert!(entries.iter().any(|(key, _)| key == &field("edges")));
}

#[test]
fn dx7_registry_entry_is_implemented_wrapper() {
    let registry = default_audio_synth_registry();
    let entry = registry.get(&dx7_component_id()).expect("dx7 entry");

    assert!(entry.is_implemented());
    assert_eq!(entry.ports().len(), 4);
    assert_eq!(entry.params().len(), 5);
    let instance = entry.instantiate().expect("dx7 voice instance");
    assert_eq!(instance.component_id(), dx7_component_id());
}

#[test]
fn dx7_render_fixture_catalog_records_metadata() {
    let fixtures = dx7_render_fixtures();
    assert_eq!(fixtures.len(), DX7_ALGORITHM_COUNT + 3);

    let algorithm_ids = fixtures
        .iter()
        .filter_map(|fixture| match fixture.kind {
            Dx7RenderFixtureKind::Algorithm(id) => Some(id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        algorithm_ids,
        (1..=DX7_ALGORITHM_COUNT as u8).collect::<Vec<_>>()
    );
    assert!(fixtures.iter().any(|fixture| fixture.id == "dx7-single-op"
        && fixture.kind == Dx7RenderFixtureKind::SingleOperator));
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.id == "dx7-two-op-modulation"
                && fixture.kind == Dx7RenderFixtureKind::TwoOperatorModulation)
    );
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.id == "dx7-envelope-gate-release"
                && fixture.kind == Dx7RenderFixtureKind::Envelope)
    );

    for fixture in fixtures {
        assert_eq!(fixture.metadata.sample_rate_hz, fixture.sample_rate_hz);
        assert_eq!(fixture.metadata.mode, "algorithmic");
        assert_eq!(fixture.metadata.patch_hash.len(), 16);
        assert_eq!(fixture.metadata.trace_hash.len(), 16);
        assert!(!fixture.metadata.midi_sequence.is_empty());
        assert!(
            fixture
                .metadata
                .component_versions
                .iter()
                .any(|version| version.starts_with("Dx7Voice@"))
        );

        let output = render_dx7_fixture(&fixture);
        assert_eq!(output.len(), fixture.channels);
        assert_eq!(output[0].len(), fixture.frames);
        assert!(output[0].iter().all(|sample| sample.is_finite()));
        assert!(output[0].iter().any(|sample| sample.abs() > 0.0));
    }
}

#[test]
fn dx7_compatible_renderer_tolerance_checks_pass() {
    for fixture in dx7_render_fixtures() {
        let report = dx7_compatible_renderer_tolerance_report(&fixture);
        assert_eq!(report.frames, fixture.frames * fixture.channels);
        assert!(report.algorithmic_peak.is_finite());
        assert!(report.compatible_peak.is_finite());
        assert!(
            report.passed,
            "{} max={} mean={}",
            report.fixture_id, report.max_abs_delta, report.mean_abs_delta
        );
    }
}

#[test]
fn dx7_accurate_renderer_gate_records_oracle_status() {
    let gate = dx7_accurate_renderer_gate();
    assert_eq!(gate.renderer, ComponentBackend::Modeled);
    assert_eq!(
        gate.status,
        Dx7RendererAccuracyStatus::IncompleteExactOraclePending
    );
    assert!(!gate.exact_oracle_ready());
    assert_eq!(gate.required_fixture_ids, dx7_render_fixture_ids());
    assert!(
        gate.required_fixture_ids
            .iter()
            .any(|id| id == "dx7-synthetic-bank-load-smoke")
    );
}

#[test]
fn dx7_synthetic_bank_load_smoke_records_all_patch_hashes() {
    let bank = dx7_synthetic_bank_fixture();
    assert_eq!(bank.id, "dx7-synthetic-bank-load-smoke");
    assert_eq!(bank.patches.len(), DX7_ALGORITHM_COUNT);
    assert_eq!(bank.patch_hashes.len(), DX7_ALGORITHM_COUNT);
    assert_eq!(
        bank.user_sysex_path,
        "$HOME/.local/share/sim/dx7/user-bank.syx"
    );
    assert_eq!(
        bank.patches
            .iter()
            .map(|patch| patch.algorithm)
            .collect::<Vec<_>>(),
        (1..=DX7_ALGORITHM_COUNT as u8).collect::<Vec<_>>()
    );
    let unique_hashes = bank.patch_hashes.iter().collect::<BTreeSet<_>>();
    assert_eq!(unique_hashes.len(), DX7_ALGORITHM_COUNT);
}

#[test]
fn dx7_fixture_manifest_matches_regenerated_output() {
    assert_eq!(
        dx7_fixture_regeneration_command(),
        DX7_FIXTURE_REGENERATE_COMMAND
    );
    assert_eq!(
        include_str!("../../fixtures/dx7/render-fixtures.toml"),
        dx7_render_fixture_manifest()
    );
}

fn probe_patch(algorithm: u8) -> Dx7Patch {
    let operators = (0..DX7_OPERATOR_COUNT)
        .map(|index| Dx7PatchOperator {
            output_level: 45 + index as u8 * 7,
            frequency_coarse: 1 + index as u8,
            frequency_fine: index as u8 * 5,
            key_velocity_sens: index as u8 % 7,
            amp_mod_sens: index as u8 % 4,
            ..Dx7PatchOperator::default()
        })
        .collect::<Vec<_>>();
    Dx7Patch {
        name: format!("probe-{algorithm:02}"),
        operators,
        algorithm,
        feedback: algorithm % 8,
        lfo: Dx7Lfo {
            pitch_mod_depth: 8,
            amp_mod_depth: 4,
            pitch_mod_sens: 2,
            ..Dx7Lfo::default()
        },
        raw: Dx7RawPatch::default(),
        ..Dx7Patch::default()
    }
}

fn render_voice(voice: &mut Dx7Voice, events: &[BlockEvent<'_>], frames: u32) -> Vec<f32> {
    let mut output = vec![0.0; frames as usize];
    {
        let in_audio: [&[f32]; 0] = [];
        let mut out_audio = [output.as_mut_slice()];
        let mut out_events = NullEventSink;
        let mut scratch = BlockArena::empty();
        let mut block = ProcessBlock {
            frames,
            in_audio: &in_audio,
            out_audio: &mut out_audio,
            in_events: events,
            out_events: &mut out_events,
            transport: Transport::default(),
            scratch: &mut scratch,
        };
        Processor::process(voice, &mut block);
    }
    output
}

fn round6(samples: &[f32]) -> Vec<i64> {
    samples
        .iter()
        .map(|sample| (sample * 1_000_000.0).round() as i64)
        .collect()
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym("audio-synth/dx7-graph", name)
}
