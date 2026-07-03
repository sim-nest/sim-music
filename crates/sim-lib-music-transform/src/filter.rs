use std::collections::{BTreeMap, BTreeSet};

use sim_lib_music_core::{LaneId, LaneKind, PlayEvent, TracePolicy};
use thiserror::Error;

/// A permission a custom filter must hold to perform a class of operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FilterCapability {
    /// Run rule-based bodies.
    Rule,
    /// Invoke a registered callable filter.
    Callable,
    /// Use read-eval inside a callable filter.
    ReadEval,
    /// Duplicate events.
    Clone,
    /// Rewrite event pitch, velocity, or lane.
    Rewrite,
    /// Move events to a different lane.
    Route,
    /// Attach annotations to events.
    Annotate,
    /// Quantize event times to a grid.
    Quantize,
    /// Drop events to thin a stream.
    Thin,
    /// Expand an event into time-shifted copies.
    Expand,
    /// Emit sidechain control events.
    Sidechain,
}

impl FilterCapability {
    /// Returns the stable wire label for this capability.
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::Rule => "rule",
            Self::Callable => "callable",
            Self::ReadEval => "read-eval",
            Self::Clone => "clone",
            Self::Rewrite => "rewrite",
            Self::Route => "route",
            Self::Annotate => "annotate",
            Self::Quantize => "quantize",
            Self::Thin => "thin",
            Self::Expand => "expand",
            Self::Sidechain => "sidechain",
        }
    }

    /// Parses a capability from its wire label, if recognized.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_music_transform::FilterCapability;
    ///
    /// let cap = FilterCapability::Rewrite;
    /// assert_eq!(FilterCapability::from_wire_label(cap.wire_label()), Some(cap));
    /// assert_eq!(FilterCapability::from_wire_label("nope"), None);
    /// ```
    pub fn from_wire_label(value: &str) -> Option<Self> {
        Some(match value {
            "rule" => Self::Rule,
            "callable" => Self::Callable,
            "read-eval" => Self::ReadEval,
            "clone" => Self::Clone,
            "rewrite" => Self::Rewrite,
            "route" => Self::Route,
            "annotate" => Self::Annotate,
            "quantize" => Self::Quantize,
            "thin" => Self::Thin,
            "expand" => Self::Expand,
            "sidechain" => Self::Sidechain,
            _ => return None,
        })
    }
}

/// An ordered set of [`FilterCapability`] grants.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FilterCapabilitySet {
    items: BTreeSet<FilterCapability>,
}

impl FilterCapabilitySet {
    /// Builds a set from a collection of capabilities.
    pub fn new(items: impl IntoIterator<Item = FilterCapability>) -> Self {
        Self {
            items: items.into_iter().collect(),
        }
    }

    /// Returns a set granting every capability.
    pub fn all() -> Self {
        Self::new([
            FilterCapability::Rule,
            FilterCapability::Callable,
            FilterCapability::ReadEval,
            FilterCapability::Clone,
            FilterCapability::Rewrite,
            FilterCapability::Route,
            FilterCapability::Annotate,
            FilterCapability::Quantize,
            FilterCapability::Thin,
            FilterCapability::Expand,
            FilterCapability::Sidechain,
        ])
    }

    /// Builds a set with the `Rule` capability plus the given operation grants.
    pub fn rule_ops(items: impl IntoIterator<Item = FilterCapability>) -> Self {
        let mut set = Self::new([FilterCapability::Rule]);
        set.items.extend(items);
        set
    }

    /// Returns whether the set grants the given capability.
    pub fn contains(&self, capability: FilterCapability) -> bool {
        self.items.contains(&capability)
    }

    /// Adds a capability to the set.
    pub fn insert(&mut self, capability: FilterCapability) {
        self.items.insert(capability);
    }

    /// Iterates over the granted capabilities.
    pub fn iter(&self) -> impl Iterator<Item = FilterCapability> + '_ {
        self.items.iter().copied()
    }
}

/// Determinism contract a custom filter promises to honor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeterminismPolicy {
    /// Output depends only on input; no randomness.
    Deterministic,
    /// Randomness is allowed but a seed must be supplied.
    RequiresSeed,
    /// Nondeterministic behavior is permitted.
    AllowNondeterministic,
}

impl DeterminismPolicy {
    /// Returns the stable wire label for this policy.
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::RequiresSeed => "requires-seed",
            Self::AllowNondeterministic => "allow-nondeterministic",
        }
    }

    /// Parses a policy from its wire label, if recognized.
    pub fn from_wire_label(value: &str) -> Option<Self> {
        Some(match value {
            "deterministic" => Self::Deterministic,
            "requires-seed" => Self::RequiresSeed,
            "allow-nondeterministic" => Self::AllowNondeterministic,
            _ => return None,
        })
    }
}

/// The set of lane kinds a filter port accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterShape {
    kinds: BTreeSet<LaneKind>,
}

impl FilterShape {
    /// Builds a shape from lane kinds, rejecting an empty set.
    pub fn new(kinds: impl IntoIterator<Item = LaneKind>) -> Result<Self, CustomFilterError> {
        let shape = Self {
            kinds: kinds.into_iter().collect(),
        };
        if shape.kinds.is_empty() {
            return Err(CustomFilterError::EmptyShape);
        }
        Ok(shape)
    }

    /// Returns a shape accepting every common event lane kind.
    pub fn any_event() -> Self {
        Self::new([
            LaneKind::Note,
            LaneKind::Midi,
            LaneKind::Pitch,
            LaneKind::Control,
            LaneKind::Audio,
            LaneKind::Playable,
            LaneKind::Performance,
            LaneKind::Diagnostic,
            LaneKind::Trace,
        ])
        .expect("non-empty shape")
    }

    /// Returns a shape accepting only note events.
    pub fn notes() -> Self {
        Self::new([LaneKind::Note]).expect("non-empty shape")
    }

    /// Returns whether the shape accepts the given event.
    pub fn accepts(&self, event: &PlayEvent) -> bool {
        self.kinds.contains(&event.kind())
    }

    /// Iterates over the accepted lane kinds.
    pub fn kinds(&self) -> impl Iterator<Item = LaneKind> + '_ {
        self.kinds.iter().copied()
    }
}

/// A declared, capability-gated event filter with input/output shapes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomFilter {
    /// Stable identifier of the filter.
    pub id: String,
    /// Shape of events the filter accepts.
    pub input: FilterShape,
    /// Shape of events the filter must produce.
    pub output: FilterShape,
    /// Capabilities the filter is permitted to use.
    pub capabilities: FilterCapabilitySet,
    /// Determinism contract the filter honors.
    pub determinism: DeterminismPolicy,
    /// Tracing policy for the filter's operations.
    pub trace: TracePolicy,
    /// The filter's evaluable body.
    pub body: FilterBody,
}

impl CustomFilter {
    /// Builds a filter and validates that its body's capabilities are declared.
    pub fn new(
        id: impl Into<String>,
        input: FilterShape,
        output: FilterShape,
        capabilities: FilterCapabilitySet,
        determinism: DeterminismPolicy,
        trace: TracePolicy,
        body: FilterBody,
    ) -> Result<Self, CustomFilterError> {
        let filter = Self {
            id: id.into(),
            input,
            output,
            capabilities,
            determinism,
            trace,
            body,
        };
        filter.validate_declaration()?;
        Ok(filter)
    }

    fn validate_declaration(&self) -> Result<(), CustomFilterError> {
        for capability in self.body.declared_capabilities().iter() {
            if !self.capabilities.contains(capability) {
                return Err(CustomFilterError::UndeclaredCapability(
                    capability.wire_label().to_owned(),
                ));
            }
        }
        Ok(())
    }
}

/// The evaluable body of a [`CustomFilter`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterBody {
    /// An inline list of rules evaluated in order.
    Rule(Vec<FilterRule>),
    /// A reference to a registered callable filter.
    Callable(CallableFilterRef),
}

impl FilterBody {
    fn declared_capabilities(&self) -> FilterCapabilitySet {
        match self {
            Self::Rule(rules) => {
                let mut set = FilterCapabilitySet::new([FilterCapability::Rule]);
                for rule in rules {
                    set.insert(rule.op.capability());
                }
                set
            }
            Self::Callable(_) => FilterCapabilitySet::new([FilterCapability::Callable]),
        }
    }
}

/// A `when -> op` rule: an operation applied to matching events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterRule {
    /// Predicate selecting which events the rule applies to.
    pub when: FilterPredicate,
    /// Operation performed on matching events.
    pub op: FilterOp,
}

impl FilterRule {
    /// Builds a rule from a predicate and an operation.
    pub fn new(when: FilterPredicate, op: FilterOp) -> Self {
        Self { when, op }
    }
}

/// Condition selecting which events a [`FilterRule`] acts on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterPredicate {
    /// Match every event.
    Any,
    /// Match events of a given lane kind.
    Kind(LaneKind),
    /// Match events on a specific lane.
    Lane(LaneId),
}

impl FilterPredicate {
    pub(crate) fn matches(&self, event: &PlayEvent) -> bool {
        match self {
            Self::Any => true,
            Self::Kind(kind) => event.kind() == *kind,
            Self::Lane(lane) => event.lane_id() == lane,
        }
    }
}

/// An operation a [`FilterRule`] performs on a matching event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterOp {
    /// Pass the event through unchanged.
    Accept,
    /// Drop the event.
    Reject,
    /// Emit additional copies of the event.
    Clone {
        /// Number of extra copies to emit.
        copies: u8,
    },
    /// Rewrite pitch, velocity, and optionally the lane.
    Rewrite {
        /// Target lane, or `None` to keep the current lane.
        lane: Option<LaneId>,
        /// Semitone offset added to pitched events.
        pitch_delta: i16,
        /// Velocity offset added to note events.
        velocity_delta: i16,
    },
    /// Move the event to another lane.
    Route {
        /// Destination lane.
        lane: LaneId,
    },
    /// Attach an annotation message via a trace.
    Annotate {
        /// Annotation text.
        message: String,
    },
    /// Snap the event time to a tick grid.
    Quantize {
        /// Grid size in ticks; must be positive.
        grid_ticks: i64,
    },
    /// Keep one event for every `keep_every` matches.
    Thin {
        /// Keep period; must be positive.
        keep_every: u32,
    },
    /// Expand the event into time-shifted copies.
    Expand {
        /// Number of additional copies.
        copies: u8,
        /// Tick step between successive copies.
        step_ticks: i64,
    },
    /// Emit a sidechain control event alongside the original.
    Sidechain {
        /// Lane the control event is emitted on.
        lane: LaneId,
        /// Control name suffix.
        control: String,
    },
}

impl FilterOp {
    /// Returns the capability this operation requires.
    pub fn capability(&self) -> FilterCapability {
        match self {
            Self::Accept | Self::Reject => FilterCapability::Rule,
            Self::Clone { .. } => FilterCapability::Clone,
            Self::Rewrite { .. } => FilterCapability::Rewrite,
            Self::Route { .. } => FilterCapability::Route,
            Self::Annotate { .. } => FilterCapability::Annotate,
            Self::Quantize { .. } => FilterCapability::Quantize,
            Self::Thin { .. } => FilterCapability::Thin,
            Self::Expand { .. } => FilterCapability::Expand,
            Self::Sidechain { .. } => FilterCapability::Sidechain,
        }
    }

    /// Returns the stable wire label for this operation.
    pub fn wire_label(&self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Reject => "reject",
            Self::Clone { .. } => "clone",
            Self::Rewrite { .. } => "rewrite",
            Self::Route { .. } => "route",
            Self::Annotate { .. } => "annotate",
            Self::Quantize { .. } => "quantize",
            Self::Thin { .. } => "thin",
            Self::Expand { .. } => "expand",
            Self::Sidechain { .. } => "sidechain",
        }
    }
}

/// A by-name reference to a registered callable filter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallableFilterRef {
    /// Name the callable is registered under.
    pub name: String,
}

impl CallableFilterRef {
    /// Builds a reference to a callable by name.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// A named, reusable filter definition stored in a registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallableFilterDefinition {
    /// Name the definition is registered under.
    pub name: String,
    /// Whether the definition is deterministic.
    pub deterministic: bool,
    /// Whether the definition uses read-eval.
    pub uses_read_eval: bool,
    /// Capabilities the definition requires.
    pub capabilities: FilterCapabilitySet,
    /// Rules the definition evaluates.
    pub rules: Vec<FilterRule>,
}

impl CallableFilterDefinition {
    /// Builds a deterministic definition, deriving capabilities from `rules`.
    pub fn new(name: impl Into<String>, rules: Vec<FilterRule>) -> Self {
        let mut capabilities = FilterCapabilitySet::new([FilterCapability::Callable]);
        for rule in &rules {
            capabilities.insert(rule.op.capability());
        }
        Self {
            name: name.into(),
            deterministic: true,
            uses_read_eval: false,
            capabilities,
            rules,
        }
    }

    /// Marks the definition as nondeterministic.
    pub fn nondeterministic(mut self) -> Self {
        self.deterministic = false;
        self
    }

    /// Marks the definition as using read-eval and grants that capability.
    pub fn with_read_eval(mut self) -> Self {
        self.uses_read_eval = true;
        self.capabilities.insert(FilterCapability::ReadEval);
        self
    }
}

/// Lookup table of [`CallableFilterDefinition`] values by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CallableFilterRegistry {
    callables: BTreeMap<String, CallableFilterDefinition>,
}

impl CallableFilterRegistry {
    /// Inserts a definition under its own name, replacing any prior entry.
    pub fn register(&mut self, definition: CallableFilterDefinition) {
        self.callables.insert(definition.name.clone(), definition);
    }

    /// Looks up a definition by name.
    pub fn get(&self, name: &str) -> Option<&CallableFilterDefinition> {
        self.callables.get(name)
    }
}

/// Runtime grants and seed supplied when evaluating a custom filter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterContext {
    /// Capabilities granted to the filter at run time.
    pub capabilities: FilterCapabilitySet,
    /// Optional seed satisfying a `RequiresSeed` determinism policy.
    pub seed: Option<u64>,
}

impl FilterContext {
    /// Builds a context granting the given capabilities and no seed.
    pub fn new(capabilities: FilterCapabilitySet) -> Self {
        Self {
            capabilities,
            seed: None,
        }
    }

    /// Builds a context granting every capability.
    pub fn all_capabilities() -> Self {
        Self::new(FilterCapabilitySet::all())
    }

    /// Sets the seed on the context.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }
}

/// Output of evaluating a custom filter: produced events and traces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomFilterRun {
    /// Events the filter produced.
    pub events: Vec<PlayEvent>,
    /// Traces recorded during evaluation.
    pub traces: Vec<CustomFilterTrace>,
}

/// A single recorded step of a custom filter's evaluation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomFilterTrace {
    /// Monotonic sequence number within the run.
    pub sequence: u64,
    /// Identifier of the filter that produced the trace.
    pub filter_id: String,
    /// Wire label of the operation involved.
    pub operation: &'static str,
    /// Action the operation took.
    pub action: FilterTraceAction,
    /// Event the action applied to.
    pub event: PlayEvent,
    /// Optional message (for example, an annotation).
    pub message: String,
}

/// The kind of action recorded in a [`CustomFilterTrace`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterTraceAction {
    /// An event was accepted.
    Accepted,
    /// An event was rejected.
    Rejected,
    /// An event was cloned.
    Cloned,
    /// An event was rewritten.
    Rewritten,
    /// An event was routed to another lane.
    Routed,
    /// An event was annotated.
    Annotated,
    /// An event was quantized.
    Quantized,
    /// An event was thinned out.
    Thinned,
    /// An event was expanded into copies.
    Expanded,
    /// A sidechain control event was emitted.
    Sidechained,
}

/// Error raised while building or evaluating a custom filter.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CustomFilterError {
    /// A filter shape was constructed with no lane kinds.
    #[error("custom filter shape cannot be empty")]
    EmptyShape,
    /// The body used a capability the filter did not declare.
    #[error("custom filter capability is not declared: {0}")]
    UndeclaredCapability(String),
    /// The run context lacked a required capability.
    #[error("custom filter missing capability: {0}")]
    MissingCapability(String),
    /// An event's kind was not accepted by the input or output shape.
    #[error("custom filter {phase} shape does not accept {kind} events")]
    ShapeMismatch {
        /// Phase that failed (`input` or `output`).
        phase: &'static str,
        /// Wire label of the offending event kind.
        kind: &'static str,
    },
    /// A referenced callable filter was not in the registry.
    #[error("custom filter callable is not registered: {0}")]
    MissingCallable(String),
    /// A nondeterministic callable was used without the allowing policy.
    #[error("custom filter callable is nondeterministic: {0}")]
    NondeterministicCallable(String),
    /// A seed was required by the determinism policy but not supplied.
    #[error("custom filter seed is required")]
    MissingSeed,
    /// An operation parameter was invalid (for example, a non-positive grid).
    #[error("custom filter operation is invalid: {0}")]
    InvalidOperation(String),
    /// A codec name was not supported.
    #[error("custom filter codec is unsupported: {0}")]
    UnsupportedCodec(String),
}

/// Parses a lane kind from its wire label, if recognized.
pub fn lane_kind_from_wire(value: &str) -> Option<LaneKind> {
    Some(match value {
        "note" => LaneKind::Note,
        "drum" => LaneKind::Drum,
        "scale-degree" => LaneKind::ScaleDegree,
        "midi" => LaneKind::Midi,
        "pitch" => LaneKind::Pitch,
        "control" => LaneKind::Control,
        "automation" => LaneKind::Automation,
        "audio" => LaneKind::Audio,
        "object" => LaneKind::Object,
        "playable" => LaneKind::Playable,
        "performance" => LaneKind::Performance,
        "diagnostic" => LaneKind::Diagnostic,
        "trace" => LaneKind::Trace,
        _ => return None,
    })
}

/// Checks that a codec name is supported by the custom filter surface.
pub fn ensure_custom_filter_codec(codec: &str) -> Result<(), CustomFilterError> {
    match codec {
        "lisp" | "json" => Ok(()),
        _ => Err(CustomFilterError::UnsupportedCodec(codec.to_owned())),
    }
}
