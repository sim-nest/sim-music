//! Discrete-component contract for the audio-synth runtime.
//!
//! A [`DiscreteComponent`] is one tick-or-block-driven DSP unit (oscillator,
//! envelope, filter, and so on). This module defines its prepare/render
//! contract ([`ComponentPrepareConfig`], [`ComponentTick`],
//! [`ComponentTickResult`]), its self-describing metadata
//! ([`ComponentDescriptor`], [`ComponentInspection`]), and the helpers that
//! project a component's ports and parameters into the editor descriptor
//! [`Expr`] consumed by the component-editor surface.

use sim_kernel::{Expr, Symbol};
use sim_lib_audio_graph_core::{ClockDomain, LatencyClass, PrepareConfig, ProcessBlock};
use sim_value::build::{float, int, map, text, vector};

use crate::{
    ComponentBackend, ComponentParamDescriptor, ComponentParamRange, ComponentPortDescriptor,
    ComponentTraceFrame,
};

/// Configuration handed to a component at prepare time, describing the audio
/// graph's sample rate, block size, and channel counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentPrepareConfig {
    /// Sample rate of the host graph, in Hz (clamped to at least 1).
    pub sample_rate_hz: u32,
    /// Maximum number of frames in any single render block (at least 1).
    pub max_block_frames: u32,
    /// Number of input channels feeding the component.
    pub in_channels: u16,
    /// Number of output channels the component writes (at least 1).
    pub out_channels: u16,
}

impl ComponentPrepareConfig {
    /// Builds a prepare config, clamping the sample rate, block size, and
    /// output channel count to sane minimums.
    pub fn new(
        sample_rate_hz: u32,
        max_block_frames: u32,
        in_channels: u16,
        out_channels: u16,
    ) -> Self {
        Self {
            sample_rate_hz: sample_rate_hz.max(1),
            max_block_frames: max_block_frames.max(1),
            in_channels,
            out_channels: out_channels.max(1),
        }
    }
}

impl From<PrepareConfig> for ComponentPrepareConfig {
    fn from(config: PrepareConfig) -> Self {
        Self::new(
            config.sample_rate_hz,
            config.max_block_frames,
            config.in_channels,
            config.out_channels,
        )
    }
}

impl From<ComponentPrepareConfig> for PrepareConfig {
    fn from(config: ComponentPrepareConfig) -> Self {
        PrepareConfig::new(
            config.sample_rate_hz,
            config.max_block_frames,
            config.in_channels,
            config.out_channels,
        )
    }
}

/// A single-sample tick presented to a component's [`DiscreteComponent::tick`]
/// path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComponentTick {
    /// Absolute frame index of this tick since prepare.
    pub frame: u64,
    /// Whether the gate (note-on) is currently asserted.
    pub gate: bool,
    /// Input sample value for this frame.
    pub input: f32,
}

/// Result produced by ticking a component for one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComponentTickResult {
    /// Output sample value produced for the frame.
    pub output: f32,
    /// Optional diagnostic trace frame captured during the tick.
    pub trace: Option<ComponentTraceFrame>,
}

/// Live snapshot of a component's identity and current field values for
/// inspection and editor display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentInspection {
    component: Symbol,
    backend: ComponentBackend,
    active: bool,
    fields: Vec<(Symbol, String)>,
}

impl ComponentInspection {
    /// Starts an inspection for the named component and backend, marking whether
    /// it is currently active.
    pub fn new(component: Symbol, backend: ComponentBackend, active: bool) -> Self {
        Self {
            component,
            backend,
            active,
            fields: Vec::new(),
        }
    }

    /// Appends a named string field to the inspection, returning `self` for
    /// chaining.
    pub fn with_field(mut self, key: Symbol, value: impl Into<String>) -> Self {
        self.fields.push((key, value.into()));
        self
    }

    /// Returns the inspected component's identity symbol.
    pub fn component(&self) -> &Symbol {
        &self.component
    }

    /// Returns the backend that implements the component.
    pub fn backend(&self) -> ComponentBackend {
        self.backend
    }

    /// Returns whether the component is currently active.
    pub fn active(&self) -> bool {
        self.active
    }

    /// Returns the captured `(key, value)` inspection fields.
    pub fn fields(&self) -> &[(Symbol, String)] {
        &self.fields
    }
}

/// Static description of a component: its identity, backend, scheduling
/// metadata, ports, and parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentDescriptor {
    component: Symbol,
    backend: ComponentBackend,
    clock_domain: ClockDomain,
    latency_class: LatencyClass,
    realtime_pin: bool,
    ports: Vec<ComponentPortDescriptor>,
    params: Vec<ComponentParamDescriptor>,
}

impl ComponentDescriptor {
    /// Assembles a descriptor from a component's identity, backend, clock and
    /// latency metadata, realtime pin flag, ports, and parameters.
    pub fn new(
        component: Symbol,
        backend: ComponentBackend,
        clock_domain: ClockDomain,
        latency_class: LatencyClass,
        realtime_pin: bool,
        ports: Vec<ComponentPortDescriptor>,
        params: Vec<ComponentParamDescriptor>,
    ) -> Self {
        Self {
            component,
            backend,
            clock_domain,
            latency_class,
            realtime_pin,
            ports,
            params,
        }
    }

    /// Returns the component's identity symbol.
    pub fn component(&self) -> &Symbol {
        &self.component
    }

    /// Returns the backend that implements the component.
    pub fn backend(&self) -> ComponentBackend {
        self.backend
    }

    /// Returns the clock domain the component runs in.
    pub fn clock_domain(&self) -> ClockDomain {
        self.clock_domain
    }

    /// Returns the component's latency class.
    pub fn latency_class(&self) -> LatencyClass {
        self.latency_class
    }

    /// Returns whether the component is pinned to the realtime thread.
    pub fn realtime_pin(&self) -> bool {
        self.realtime_pin
    }

    /// Returns the component's port descriptors.
    pub fn ports(&self) -> &[ComponentPortDescriptor] {
        &self.ports
    }

    /// Returns the component's parameter descriptors.
    pub fn params(&self) -> &[ComponentParamDescriptor] {
        &self.params
    }
}

/// Returns the tag symbol that marks an audio-synth component editor descriptor
/// map.
pub fn component_editor_descriptor_tag() -> Symbol {
    Symbol::qualified("audio-synth", "component-editor-descriptor")
}

/// Builds the editor descriptor [`Expr`] for a component from its identity,
/// category, wrapper, capabilities, ports, parameters, and trace routing.
///
/// The `identity` pair is the component symbol and its display name; `routing`
/// carries whether a trace view is available and the optional specialized-view
/// symbol.
pub fn component_editor_descriptor_expr(
    identity: (&Symbol, &str),
    category: Symbol,
    wrapper: Symbol,
    capabilities: Vec<Symbol>,
    ports: &[ComponentPortDescriptor],
    params: &[ComponentParamDescriptor],
    routing: (bool, Option<Symbol>),
) -> Expr {
    map(vec![
        ("tag", Expr::Symbol(component_editor_descriptor_tag())),
        ("component", Expr::Symbol(identity.0.clone())),
        ("name", text(identity.1)),
        ("category", Expr::Symbol(category)),
        ("wrapper", Expr::Symbol(wrapper)),
        (
            "capabilities",
            vector(capabilities.into_iter().map(Expr::Symbol).collect()),
        ),
        (
            "ports",
            vector(ports.iter().map(port_descriptor_expr).collect()),
        ),
        ("parameter-groups", parameter_groups_expr(params)),
        ("current-values", current_values_expr(params)),
        ("validation-errors", vector(Vec::new())),
        ("trace-available", Expr::Bool(routing.0)),
        ("trace-fields", trace_fields_expr(routing.0)),
        (
            "specialized-view",
            routing.1.map(Expr::Symbol).unwrap_or(Expr::Nil),
        ),
    ])
}

fn parameter_groups_expr(params: &[ComponentParamDescriptor]) -> Expr {
    if params.is_empty() {
        return vector(Vec::new());
    }
    vector(vec![map(vec![
        (
            "name",
            Expr::Symbol(Symbol::qualified("audio-synth/param-group", "main")),
        ),
        ("label", text("Parameters")),
        (
            "params",
            vector(params.iter().map(param_descriptor_expr).collect()),
        ),
    ])])
}

fn port_descriptor_expr(port: &ComponentPortDescriptor) -> Expr {
    let rate = port.rate_contract();
    map(vec![
        ("id", Expr::Symbol(port.id().clone())),
        ("media", Expr::Symbol(port.media().symbol())),
        ("direction", Expr::Symbol(port.direction().symbol())),
        ("channels", int(i64::from(port.channels()))),
        ("required", Expr::Bool(port.required())),
        (
            "rate-contract",
            map(vec![
                ("clock-domain", Expr::Symbol(rate.clock_domain().symbol())),
                ("latency-class", Expr::Symbol(rate.latency_class().symbol())),
                (
                    "nominal-rate-hz",
                    rate.nominal_rate_hz()
                        .map(|rate| int(i64::from(rate)))
                        .unwrap_or(Expr::Nil),
                ),
            ]),
        ),
    ])
}

fn param_descriptor_expr(param: &ComponentParamDescriptor) -> Expr {
    map(vec![
        ("id", Expr::Symbol(param.id().clone())),
        ("label", text(param.label())),
        ("unit", Expr::Symbol(param.unit().symbol())),
        ("editor", Expr::Symbol(param_editor_symbol(param))),
        ("range", param.range().map(range_expr).unwrap_or(Expr::Nil)),
        (
            "enum-values",
            vector(
                param
                    .enum_values()
                    .iter()
                    .cloned()
                    .map(Expr::Symbol)
                    .collect(),
            ),
        ),
        ("value", param_default_expr(param)),
        ("normalized-value", float(param.normalized_default())),
        (
            "raw-default",
            param.raw_default().map(int).unwrap_or(Expr::Nil),
        ),
        ("read-only", Expr::Bool(false)),
    ])
}

fn param_editor_symbol(param: &ComponentParamDescriptor) -> Symbol {
    let editor = if !param.enum_values().is_empty() {
        "enum"
    } else if param.raw_default().is_some() || param.unit() == crate::ComponentParamUnit::RawInteger
    {
        "integer-range"
    } else {
        "normalized"
    };
    Symbol::qualified("component-editor/editor", editor)
}

fn range_expr(range: ComponentParamRange) -> Expr {
    map(vec![
        ("min", float(range.min())),
        ("max", float(range.max())),
        ("default", float(range.default())),
    ])
}

fn param_default_expr(param: &ComponentParamDescriptor) -> Expr {
    if let Some(raw) = param.raw_default() {
        int(raw)
    } else if let Some(range) = param.range() {
        float(range.default())
    } else {
        float(param.normalized_default())
    }
}

fn current_values_expr(params: &[ComponentParamDescriptor]) -> Expr {
    Expr::Map(
        params
            .iter()
            .map(|param| (Expr::Symbol(param.id().clone()), param_default_expr(param)))
            .collect(),
    )
}

fn trace_fields_expr(trace_available: bool) -> Expr {
    if !trace_available {
        return vector(Vec::new());
    }
    vector(vec![map(vec![
        (
            "id",
            Expr::Symbol(Symbol::qualified("audio-synth/trace", "latest-frame")),
        ),
        ("label", text("Latest trace frame")),
        (
            "editor",
            Expr::Symbol(Symbol::qualified(
                "component-editor/editor",
                "trace-readonly",
            )),
        ),
        ("value", text("available")),
        ("read-only", Expr::Bool(true)),
    ])])
}

/// A single discrete DSP component that can be prepared, reset, and rendered as
/// part of an audio graph.
///
/// Implementors describe their identity, backend, ports, and parameters, then
/// process audio either per block via [`render`](DiscreteComponent::render) or
/// per sample via [`tick`](DiscreteComponent::tick). Scheduling metadata
/// ([`clock_domain`](DiscreteComponent::clock_domain),
/// [`latency_class`](DiscreteComponent::latency_class),
/// [`realtime_pin`](DiscreteComponent::realtime_pin)) and tracing have defaults.
pub trait DiscreteComponent: Send {
    /// Returns the component's identity symbol.
    fn component_id(&self) -> Symbol;

    /// Returns the backend that implements the component.
    fn backend(&self) -> ComponentBackend;

    /// Returns the component's port descriptors.
    fn ports(&self) -> Vec<ComponentPortDescriptor>;

    /// Returns the component's parameter descriptors.
    fn params(&self) -> Vec<ComponentParamDescriptor>;

    /// Clears internal state back to its initial condition.
    fn reset(&mut self);

    /// Prepares the component for the given sample rate, block size, and channel
    /// layout before rendering begins.
    fn prepare(&mut self, config: ComponentPrepareConfig);

    /// Renders one block of audio into `block`.
    fn render(&mut self, block: &mut ProcessBlock<'_>);

    /// Returns the clock domain the component runs in; defaults to
    /// `ClockDomain::Sample`.
    fn clock_domain(&self) -> ClockDomain {
        ClockDomain::Sample
    }

    /// Returns the component's latency class; defaults to
    /// `LatencyClass::BlockLocal`.
    fn latency_class(&self) -> LatencyClass {
        LatencyClass::BlockLocal
    }

    /// Returns whether the component must run on the realtime thread; defaults
    /// to `true`.
    fn realtime_pin(&self) -> bool {
        true
    }

    /// Builds the component's static [`ComponentDescriptor`] from its identity,
    /// backend, scheduling metadata, ports, and parameters.
    fn descriptor(&self) -> ComponentDescriptor {
        ComponentDescriptor::new(
            self.component_id(),
            self.backend(),
            self.clock_domain(),
            self.latency_class(),
            self.realtime_pin(),
            self.ports(),
            self.params(),
        )
    }

    /// Advances the component by one frame given a [`ComponentTick`]; the
    /// default returns a silent [`ComponentTickResult`].
    fn tick(&mut self, _tick: ComponentTick) -> ComponentTickResult {
        ComponentTickResult::default()
    }

    /// Returns a live [`ComponentInspection`] of the component's current state.
    fn inspect(&self) -> ComponentInspection;

    /// Returns the most recent diagnostic trace frame, or `None` when tracing is
    /// off; defaults to `None`.
    fn trace(&self) -> Option<ComponentTraceFrame> {
        None
    }
}
