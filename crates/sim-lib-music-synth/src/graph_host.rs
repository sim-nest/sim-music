use std::collections::{BTreeMap, VecDeque};

use sim_kernel::{Error, Result, Symbol};
use sim_lib_audio_graph_core::{
    BlockArena, NullEventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentPortDescriptor,
    ComponentPortDirection, ComponentPortMedia, ComponentPrepareConfig, ComponentTraceFrame,
    ComponentTraceValue, DiscreteComponent, RateContract,
};

/// A reference to one audio port on a graph node: the node id and the channel
/// index within that node's input or output ports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentGraphEndpoint {
    /// The node id.
    pub node_id: String,
    /// The channel index within the node's ports.
    pub port_index: usize,
}

impl ComponentGraphEndpoint {
    /// Creates an endpoint referencing the given node and port index.
    pub fn new(node_id: impl Into<String>, port_index: usize) -> Self {
        Self {
            node_id: node_id.into(),
            port_index,
        }
    }
}

/// A directed audio connection between two graph endpoints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentGraphCable {
    /// The source endpoint (output port).
    pub from: ComponentGraphEndpoint,
    /// The destination endpoint (input port).
    pub to: ComponentGraphEndpoint,
}

impl ComponentGraphCable {
    /// Creates a cable from one endpoint to another.
    pub fn new(from: ComponentGraphEndpoint, to: ComponentGraphEndpoint) -> Self {
        Self { from, to }
    }
}

/// A host for a directed acyclic graph of discrete components, processing audio
/// in topological order. Itself a [`DiscreteComponent`].
pub struct DiscreteComponentGraph {
    id: Symbol,
    nodes: BTreeMap<String, ComponentGraphNode>,
    cables: Vec<ComponentGraphCable>,
    order: Vec<String>,
    prepared: Option<ComponentPrepareConfig>,
    arena: BlockArena,
    last_error: Option<String>,
}

struct ComponentGraphNode {
    component: Box<dyn DiscreteComponent>,
    in_channels: u16,
    out_channels: u16,
}

impl DiscreteComponentGraph {
    /// Creates an empty graph with the given id.
    pub fn new(id: Symbol) -> Self {
        Self {
            id,
            nodes: BTreeMap::new(),
            cables: Vec::new(),
            order: Vec::new(),
            prepared: None,
            arena: BlockArena::empty(),
            last_error: None,
        }
    }

    /// Returns the graph id.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the cables connecting the nodes.
    pub fn cables(&self) -> &[ComponentGraphCable] {
        &self.cables
    }

    /// Returns the most recent processing error, if any.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Adds a node with the given id, component, and channel counts, erroring
    /// on an empty or duplicate id.
    pub fn add_node(
        &mut self,
        id: impl Into<String>,
        component: Box<dyn DiscreteComponent>,
        in_channels: u16,
        out_channels: u16,
    ) -> Result<()> {
        let id = id.into();
        if id.is_empty() {
            return Err(Error::Eval(
                "discrete component graph node id cannot be empty".to_owned(),
            ));
        }
        if self.nodes.contains_key(&id) {
            return Err(Error::Eval(format!(
                "duplicate discrete component graph node: {id}"
            )));
        }
        self.nodes.insert(
            id,
            ComponentGraphNode {
                component,
                in_channels,
                out_channels: out_channels.max(1),
            },
        );
        self.order.clear();
        self.prepared = None;
        Ok(())
    }

    /// Connects an output endpoint to an input endpoint, validating both
    /// endpoints and rate compatibility and rejecting any cable that would form
    /// a cycle.
    pub fn connect(
        &mut self,
        from: ComponentGraphEndpoint,
        to: ComponentGraphEndpoint,
    ) -> Result<()> {
        let source_rate = self.validate_endpoint(&from, EndpointDirection::Output)?;
        let target_rate = self.validate_endpoint(&to, EndpointDirection::Input)?;
        source_rate.ensure_compatible(target_rate)?;
        self.cables.push(ComponentGraphCable::new(from, to));
        match self.topological_order() {
            Ok(order) => {
                self.order = order;
                self.prepared = None;
                Ok(())
            }
            Err(error) => {
                self.cables.pop();
                Err(error)
            }
        }
    }

    /// Prepares every node for processing at the given config and sizes the
    /// scratch arena, erroring when the graph contains a cycle.
    pub fn prepare_graph(&mut self, config: ComponentPrepareConfig) -> Result<()> {
        let order = self.topological_order()?;
        let mut max_channels = 1usize;
        for id in &order {
            let node = self
                .nodes
                .get_mut(id)
                .ok_or_else(|| Error::Eval(format!("missing component graph node: {id}")))?;
            max_channels = max_channels.max(usize::from(node.in_channels));
            max_channels = max_channels.max(usize::from(node.out_channels));
            node.component.prepare(ComponentPrepareConfig::new(
                config.sample_rate_hz,
                config.max_block_frames,
                node.in_channels,
                node.out_channels,
            ));
        }
        self.order = order;
        self.arena = BlockArena::with_f32_capacity(config.max_block_frames as usize * max_channels);
        self.prepared = Some(config);
        self.last_error = None;
        Ok(())
    }

    /// Processes one offline block through the graph in topological order,
    /// returning the output node's audio. Requires the graph to be prepared and
    /// errors on an unprepared, empty, or short-input graph.
    pub fn process_offline(&mut self, input: &[Vec<f32>], frames: u32) -> Result<Vec<Vec<f32>>> {
        let prepared = self.prepared.ok_or_else(|| {
            Error::Eval("discrete component graph must be prepared before processing".to_owned())
        })?;
        if frames > prepared.max_block_frames {
            return Err(Error::Eval(format!(
                "process block has {frames} frames, max prepared block is {}",
                prepared.max_block_frames
            )));
        }
        if self.order.is_empty() {
            return Err(Error::Eval(
                "discrete component graph has no nodes".to_owned(),
            ));
        }
        let frames_len = frames as usize;
        for (index, lane) in input.iter().enumerate() {
            if lane.len() < frames_len {
                return Err(Error::Eval(format!(
                    "input audio lane {index} has {} frames, expected at least {frames_len}",
                    lane.len()
                )));
            }
        }

        let mut node_outputs = BTreeMap::<String, Vec<Vec<f32>>>::new();
        for id in self.order.clone() {
            let (in_channels, out_channels) = self.node_channel_counts(&id)?;
            let mut in_buffers = vec![vec![0.0; frames_len]; usize::from(in_channels)];
            let incoming = self.incoming_edges(&id)?;
            if incoming.is_empty() {
                copy_graph_input(input, &mut in_buffers, frames_len);
            } else {
                copy_connected_inputs(&id, &incoming, &node_outputs, &mut in_buffers, frames_len)?;
            }

            let mut out_buffers = vec![vec![0.0; frames_len]; usize::from(out_channels)];
            {
                let in_audio = in_buffers.iter().map(Vec::as_slice).collect::<Vec<_>>();
                let mut out_audio = out_buffers
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>();
                let in_events = [];
                let mut out_events = NullEventSink;
                self.arena.reset();
                let mut block = ProcessBlock {
                    frames,
                    in_audio: in_audio.as_slice(),
                    out_audio: out_audio.as_mut_slice(),
                    in_events: &in_events,
                    out_events: &mut out_events,
                    transport: Transport::default(),
                    scratch: &mut self.arena,
                };
                block.validate_audio_lanes()?;
                let node = self
                    .nodes
                    .get_mut(&id)
                    .ok_or_else(|| Error::Eval(format!("missing component graph node: {id}")))?;
                node.component.render(&mut block);
            }
            node_outputs.insert(id, out_buffers);
        }

        let output_node = self
            .order
            .last()
            .ok_or_else(|| Error::Eval("discrete component graph has no output node".to_owned()))?;
        node_outputs.remove(output_node).ok_or_else(|| {
            Error::Eval(format!(
                "missing discrete component graph output for node {output_node}"
            ))
        })
    }

    fn validate_endpoint(
        &self,
        endpoint: &ComponentGraphEndpoint,
        direction: EndpointDirection,
    ) -> Result<RateContract> {
        let node = self.nodes.get(&endpoint.node_id).ok_or_else(|| {
            Error::Eval(format!(
                "unknown discrete component graph node: {}",
                endpoint.node_id
            ))
        })?;
        let limit = match direction {
            EndpointDirection::Input => node.in_channels,
            EndpointDirection::Output => node.out_channels,
        };
        if endpoint.port_index >= usize::from(limit) {
            return Err(Error::Eval(format!(
                "port index {} is out of range for node {}",
                endpoint.port_index, endpoint.node_id
            )));
        }
        endpoint_rate_contract(node, endpoint.port_index, direction)
    }

    fn node_channel_counts(&self, id: &str) -> Result<(u16, u16)> {
        self.nodes
            .get(id)
            .map(|node| (node.in_channels, node.out_channels))
            .ok_or_else(|| Error::Eval(format!("missing component graph node: {id}")))
    }

    fn incoming_edges(&self, node_id: &str) -> Result<Vec<ComponentGraphCable>> {
        Ok(self
            .cables
            .iter()
            .filter(|cable| cable.to.node_id == node_id)
            .cloned()
            .collect())
    }

    fn topological_order(&self) -> Result<Vec<String>> {
        let mut indegree = self
            .nodes
            .keys()
            .map(|id| (id.clone(), 0usize))
            .collect::<BTreeMap<_, _>>();
        let mut outgoing = BTreeMap::<String, Vec<String>>::new();
        for cable in &self.cables {
            *indegree.get_mut(&cable.to.node_id).ok_or_else(|| {
                Error::Eval(format!(
                    "missing target node in component graph cable: {}",
                    cable.to.node_id
                ))
            })? += 1;
            outgoing
                .entry(cable.from.node_id.clone())
                .or_default()
                .push(cable.to.node_id.clone());
        }

        let mut ready = indegree
            .iter()
            .filter_map(|(id, degree)| (*degree == 0).then_some(id.clone()))
            .collect::<VecDeque<_>>();
        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(id) = ready.pop_front() {
            order.push(id.clone());
            if let Some(targets) = outgoing.get(&id) {
                for target in targets {
                    let degree = indegree.get_mut(target).ok_or_else(|| {
                        Error::Eval(format!("missing component graph node: {target}"))
                    })?;
                    *degree -= 1;
                    if *degree == 0 {
                        ready.push_back(target.clone());
                    }
                }
            }
        }
        if order.len() != self.nodes.len() {
            return Err(Error::Eval(
                "discrete component graph contains a cycle".to_owned(),
            ));
        }
        Ok(order)
    }
}

impl Processor for DiscreteComponentGraph {
    fn prepare(&mut self, cfg: PrepareConfig) {
        if let Err(error) = self.prepare_graph(cfg.into()) {
            self.last_error = Some(error.to_string());
        }
    }

    fn reset(&mut self) {
        for node in self.nodes.values_mut() {
            node.component.reset();
        }
        self.last_error = None;
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for output in block.out_audio.iter_mut() {
            output[..frames].fill(0.0);
        }
        let input = block
            .in_audio
            .iter()
            .map(|lane| lane[..frames].to_vec())
            .collect::<Vec<_>>();
        match self.process_offline(&input, block.frames) {
            Ok(output) => {
                for (source, target) in output.iter().zip(block.out_audio.iter_mut()) {
                    target[..frames].copy_from_slice(&source[..frames]);
                }
                self.last_error = None;
            }
            Err(error) => {
                self.last_error = Some(error.to_string());
            }
        }
    }
}

impl DiscreteComponent for DiscreteComponentGraph {
    fn component_id(&self) -> Symbol {
        discrete_component_graph_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        discrete_component_graph_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        Vec::new()
    }

    fn reset(&mut self) {
        <Self as Processor>::reset(self);
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        if let Err(error) = self.prepare_graph(config) {
            self.last_error = Some(error.to_string());
        }
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        <Self as Processor>::process(self, block);
    }

    fn inspect(&self) -> ComponentInspection {
        let mut inspection = ComponentInspection::new(
            discrete_component_graph_id(),
            ComponentBackend::Algorithmic,
            !self.nodes.is_empty(),
        )
        .with_field(
            Symbol::qualified("audio-synth/inspect", "nodes"),
            self.nodes.len().to_string(),
        )
        .with_field(
            Symbol::qualified("audio-synth/inspect", "cables"),
            self.cables.len().to_string(),
        );
        if let Some(error) = &self.last_error {
            inspection =
                inspection.with_field(Symbol::qualified("audio-synth/inspect", "error"), error);
        }
        inspection
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        Some(
            ComponentTraceFrame::new(
                discrete_component_graph_id(),
                ComponentBackend::Algorithmic,
                0,
            )
            .with_integer(
                Symbol::qualified("audio-synth/trace", "nodes"),
                self.nodes.len() as i64,
            )
            .with_state(
                Symbol::qualified("audio-synth/trace", "last-error"),
                ComponentTraceValue::Text(self.last_error.clone().unwrap_or_default()),
            ),
        )
    }
}

/// Returns the component id of the discrete component graph.
pub fn discrete_component_graph_id() -> Symbol {
    Symbol::qualified("audio-synth", "DiscreteComponentGraph")
}

/// Returns the port descriptors for the discrete component graph: an optional
/// stereo audio input and a stereo audio output.
pub fn discrete_component_graph_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "audio-in"),
            ComponentPortMedia::AudioRate,
            ComponentPortDirection::Input,
            2,
        )
        .optional(),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "audio-out"),
            ComponentPortMedia::AudioRate,
            ComponentPortDirection::Output,
            2,
        ),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EndpointDirection {
    Input,
    Output,
}

impl EndpointDirection {
    fn component_direction(self) -> ComponentPortDirection {
        match self {
            Self::Input => ComponentPortDirection::Input,
            Self::Output => ComponentPortDirection::Output,
        }
    }
}

fn endpoint_rate_contract(
    node: &ComponentGraphNode,
    port_index: usize,
    direction: EndpointDirection,
) -> Result<RateContract> {
    let mut offset = 0usize;
    for port in node
        .component
        .ports()
        .into_iter()
        .filter(|port| port.direction() == direction.component_direction())
    {
        let next = offset + usize::from(port.channels());
        if port_index < next {
            return Ok(port.rate_contract());
        }
        offset = next;
    }
    Err(Error::Eval(format!(
        "component port descriptor missing for port index {port_index}"
    )))
}

fn copy_graph_input(input: &[Vec<f32>], in_buffers: &mut [Vec<f32>], frames_len: usize) {
    for (channel, buffer) in in_buffers.iter_mut().enumerate() {
        if let Some(source) = input.get(channel) {
            buffer.copy_from_slice(&source[..frames_len]);
        }
    }
}

fn copy_connected_inputs(
    node_id: &str,
    incoming: &[ComponentGraphCable],
    node_outputs: &BTreeMap<String, Vec<Vec<f32>>>,
    in_buffers: &mut [Vec<f32>],
    frames_len: usize,
) -> Result<()> {
    for cable in incoming {
        let source_outputs = node_outputs.get(&cable.from.node_id).ok_or_else(|| {
            Error::Eval(format!(
                "missing processed output for node {}",
                cable.from.node_id
            ))
        })?;
        let source = source_outputs
            .get(cable.from.port_index)
            .ok_or_else(|| Error::Eval("source output channel is out of range".to_owned()))?;
        let target = in_buffers.get_mut(cable.to.port_index).ok_or_else(|| {
            Error::Eval(format!(
                "target input channel {} is out of range for node {node_id}",
                cable.to.port_index
            ))
        })?;
        target.copy_from_slice(&source[..frames_len]);
    }
    Ok(())
}
