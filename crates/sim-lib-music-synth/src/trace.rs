use sim_kernel::Symbol;

use crate::ComponentBackend;

/// The role a [`ComponentTraceRecord`] plays in a component's trace frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentTraceRole {
    /// A value fed into the component this frame.
    Input,
    /// A piece of the component's internal state.
    State,
    /// A value the component produced this frame.
    Output,
    /// A clock or timing position.
    Clock,
    /// A plain integer quantity (for example a counter).
    Integer,
}

impl ComponentTraceRole {
    /// Returns the stable kebab-case identifier for this role.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::State => "state",
            Self::Output => "output",
            Self::Clock => "clock",
            Self::Integer => "integer",
        }
    }

    /// Returns the qualified symbol naming this role.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/trace-role", self.as_str())
    }
}

/// A typed value carried by a [`ComponentTraceRecord`].
#[derive(Clone, Debug, PartialEq)]
pub enum ComponentTraceValue {
    /// A floating-point value.
    Float(f64),
    /// An integer value.
    Integer(i64),
    /// A boolean value.
    Bool(bool),
    /// A text value.
    Text(String),
}

/// A single keyed observation within a [`ComponentTraceFrame`]: a role, a key,
/// and a typed value.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentTraceRecord {
    role: ComponentTraceRole,
    key: Symbol,
    value: ComponentTraceValue,
}

impl ComponentTraceRecord {
    /// Builds a record with the [`ComponentTraceRole::State`] role.
    pub fn new(key: Symbol, value: ComponentTraceValue) -> Self {
        Self::state(key, value)
    }

    /// Builds an [`ComponentTraceRole::Input`] record.
    pub fn input(key: Symbol, value: ComponentTraceValue) -> Self {
        Self::with_role(ComponentTraceRole::Input, key, value)
    }

    /// Builds a [`ComponentTraceRole::State`] record.
    pub fn state(key: Symbol, value: ComponentTraceValue) -> Self {
        Self::with_role(ComponentTraceRole::State, key, value)
    }

    /// Builds an [`ComponentTraceRole::Output`] record.
    pub fn output(key: Symbol, value: ComponentTraceValue) -> Self {
        Self::with_role(ComponentTraceRole::Output, key, value)
    }

    /// Builds a [`ComponentTraceRole::Clock`] record.
    pub fn clock(key: Symbol, value: ComponentTraceValue) -> Self {
        Self::with_role(ComponentTraceRole::Clock, key, value)
    }

    /// Builds a [`ComponentTraceRole::Integer`] record wrapping `value`.
    pub fn integer(key: Symbol, value: i64) -> Self {
        Self::with_role(
            ComponentTraceRole::Integer,
            key,
            ComponentTraceValue::Integer(value),
        )
    }

    /// Builds a record with an explicit role, key, and value.
    pub fn with_role(role: ComponentTraceRole, key: Symbol, value: ComponentTraceValue) -> Self {
        Self { role, key, value }
    }

    /// Returns the record's role.
    pub fn role(&self) -> ComponentTraceRole {
        self.role
    }

    /// Returns the record's key.
    pub fn key(&self) -> &Symbol {
        &self.key
    }

    /// Returns the record's typed value.
    pub fn value(&self) -> &ComponentTraceValue {
        &self.value
    }
}

/// A trace of one component at one clock position: the component identity, its
/// backend, the clock, and an ordered list of [`ComponentTraceRecord`]s.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentTraceFrame {
    component: Symbol,
    backend: ComponentBackend,
    clock: u64,
    records: Vec<ComponentTraceRecord>,
}

impl ComponentTraceFrame {
    /// Builds an empty frame for `component` on `backend` at `clock`.
    pub fn new(component: Symbol, backend: ComponentBackend, clock: u64) -> Self {
        Self {
            component,
            backend,
            clock,
            records: Vec::new(),
        }
    }

    /// Appends a state-role record and returns the frame (builder style).
    pub fn with_record(mut self, key: Symbol, value: ComponentTraceValue) -> Self {
        self.records.push(ComponentTraceRecord::new(key, value));
        self
    }

    /// Appends an input-role record and returns the frame (builder style).
    pub fn with_input(mut self, key: Symbol, value: ComponentTraceValue) -> Self {
        self.records.push(ComponentTraceRecord::input(key, value));
        self
    }

    /// Appends a state-role record and returns the frame (builder style).
    pub fn with_state(mut self, key: Symbol, value: ComponentTraceValue) -> Self {
        self.records.push(ComponentTraceRecord::state(key, value));
        self
    }

    /// Appends an output-role record and returns the frame (builder style).
    pub fn with_output(mut self, key: Symbol, value: ComponentTraceValue) -> Self {
        self.records.push(ComponentTraceRecord::output(key, value));
        self
    }

    /// Appends a clock-role record holding `clock` (saturated into `i64`) and
    /// returns the frame (builder style).
    pub fn with_clock_position(mut self, key: Symbol, clock: u64) -> Self {
        let clock = clock.min(i64::MAX as u64) as i64;
        self.records.push(ComponentTraceRecord::clock(
            key,
            ComponentTraceValue::Integer(clock),
        ));
        self
    }

    /// Appends an integer-role record and returns the frame (builder style).
    pub fn with_integer(mut self, key: Symbol, value: i64) -> Self {
        self.records.push(ComponentTraceRecord::integer(key, value));
        self
    }

    /// Returns the traced component's symbol.
    pub fn component(&self) -> &Symbol {
        &self.component
    }

    /// Returns the backend that produced this frame.
    pub fn backend(&self) -> ComponentBackend {
        self.backend
    }

    /// Returns the clock position of this frame.
    pub fn clock(&self) -> u64 {
        self.clock
    }

    /// Returns the records captured in this frame, in insertion order.
    pub fn records(&self) -> &[ComponentTraceRecord] {
        &self.records
    }
}
