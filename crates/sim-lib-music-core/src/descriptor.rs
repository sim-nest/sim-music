use std::collections::BTreeSet;

use sim_kernel::{Expr, Symbol};
use sim_lib_stream_core::RateContract;

use crate::{LaneDescriptor, LaneKind};

const DESCRIPTOR_NS: &str = "music/component-descriptor";

/// A capability a music component advertises to the runtime.
///
/// Capabilities describe what a component can do (be played, drive other
/// components, render, and so on) and are carried in a
/// [`MusicComponentDescriptor`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MusicCapability {
    /// The component can be played as part of a performance.
    Playable,
    /// The component plays incoming material (a player-family component).
    Player,
    /// The component produces a control/modulation signal.
    Modulator,
    /// The component generates an oscillating signal.
    Oscillator,
    /// The component is a source of live performance events.
    PerformanceSource,
    /// The component can be rendered to output.
    Renderable,
}

impl MusicCapability {
    /// Returns the stable wire label for this capability.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_music_core::MusicCapability;
    ///
    /// assert_eq!(MusicCapability::Playable.wire_label(), "playable");
    /// assert_eq!(MusicCapability::PerformanceSource.wire_label(), "performance-source");
    /// ```
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::Playable => "playable",
            Self::Player => "player",
            Self::Modulator => "modulator",
            Self::Oscillator => "oscillator",
            Self::PerformanceSource => "performance-source",
            Self::Renderable => "renderable",
        }
    }

    /// Returns the qualified `music/capability` symbol for this capability.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("music/capability", self.wire_label())
    }
}

/// The broad category a music component belongs to.
///
/// Categories group descriptors for browsing and routing; they are carried in
/// a [`MusicComponentDescriptor`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MusicComponentCategory {
    /// A player-family component that interprets incoming material.
    PlayerFamily,
    /// An instrument that renders events to audio.
    Instrument,
    /// A control component such as a modulator or performance source.
    Control,
}

impl MusicComponentCategory {
    /// Returns the stable wire label for this category.
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::PlayerFamily => "player-family",
            Self::Instrument => "instrument",
            Self::Control => "control",
        }
    }

    /// Returns the qualified `music/component-category` symbol for this category.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("music/component-category", self.wire_label())
    }
}

/// The direction of a component port relative to the component.
///
/// Carried in a [`MusicPortDescriptor`] to mark whether a port consumes or
/// produces material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MusicPortDirection {
    /// The port receives material into the component.
    Input,
    /// The port emits material from the component.
    Output,
    /// The port receives auxiliary side-chain material.
    Sidechain,
}

impl MusicPortDirection {
    /// Returns the stable wire label for this direction.
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
            Self::Sidechain => "sidechain",
        }
    }

    /// Returns the qualified `music/port-direction` symbol for this direction.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("music/port-direction", self.wire_label())
    }
}

/// The unit a parameter value is measured in.
///
/// Carried in a [`MusicParamDescriptor`] to describe how a parameter's value
/// should be interpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MusicUnit {
    /// No unit; the value is dimensionless or categorical.
    None,
    /// Measured in beats.
    Beats,
    /// Measured in MIDI ticks.
    Ticks,
    /// Measured as a percentage.
    Percent,
    /// Measured in semitones.
    Semitone,
    /// Measured in hertz.
    Hertz,
}

impl MusicUnit {
    /// Returns the stable wire label for this unit.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_music_core::MusicUnit;
    ///
    /// assert_eq!(MusicUnit::None.wire_label(), "none");
    /// assert_eq!(MusicUnit::Hertz.wire_label(), "hertz");
    /// ```
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Beats => "beats",
            Self::Ticks => "ticks",
            Self::Percent => "percent",
            Self::Semitone => "semitone",
            Self::Hertz => "hertz",
        }
    }

    /// Returns the qualified `music/unit` symbol for this unit.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("music/unit", self.wire_label())
    }
}

/// How reproducible a component's output is.
///
/// Carried in a [`MusicComponentDescriptor`] to declare whether output is
/// fixed, seed-driven, or dependent on live input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeterminismPolicy {
    /// Output is fully determined by the inputs.
    Deterministic,
    /// Output is reproducible given a seed.
    Seeded,
    /// Output depends on live input and is not reproducible.
    LiveInput,
}

impl DeterminismPolicy {
    /// Returns the stable wire label for this policy.
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::Seeded => "seeded",
            Self::LiveInput => "live-input",
        }
    }

    /// Returns the qualified `music/determinism` symbol for this policy.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("music/determinism", self.wire_label())
    }
}

/// Describes a single input or output port of a music component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicPortDescriptor {
    /// Stable identifier for the port.
    pub id: Symbol,
    /// Human-readable label.
    pub label: String,
    /// Whether the port is an input, output, or side-chain.
    pub direction: MusicPortDirection,
    /// The rate contract the port operates at.
    pub rate: RateContract,
    /// Lane kinds the port accepts as input.
    pub accepted_event_families: Vec<LaneKind>,
    /// Lane kinds the port emits as output.
    pub output_families: Vec<LaneKind>,
}

impl MusicPortDescriptor {
    /// Creates a port descriptor with no accepted or output families.
    pub fn new(
        id: Symbol,
        label: impl Into<String>,
        direction: MusicPortDirection,
        rate: RateContract,
    ) -> Self {
        Self {
            id,
            label: label.into(),
            direction,
            rate,
            accepted_event_families: Vec::new(),
            output_families: Vec::new(),
        }
    }

    /// Sets the accepted and output lane kinds, sorting and deduplicating each.
    pub fn with_events(mut self, accepted: Vec<LaneKind>, output: Vec<LaneKind>) -> Self {
        self.accepted_event_families = stable_lane_kinds(accepted);
        self.output_families = stable_lane_kinds(output);
        self
    }

    /// Renders the port descriptor as an `Expr` map for wire transport.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            field("id", Expr::Symbol(self.id.clone())),
            field("label", Expr::String(self.label.clone())),
            field("direction", Expr::Symbol(self.direction.symbol())),
            field("rate", rate_expr(self.rate)),
            field("accepted", lane_kind_list(&self.accepted_event_families)),
            field("output", lane_kind_list(&self.output_families)),
        ])
    }
}

/// Describes a single configurable parameter of a music component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicParamDescriptor {
    /// Stable identifier for the parameter.
    pub id: Symbol,
    /// Human-readable label.
    pub label: String,
    /// The unit the parameter value is measured in.
    pub unit: MusicUnit,
    /// The rate contract the parameter is updated at.
    pub rate: RateContract,
    /// The default value for the parameter.
    pub default: Expr,
}

impl MusicParamDescriptor {
    /// Creates a parameter descriptor from its fields.
    pub fn new(
        id: Symbol,
        label: impl Into<String>,
        unit: MusicUnit,
        rate: RateContract,
        default: Expr,
    ) -> Self {
        Self {
            id,
            label: label.into(),
            unit,
            rate,
            default,
        }
    }

    /// Renders the parameter descriptor as an `Expr` map for wire transport.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            field("id", Expr::Symbol(self.id.clone())),
            field("label", Expr::String(self.label.clone())),
            field("unit", Expr::Symbol(self.unit.symbol())),
            field("rate", rate_expr(self.rate)),
            field("default", self.default.clone()),
        ])
    }
}

/// Full description of a music component: its identity, category,
/// capabilities, ports, lanes, parameters, and behavioral contracts.
///
/// Built fluently via [`MusicComponentDescriptor::new`] and the `with_*`
/// methods, then rendered for transport with
/// [`MusicComponentDescriptor::to_expr`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicComponentDescriptor {
    /// Stable identifier for the component.
    pub id: Symbol,
    /// Human-readable label.
    pub label: String,
    /// The category the component belongs to.
    pub category: MusicComponentCategory,
    /// Capabilities the component advertises.
    pub capabilities: BTreeSet<MusicCapability>,
    /// Input and output ports, ordered by id.
    pub ports: Vec<MusicPortDescriptor>,
    /// Lane descriptors produced by the component, in stable order.
    pub lanes: Vec<LaneDescriptor>,
    /// Configurable parameters, ordered by id.
    pub params: Vec<MusicParamDescriptor>,
    /// The rate contract the component runs at.
    pub rate: RateContract,
    /// How reproducible the component's output is.
    pub determinism: DeterminismPolicy,
    /// Lane kinds the component accepts as input.
    pub accepted_event_families: Vec<LaneKind>,
    /// Lane kinds the component emits as output.
    pub output_families: Vec<LaneKind>,
    /// The latency class symbol derived from the rate contract.
    pub latency: Symbol,
    /// Whether the component has a working implementation.
    pub implemented: bool,
}

impl MusicComponentDescriptor {
    /// Creates a descriptor with default capabilities, ports, lanes, and params.
    ///
    /// The latency symbol is derived from `rate` and `implemented` defaults to
    /// `true`.
    pub fn new(
        id: Symbol,
        label: impl Into<String>,
        category: MusicComponentCategory,
        rate: RateContract,
    ) -> Self {
        let latency = rate.latency_class().symbol();
        Self {
            id,
            label: label.into(),
            category,
            capabilities: BTreeSet::new(),
            ports: Vec::new(),
            lanes: Vec::new(),
            params: Vec::new(),
            rate,
            determinism: DeterminismPolicy::Deterministic,
            accepted_event_families: Vec::new(),
            output_families: Vec::new(),
            latency,
            implemented: true,
        }
    }

    /// Adds a capability and returns the updated descriptor.
    pub fn with_capability(mut self, capability: MusicCapability) -> Self {
        self.capabilities.insert(capability);
        self
    }

    /// Adds a port and re-sorts the port list by id.
    pub fn with_port(mut self, port: MusicPortDescriptor) -> Self {
        self.ports.push(port);
        self.ports.sort_by(|left, right| left.id.cmp(&right.id));
        self
    }

    /// Adds a lane and restores the stable lane order.
    pub fn with_lane(mut self, lane: LaneDescriptor) -> Self {
        self.lanes.push(lane);
        self.lanes = crate::stable_lane_order(self.lanes);
        self
    }

    /// Adds a parameter and re-sorts the parameter list by id.
    pub fn with_param(mut self, param: MusicParamDescriptor) -> Self {
        self.params.push(param);
        self.params.sort_by(|left, right| left.id.cmp(&right.id));
        self
    }

    /// Sets the accepted and output lane kinds, sorting and deduplicating each.
    pub fn with_events(mut self, accepted: Vec<LaneKind>, output: Vec<LaneKind>) -> Self {
        self.accepted_event_families = stable_lane_kinds(accepted);
        self.output_families = stable_lane_kinds(output);
        self
    }

    /// Sets the determinism policy and returns the updated descriptor.
    pub fn with_determinism(mut self, determinism: DeterminismPolicy) -> Self {
        self.determinism = determinism;
        self
    }

    /// Sets the implemented flag and returns the updated descriptor.
    pub fn with_implemented(mut self, implemented: bool) -> Self {
        self.implemented = implemented;
        self
    }

    /// Returns `true` if the descriptor advertises `capability`.
    pub fn has_capability(&self, capability: MusicCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// Renders the full descriptor as an `Expr` map for wire transport.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            field("id", Expr::Symbol(self.id.clone())),
            field("label", Expr::String(self.label.clone())),
            field("category", Expr::Symbol(self.category.symbol())),
            field(
                "capabilities",
                Expr::Vector(
                    self.capabilities
                        .iter()
                        .map(|capability| Expr::Symbol(capability.symbol()))
                        .collect(),
                ),
            ),
            field(
                "ports",
                Expr::Vector(
                    self.ports
                        .iter()
                        .map(MusicPortDescriptor::to_expr)
                        .collect(),
                ),
            ),
            field(
                "lanes",
                Expr::Vector(self.lanes.iter().map(lane_expr).collect()),
            ),
            field(
                "params",
                Expr::Vector(
                    self.params
                        .iter()
                        .map(MusicParamDescriptor::to_expr)
                        .collect(),
                ),
            ),
            field("rate", rate_expr(self.rate)),
            field("determinism", Expr::Symbol(self.determinism.symbol())),
            field("accepted", lane_kind_list(&self.accepted_event_families)),
            field("output", lane_kind_list(&self.output_families)),
            field("latency", Expr::Symbol(self.latency.clone())),
            field("implemented", Expr::Bool(self.implemented)),
        ])
    }
}

fn stable_lane_kinds(mut kinds: Vec<LaneKind>) -> Vec<LaneKind> {
    kinds.sort();
    kinds.dedup();
    kinds
}

fn lane_kind_list(kinds: &[LaneKind]) -> Expr {
    Expr::Vector(
        kinds
            .iter()
            .map(|kind| Expr::Symbol(kind.symbol()))
            .collect(),
    )
}

fn lane_expr(lane: &LaneDescriptor) -> Expr {
    Expr::Map(vec![
        field("id", Expr::String(lane.id.0.clone())),
        field("kind", Expr::Symbol(lane.kind.symbol())),
        field("target", Expr::Symbol(lane.target.symbol())),
        field("order", Expr::String(lane.order.to_string())),
    ])
}

pub(crate) fn rate_expr(rate: RateContract) -> Expr {
    Expr::Map(vec![
        field("clock-domain", Expr::Symbol(rate.clock_domain().symbol())),
        field("latency-class", Expr::Symbol(rate.latency_class().symbol())),
        field(
            "nominal-rate-hz",
            Expr::String(
                rate.nominal_rate_hz()
                    .map(|rate| rate.to_string())
                    .unwrap_or_else(|| "none".to_owned()),
            ),
        ),
    ])
}

fn field(name: &'static str, value: Expr) -> (Expr, Expr) {
    (Expr::Symbol(Symbol::qualified(DESCRIPTOR_NS, name)), value)
}
