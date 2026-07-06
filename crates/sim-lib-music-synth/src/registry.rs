use std::collections::{BTreeMap, BTreeSet};

use sim_kernel::{Error, Expr, NumberLiteral, Result, Symbol};

use crate::{ComponentParamDescriptor, ComponentPortDescriptor, DiscreteComponent};

mod entries;
pub use entries::*;

type ComponentFactory = fn() -> Box<dyn DiscreteComponent>;

/// How faithfully a registered component reproduces its hardware reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComponentRegistryCategory {
    /// A sample-exact model of the reference component.
    Exact,
    /// A behaviourally compatible stand-in, not a sample-exact model.
    Compatible,
}

impl ComponentRegistryCategory {
    /// Returns the stable lowercase token for this category.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Compatible => "compatible",
        }
    }

    /// Returns the namespaced symbol naming this category.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/component-category", self.as_str())
    }
}

/// A capability flag advertised by a registered component.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComponentCapability {
    /// Safe to run inside a real-time audio callback.
    RealtimeSafe,
    /// Exposes an editor surface for parameter and patch editing.
    Editable,
    /// Emits trace frames for inspection and diagnostics.
    Traceable,
    /// Has a dedicated specialized editor view.
    SpecializedView,
}

impl ComponentCapability {
    /// Returns the stable lowercase token for this capability.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RealtimeSafe => "realtime-safe",
            Self::Editable => "editable",
            Self::Traceable => "traceable",
            Self::SpecializedView => "specialized-view",
        }
    }

    /// Returns the namespaced symbol naming this capability.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/component-capability", self.as_str())
    }
}

/// The instrument family a registered component belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InstrumentWrapperCategory {
    /// A Yamaha DX7 FM voice or operator.
    Dx7,
    /// A modular analog component (System 700, System 55).
    ModularAnalog,
    /// A fixed-architecture polysynth (PS-3300, SubtractiveSynth).
    FixedPolysynth,
    /// A user-built discrete component graph.
    CustomGraph,
}

impl InstrumentWrapperCategory {
    /// Returns the stable lowercase token for this wrapper category.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dx7 => "dx7",
            Self::ModularAnalog => "modular-analog",
            Self::FixedPolysynth => "fixed-polysynth",
            Self::CustomGraph => "custom-graph",
        }
    }

    /// Returns the namespaced symbol naming this wrapper category.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/instrument-wrapper", self.as_str())
    }
}

/// A single registered component: its identity, classification, advertised
/// surface, and optional factory.
#[derive(Clone, Debug)]
pub struct ComponentRegistryEntry {
    id: Symbol,
    label: String,
    category: ComponentRegistryCategory,
    wrapper: InstrumentWrapperCategory,
    capabilities: BTreeSet<ComponentCapability>,
    ports: Vec<ComponentPortDescriptor>,
    params: Vec<ComponentParamDescriptor>,
    factory: Option<ComponentFactory>,
}

impl ComponentRegistryEntry {
    /// Creates an entry with the given id, label, category, and wrapper and no
    /// capabilities, ports, params, or factory.
    pub fn new(
        id: Symbol,
        label: impl Into<String>,
        category: ComponentRegistryCategory,
        wrapper: InstrumentWrapperCategory,
    ) -> Self {
        Self {
            id,
            label: label.into(),
            category,
            wrapper,
            capabilities: BTreeSet::new(),
            ports: Vec::new(),
            params: Vec::new(),
            factory: None,
        }
    }

    /// Adds a capability flag, returning the updated entry.
    pub fn with_capability(mut self, capability: ComponentCapability) -> Self {
        self.capabilities.insert(capability);
        self
    }

    /// Sets the port descriptors, returning the updated entry.
    pub fn with_ports(mut self, ports: Vec<ComponentPortDescriptor>) -> Self {
        self.ports = ports;
        self
    }

    /// Sets the parameter descriptors, returning the updated entry.
    pub fn with_params(mut self, params: Vec<ComponentParamDescriptor>) -> Self {
        self.params = params;
        self
    }

    /// Attaches the factory used to instantiate the component, returning the
    /// updated entry.
    pub fn with_factory(mut self, factory: ComponentFactory) -> Self {
        self.factory = Some(factory);
        self
    }

    /// Returns the component id.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the human-readable label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the fidelity category.
    pub fn category(&self) -> ComponentRegistryCategory {
        self.category
    }

    /// Returns the instrument wrapper family.
    pub fn wrapper(&self) -> InstrumentWrapperCategory {
        self.wrapper
    }

    /// Returns the set of advertised capabilities.
    pub fn capabilities(&self) -> &BTreeSet<ComponentCapability> {
        &self.capabilities
    }

    /// Returns the port descriptors.
    pub fn ports(&self) -> &[ComponentPortDescriptor] {
        &self.ports
    }

    /// Returns the parameter descriptors.
    pub fn params(&self) -> &[ComponentParamDescriptor] {
        &self.params
    }

    /// Returns `true` when a factory is attached and the component can be
    /// instantiated.
    pub fn is_implemented(&self) -> bool {
        self.factory.is_some()
    }

    /// Returns `true` when this entry advertises the given capability.
    pub fn has_capability(&self, capability: ComponentCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// Instantiates the component from its factory, or errors when the entry is
    /// not implemented.
    pub fn instantiate(&self) -> Result<Box<dyn DiscreteComponent>> {
        self.factory
            .map(|factory| factory())
            .ok_or_else(|| Error::Eval(format!("component is not implemented: {}", self.id)))
    }

    /// Renders this entry as the editor descriptor expression consumed by the
    /// WebUI component editor.
    pub fn to_editor_descriptor_expr(&self) -> Expr {
        let capabilities = self.capabilities.iter().map(|cap| cap.symbol()).collect();
        let trace_available = self.has_capability(ComponentCapability::Traceable);
        let specialized_view = self
            .has_capability(ComponentCapability::SpecializedView)
            .then(|| Symbol::qualified("view/component", self.wrapper.as_str()));
        crate::component::component_editor_descriptor_expr(
            (self.id(), self.label()),
            self.category().symbol(),
            self.wrapper().symbol(),
            capabilities,
            self.ports(),
            self.params(),
            (trace_available, specialized_view),
        )
    }

    fn to_inventory_item(&self) -> ComponentInventoryItem {
        ComponentInventoryItem {
            id: self.id.clone(),
            label: self.label.clone(),
            category: self.category,
            wrapper: self.wrapper,
            capabilities: self.capabilities.iter().copied().collect(),
            port_count: self.ports.len(),
            param_count: self.params.len(),
            implemented: self.is_implemented(),
        }
    }
}

/// An ordered, id-keyed collection of registered components.
#[derive(Clone, Debug, Default)]
pub struct ComponentRegistry {
    entries: BTreeMap<String, ComponentRegistryEntry>,
}

impl ComponentRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an entry, erroring when its id collides with an existing one.
    pub fn register(&mut self, entry: ComponentRegistryEntry) -> Result<()> {
        let key = entry.id.as_qualified_str();
        if self.entries.contains_key(&key) {
            return Err(Error::Eval(format!(
                "duplicate audio synth component registry id: {key}"
            )));
        }
        self.entries.insert(key, entry);
        Ok(())
    }

    /// Looks up an entry by id.
    pub fn get(&self, id: &Symbol) -> Option<&ComponentRegistryEntry> {
        self.entries.get(&id.as_qualified_str())
    }

    /// Iterates over all entries in id order.
    pub fn entries(&self) -> impl Iterator<Item = &ComponentRegistryEntry> {
        self.entries.values()
    }

    /// Returns every entry advertising the given capability.
    pub fn by_capability(&self, capability: ComponentCapability) -> Vec<&ComponentRegistryEntry> {
        self.entries
            .values()
            .filter(|entry| entry.has_capability(capability))
            .collect()
    }

    /// Returns every entry in the given fidelity category.
    pub fn by_category(&self, category: ComponentRegistryCategory) -> Vec<&ComponentRegistryEntry> {
        self.entries
            .values()
            .filter(|entry| entry.category == category)
            .collect()
    }

    /// Builds an [`ComponentInventory`] summary of all registered entries.
    pub fn inventory(&self) -> ComponentInventory {
        ComponentInventory {
            items: self
                .entries
                .values()
                .map(ComponentRegistryEntry::to_inventory_item)
                .collect(),
        }
    }
}

/// A flat, serializable summary of every entry in a [`ComponentRegistry`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentInventory {
    items: Vec<ComponentInventoryItem>,
}

impl ComponentInventory {
    /// Returns the summarized inventory items.
    pub fn items(&self) -> &[ComponentInventoryItem] {
        &self.items
    }

    /// Renders the inventory as a tagged map expression.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                field("tag"),
                Expr::Symbol(Symbol::qualified("audio-synth", "component-inventory")),
            ),
            (
                field("items"),
                Expr::Vector(
                    self.items
                        .iter()
                        .map(ComponentInventoryItem::to_expr)
                        .collect(),
                ),
            ),
        ])
    }
}

/// One row of a [`ComponentInventory`]: a registered component's identity and
/// surface counts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentInventoryItem {
    /// The component id.
    pub id: Symbol,
    /// The human-readable label.
    pub label: String,
    /// The fidelity category.
    pub category: ComponentRegistryCategory,
    /// The instrument wrapper family.
    pub wrapper: InstrumentWrapperCategory,
    /// The advertised capabilities.
    pub capabilities: Vec<ComponentCapability>,
    /// The number of port descriptors.
    pub port_count: usize,
    /// The number of parameter descriptors.
    pub param_count: usize,
    /// Whether the component has a factory and can be instantiated.
    pub implemented: bool,
}

impl ComponentInventoryItem {
    /// Renders this item as a map expression.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            (field("id"), Expr::Symbol(self.id.clone())),
            (field("label"), Expr::String(self.label.clone())),
            (field("category"), Expr::Symbol(self.category.symbol())),
            (field("wrapper"), Expr::Symbol(self.wrapper.symbol())),
            (
                field("capabilities"),
                Expr::Vector(
                    self.capabilities
                        .iter()
                        .map(|capability| Expr::Symbol(capability.symbol()))
                        .collect(),
                ),
            ),
            (field("port-count"), number_usize(self.port_count)),
            (field("param-count"), number_usize(self.param_count)),
            (field("implemented"), Expr::Bool(self.implemented)),
        ])
    }
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym("audio-synth/inventory", name)
}

fn number_usize(value: usize) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}
