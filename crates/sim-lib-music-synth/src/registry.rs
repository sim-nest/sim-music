use std::collections::{BTreeMap, BTreeSet};

use sim_kernel::{Error, Expr, NumberLiteral, Result, Symbol};

use crate::{
    ComponentParamDescriptor, ComponentPortDescriptor, DiscreteComponent, DiscreteComponentGraph,
    Dx7FmOperator, Dx7ModeledOperator, Dx7Voice, SubtractiveSynth, SynthPreset,
    discrete_component_graph_id, discrete_component_graph_ports, dx7_modeled_operator_component_id,
    dx7_operator_component_id, dx7_operator_params, dx7_operator_ports, dx7_voice_params,
    dx7_voice_ports,
    registry_ps3300::{ps_3300_registry_entry, ps3300_registry_entries},
    registry_system55::{system55_instrument_registry_entry, system55_registry_entries},
    subtractive_synth_component_id, subtractive_synth_params, subtractive_synth_ports,
    system700::{
        System700, System700Clock, System700Envelope, System700ExternalInput, System700Keyboard,
        System700Lfo, System700Mixer, System700Multiple, System700Noise, System700RingModulator,
        System700SampleHold, System700Sequencer, System700Vca, System700Vcf, System700Vco,
        System700VoltageProcessor, r700_clock_component_id, r700_clock_params, r700_clock_ports,
        r700_envelope_component_id, r700_envelope_params, r700_envelope_ports,
        r700_external_input_component_id, r700_external_input_params, r700_external_input_ports,
        r700_keyboard_component_id, r700_keyboard_params, r700_keyboard_ports,
        r700_lfo_component_id, r700_lfo_params, r700_lfo_ports, r700_mixer_component_id,
        r700_mixer_params, r700_mixer_ports, r700_multiple_component_id, r700_multiple_params,
        r700_multiple_ports, r700_noise_component_id, r700_noise_params, r700_noise_ports,
        r700_ring_component_id, r700_ring_params, r700_ring_ports, r700_sample_hold_component_id,
        r700_sample_hold_params, r700_sample_hold_ports, r700_sequencer_component_id,
        r700_sequencer_params, r700_sequencer_ports, r700_vca_component_id, r700_vca_params,
        r700_vca_ports, r700_vcf_component_id, r700_vcf_params, r700_vcf_ports,
        r700_vco_component_id, r700_vco_params, r700_vco_ports,
        r700_voltage_processor_component_id, r700_voltage_processor_params,
        r700_voltage_processor_ports, system700_params, system700_ports,
    },
};

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

/// Builds the default registry of every component this crate ships: the
/// SubtractiveSynth, the discrete graph, DX7 operators, System 700, System 55,
/// and PS-3300 modules, plus the four whole-instrument wrappers.
pub fn default_audio_synth_registry() -> ComponentRegistry {
    let mut registry = ComponentRegistry::new();
    registry
        .register(subtractive_synth_registry_entry())
        .expect("default registry ids are unique");
    registry
        .register(component_graph_registry_entry())
        .expect("default registry ids are unique");
    registry
        .register(dx7_operator_registry_entry())
        .expect("default registry ids are unique");
    registry
        .register(dx7_modeled_operator_registry_entry())
        .expect("default registry ids are unique");
    for entry in [
        r700_vco_registry_entry(),
        r700_lfo_registry_entry(),
        r700_noise_registry_entry(),
        r700_vcf_registry_entry(),
        r700_vca_registry_entry(),
        r700_ring_registry_entry(),
        r700_envelope_registry_entry(),
        r700_sample_hold_registry_entry(),
        r700_voltage_processor_registry_entry(),
        r700_mixer_registry_entry(),
        r700_multiple_registry_entry(),
        r700_external_input_registry_entry(),
        r700_keyboard_registry_entry(),
        r700_clock_registry_entry(),
        r700_sequencer_registry_entry(),
    ] {
        registry
            .register(entry)
            .expect("default registry ids are unique");
    }
    for entry in system55_registry_entries() {
        registry
            .register(entry)
            .expect("default registry ids are unique");
    }
    for entry in ps3300_registry_entries() {
        registry
            .register(entry)
            .expect("default registry ids are unique");
    }
    for entry in [
        dx7_registry_entry(),
        system_700_registry_entry(),
        system55_instrument_registry_entry(),
        ps_3300_registry_entry(),
    ] {
        registry
            .register(entry)
            .expect("default registry ids are unique");
    }
    registry
}

/// Returns the registry entry for the built-in subtractive polysynth.
pub fn subtractive_synth_registry_entry() -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        subtractive_synth_component_id(),
        "SubtractiveSynth",
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::FixedPolysynth,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_ports(subtractive_synth_ports())
    .with_params(subtractive_synth_params())
    .with_factory(|| Box::new(SubtractiveSynth::new(SynthPreset::default())))
}

/// Returns the registry entry for the user-built discrete component graph.
pub fn component_graph_registry_entry() -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        discrete_component_graph_id(),
        "DiscreteComponentGraph",
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::CustomGraph,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_capability(ComponentCapability::SpecializedView)
    .with_ports(discrete_component_graph_ports())
    .with_factory(|| {
        Box::new(DiscreteComponentGraph::new(Symbol::qualified(
            "audio-synth",
            "custom-graph",
        )))
    })
}

/// Returns the registry entry for a single algorithmic DX7 FM operator.
pub fn dx7_operator_registry_entry() -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        dx7_operator_component_id(),
        "DX7 Operator",
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::Dx7,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_ports(dx7_operator_ports())
    .with_params(dx7_operator_params())
    .with_factory(|| Box::new(Dx7FmOperator::default()))
}

/// Returns the registry entry for the analog-modeled DX7 FM operator.
pub fn dx7_modeled_operator_registry_entry() -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        dx7_modeled_operator_component_id(),
        "DX7 Modeled Operator",
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::Dx7,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_ports(dx7_operator_ports())
    .with_params(dx7_operator_params())
    .with_factory(|| Box::new(Dx7ModeledOperator::default()))
}

/// Returns the component id of the whole DX7 instrument.
pub fn dx7_component_id() -> Symbol {
    Symbol::qualified("audio-synth/instrument", "dx7")
}

/// Returns the component id of the Roland System 700 instrument.
pub fn system_700_component_id() -> Symbol {
    Symbol::qualified("audio-synth/instrument", "roland-system-700")
}

/// Returns the component id of the Moog System 55 instrument.
pub fn system_55_component_id() -> Symbol {
    Symbol::qualified("audio-synth/instrument", "moog-system-55")
}

/// Returns the component id of the Korg PS-3300 instrument.
pub fn ps_3300_component_id() -> Symbol {
    Symbol::qualified("audio-synth/instrument", "korg-ps-3300")
}

/// Returns the registry entry for the whole DX7 voice instrument.
pub fn dx7_registry_entry() -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        dx7_component_id(),
        "DX7",
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::Dx7,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_capability(ComponentCapability::SpecializedView)
    .with_ports(dx7_voice_ports())
    .with_params(dx7_voice_params())
    .with_factory(|| Box::new(Dx7Voice::default()))
}

/// Returns the registry entry for the System 700 VCO module.
pub fn r700_vco_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_vco_component_id(),
        "R700 VCO",
        r700_vco_ports(),
        r700_vco_params(),
        || Box::new(System700Vco::default()),
    )
}

/// Returns the registry entry for the System 700 LFO module.
pub fn r700_lfo_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_lfo_component_id(),
        "R700 LFO",
        r700_lfo_ports(),
        r700_lfo_params(),
        || Box::new(System700Lfo::default()),
    )
}

/// Returns the registry entry for the System 700 noise source module.
pub fn r700_noise_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_noise_component_id(),
        "R700 Noise",
        r700_noise_ports(),
        r700_noise_params(),
        || Box::new(System700Noise::default()),
    )
}

/// Returns the registry entry for the System 700 VCF module.
pub fn r700_vcf_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_vcf_component_id(),
        "R700 VCF",
        r700_vcf_ports(),
        r700_vcf_params(),
        || Box::new(System700Vcf::default()),
    )
}

/// Returns the registry entry for the System 700 VCA module.
pub fn r700_vca_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_vca_component_id(),
        "R700 VCA",
        r700_vca_ports(),
        r700_vca_params(),
        || Box::new(System700Vca::default()),
    )
}

/// Returns the registry entry for the System 700 ring modulator module.
pub fn r700_ring_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_ring_component_id(),
        "R700 Ring",
        r700_ring_ports(),
        r700_ring_params(),
        || Box::new(System700RingModulator::default()),
    )
}

/// Returns the registry entry for the System 700 envelope generator module.
pub fn r700_envelope_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_envelope_component_id(),
        "R700 Envelope",
        r700_envelope_ports(),
        r700_envelope_params(),
        || Box::new(System700Envelope::default()),
    )
}

/// Returns the registry entry for the System 700 sample-and-hold module.
pub fn r700_sample_hold_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_sample_hold_component_id(),
        "R700 Sample and Hold",
        r700_sample_hold_ports(),
        r700_sample_hold_params(),
        || Box::new(System700SampleHold::default()),
    )
}

/// Returns the registry entry for the System 700 voltage processor module.
pub fn r700_voltage_processor_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_voltage_processor_component_id(),
        "R700 Voltage Processor",
        r700_voltage_processor_ports(),
        r700_voltage_processor_params(),
        || Box::new(System700VoltageProcessor::default()),
    )
}

/// Returns the registry entry for the System 700 mixer module.
pub fn r700_mixer_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_mixer_component_id(),
        "R700 Mixer",
        r700_mixer_ports(),
        r700_mixer_params(),
        || Box::new(System700Mixer::default()),
    )
}

/// Returns the registry entry for the System 700 multiple (signal splitter) module.
pub fn r700_multiple_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_multiple_component_id(),
        "R700 Multiple",
        r700_multiple_ports(),
        r700_multiple_params(),
        || Box::new(System700Multiple::default()),
    )
}

/// Returns the registry entry for the System 700 external input module.
pub fn r700_external_input_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_external_input_component_id(),
        "R700 External Input",
        r700_external_input_ports(),
        r700_external_input_params(),
        || Box::new(System700ExternalInput::default()),
    )
}

/// Returns the registry entry for the System 700 keyboard controller module.
pub fn r700_keyboard_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_keyboard_component_id(),
        "R700 Keyboard",
        r700_keyboard_ports(),
        r700_keyboard_params(),
        || Box::new(System700Keyboard::default()),
    )
}

/// Returns the registry entry for the System 700 clock module.
pub fn r700_clock_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_clock_component_id(),
        "R700 Clock",
        r700_clock_ports(),
        r700_clock_params(),
        || Box::new(System700Clock::default()),
    )
}

/// Returns the registry entry for the System 700 sequencer module.
pub fn r700_sequencer_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_sequencer_component_id(),
        "R700 Sequencer",
        r700_sequencer_ports(),
        r700_sequencer_params(),
        || Box::new(System700Sequencer::default()),
    )
}

fn exact_modular_entry(
    id: Symbol,
    label: &'static str,
    ports: Vec<ComponentPortDescriptor>,
    params: Vec<ComponentParamDescriptor>,
    factory: ComponentFactory,
) -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        id,
        label,
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::ModularAnalog,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_ports(ports)
    .with_params(params)
    .with_factory(factory)
}

fn system_700_registry_entry() -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        system_700_component_id(),
        "Roland System 700",
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::ModularAnalog,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_capability(ComponentCapability::SpecializedView)
    .with_ports(system700_ports())
    .with_params(system700_params())
    .with_factory(|| Box::new(System700::default()))
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
