use sim_kernel::Symbol;
use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};

use crate::{
    ComponentBackend, ComponentGraphEndpoint, ComponentInspection, ComponentParamDescriptor,
    ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia, ComponentPrepareConfig,
    ComponentTickResult, DiscreteComponent, DiscreteComponentGraph, discrete_component_graph_id,
};

#[test]
fn discrete_component_graph_hosts_connected_nodes() {
    let mut graph = DiscreteComponentGraph::new(Symbol::qualified("audio-synth", "gain-chain"));
    graph
        .add_node("a", Box::new(GainComponent::new(2.0)), 1, 1)
        .unwrap();
    graph
        .add_node("b", Box::new(GainComponent::new(3.0)), 1, 1)
        .unwrap();
    graph
        .connect(
            ComponentGraphEndpoint::new("a", 0),
            ComponentGraphEndpoint::new("b", 0),
        )
        .unwrap();
    graph
        .prepare_graph(ComponentPrepareConfig::new(48_000, 8, 1, 1))
        .unwrap();

    let output = graph.process_offline(&[vec![0.25, -0.5, 1.0]], 3).unwrap();

    assert_eq!(round4(&output[0]), vec![1.5, -3.0, 6.0]);
    assert_eq!(graph.component_id(), discrete_component_graph_id());
    assert_eq!(graph.inspect().fields().len(), 2);
}

#[test]
fn discrete_component_graph_implements_processor_surface() {
    let mut graph = DiscreteComponentGraph::new(Symbol::qualified("audio-synth", "gain"));
    graph
        .add_node("gain", Box::new(GainComponent::new(0.5)), 1, 1)
        .unwrap();
    Processor::prepare(&mut graph, PrepareConfig::new(48_000, 4, 1, 1));

    let input = [vec![1.0, -0.5, 0.25, 0.0]];
    let input_refs = input.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let mut output = [vec![0.0; 4]];
    let mut output_refs = output.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>();
    let mut arena = sim_lib_audio_graph_core::BlockArena::with_f32_capacity(4);
    let mut events = sim_lib_audio_graph_core::NullEventSink;
    let in_events = [];
    let mut block = ProcessBlock {
        frames: 4,
        in_audio: input_refs.as_slice(),
        out_audio: output_refs.as_mut_slice(),
        in_events: &in_events,
        out_events: &mut events,
        transport: sim_lib_audio_graph_core::Transport::default(),
        scratch: &mut arena,
    };

    graph.process(&mut block);

    assert_eq!(round4(&output[0]), vec![0.5, -0.25, 0.125, 0.0]);
    assert!(graph.last_error().is_none());
}

#[test]
fn discrete_component_graph_rejects_mismatched_port_rate_contracts() {
    let mut graph = DiscreteComponentGraph::new(Symbol::qualified("audio-synth", "bad-rate"));
    graph
        .add_node("control", Box::new(ControlComponent), 0, 1)
        .unwrap();
    graph
        .add_node("gain", Box::new(GainComponent::new(1.0)), 1, 1)
        .unwrap();

    let error = graph
        .connect(
            ComponentGraphEndpoint::new("control", 0),
            ComponentGraphEndpoint::new("gain", 0),
        )
        .expect_err("control to audio edge must be rejected");

    assert!(error.to_string().contains("incompatible port rate"));
}

struct GainComponent {
    gain: f32,
}

impl GainComponent {
    fn new(gain: f32) -> Self {
        Self { gain }
    }
}

impl DiscreteComponent for GainComponent {
    fn component_id(&self) -> Symbol {
        Symbol::qualified("test", "GainComponent")
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        vec![
            ComponentPortDescriptor::new(
                Symbol::qualified("test/port", "audio-in"),
                ComponentPortMedia::AudioRate,
                ComponentPortDirection::Input,
                1,
            ),
            ComponentPortDescriptor::new(
                Symbol::qualified("test/port", "audio-out"),
                ComponentPortMedia::AudioRate,
                ComponentPortDirection::Output,
                1,
            ),
        ]
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        Vec::new()
    }

    fn reset(&mut self) {}

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for (input, output) in block.in_audio.iter().zip(block.out_audio.iter_mut()) {
            for frame in 0..frames {
                output[frame] = input[frame] * self.gain;
            }
        }
    }

    fn tick(&mut self, _tick: crate::ComponentTick) -> ComponentTickResult {
        ComponentTickResult::default()
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(self.component_id(), ComponentBackend::Algorithmic, true)
    }
}

struct ControlComponent;

impl DiscreteComponent for ControlComponent {
    fn component_id(&self) -> Symbol {
        Symbol::qualified("test", "ControlComponent")
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        vec![ComponentPortDescriptor::new(
            Symbol::qualified("test/port", "control-out"),
            ComponentPortMedia::ControlRate,
            ComponentPortDirection::Output,
            1,
        )]
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        Vec::new()
    }

    fn reset(&mut self) {}

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, _block: &mut ProcessBlock<'_>) {}

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(self.component_id(), ComponentBackend::Algorithmic, true)
    }
}

fn round4(values: &[f32]) -> Vec<f32> {
    values
        .iter()
        .map(|value| (value * 10_000.0).round() / 10_000.0)
        .collect()
}
