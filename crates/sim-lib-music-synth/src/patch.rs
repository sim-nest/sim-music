use sim_kernel::{Error, Expr, NumberLiteral, Result, Symbol};
use sim_lib_topology::{
    Graph, InstrumentTopologyAdapter, InstrumentTopologyCord, InstrumentTopologyJack,
    InstrumentTopologyModule, InstrumentTopologySpec, PortMode, PortRef,
};

use crate::{ComponentPortMedia, SynthPreset};

const LIB_NS: &str = "audio-synth";

/// A complete instrument patch: named modules, the cords between their jacks,
/// instrument-level settings, and an optional raw view.
#[derive(Clone, Debug, PartialEq)]
pub struct InstrumentPatch {
    /// The patch name.
    pub name: Symbol,
    /// The modules in the patch.
    pub modules: Vec<PatchModule>,
    /// The patch cords connecting module jacks.
    pub cords: Vec<PatchCord>,
    /// Instrument-level settings.
    pub settings: Vec<PatchSetting>,
    /// An optional raw, format-specific view of the patch.
    pub raw_view: Option<PatchRawView>,
}

impl InstrumentPatch {
    /// Creates an empty patch with the given name.
    pub fn new(name: Symbol) -> Self {
        Self {
            name,
            modules: Vec::new(),
            cords: Vec::new(),
            settings: Vec::new(),
            raw_view: None,
        }
    }

    /// Adds a module, returning the updated patch.
    pub fn with_module(mut self, module: PatchModule) -> Self {
        self.modules.push(module);
        self
    }

    /// Adds a patch cord, returning the updated patch.
    pub fn with_cord(mut self, cord: PatchCord) -> Self {
        self.cords.push(cord);
        self
    }

    /// Adds an instrument-level setting, returning the updated patch.
    pub fn with_setting(mut self, key: Symbol, value: Expr) -> Self {
        self.settings.push(PatchSetting { key, value });
        self
    }

    /// Attaches a raw view, returning the updated patch.
    pub fn with_raw_view(mut self, raw_view: PatchRawView) -> Self {
        self.raw_view = Some(raw_view);
        self
    }

    /// Builds the topology graph for this patch via the topology adapter.
    pub fn topology_graph(&self) -> Graph {
        InstrumentTopologyAdapter.graph_from_spec(&self.topology_spec())
    }

    /// Builds the topology spec describing this patch's modules, cords, and
    /// metadata.
    pub fn topology_spec(&self) -> InstrumentTopologySpec {
        let mut spec = InstrumentTopologySpec::new(self.name.clone());
        if !self.settings.is_empty() {
            spec = spec.with_metadata(
                Symbol::qualified(LIB_NS, "settings"),
                Expr::Map(setting_expr_entries(&self.settings)),
            );
        }
        if let Some(raw_view) = &self.raw_view {
            spec = spec.with_metadata(Symbol::qualified(LIB_NS, "raw-view"), raw_view.to_expr());
        }
        for module in &self.modules {
            spec = spec.with_module(module.to_topology_module());
        }
        for cord in &self.cords {
            spec = spec.with_cord(cord.to_topology_cord());
        }
        spec
    }

    /// Renders the patch as a tagged map expression.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            entry("tag", Expr::Symbol(Symbol::qualified(LIB_NS, "patch"))),
            entry("name", Expr::Symbol(self.name.clone())),
            entry(
                "modules",
                Expr::List(self.modules.iter().map(PatchModule::to_expr).collect()),
            ),
            entry(
                "cords",
                Expr::List(self.cords.iter().map(PatchCord::to_expr).collect()),
            ),
            entry("settings", Expr::Map(setting_expr_entries(&self.settings))),
            entry(
                "raw-view",
                self.raw_view
                    .as_ref()
                    .map(PatchRawView::to_expr)
                    .unwrap_or(Expr::Nil),
            ),
        ])
    }

    /// Parses a patch from a tagged map expression, erroring on a missing tag
    /// or malformed fields.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let map = expr_map(expr, "instrument patch")?;
        match lookup(map, "tag") {
            Some(Expr::Symbol(symbol)) if is_symbol(symbol, LIB_NS, "patch") => {}
            Some(_) => return Err(patch_error("instrument patch tag is invalid")),
            None => return Err(missing("tag")),
        }
        let modules = expr_list(lookup_required(map, "modules")?, "patch modules")?
            .iter()
            .map(PatchModule::from_expr)
            .collect::<Result<Vec<_>>>()?;
        let cords = expr_list(lookup_required(map, "cords")?, "patch cords")?
            .iter()
            .map(PatchCord::from_expr)
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            name: expr_symbol(lookup_required(map, "name")?, "patch name")?.clone(),
            modules,
            cords,
            settings: settings_from_expr(lookup_required(map, "settings")?)?,
            raw_view: raw_view_from_expr(lookup_required(map, "raw-view")?)?,
        })
    }
}

/// A single module within an [`InstrumentPatch`]: its identity, kind, input and
/// output jacks, settings, and optional raw view.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchModule {
    /// The module instance id, unique within the patch.
    pub id: Symbol,
    /// The module kind (component type).
    pub kind: Symbol,
    /// The input jacks.
    pub inputs: Vec<PatchJack>,
    /// The output jacks.
    pub outputs: Vec<PatchJack>,
    /// Module-level settings.
    pub settings: Vec<PatchSetting>,
    /// An optional raw, format-specific view of the module.
    pub raw_view: Option<PatchRawView>,
}

impl PatchModule {
    /// Creates a module with the given id and kind and no jacks or settings.
    pub fn new(id: Symbol, kind: Symbol) -> Self {
        Self {
            id,
            kind,
            inputs: Vec::new(),
            outputs: Vec::new(),
            settings: Vec::new(),
            raw_view: None,
        }
    }

    /// Adds an input jack, returning the updated module.
    pub fn with_input(mut self, jack: PatchJack) -> Self {
        self.inputs.push(jack);
        self
    }

    /// Adds an output jack, returning the updated module.
    pub fn with_output(mut self, jack: PatchJack) -> Self {
        self.outputs.push(jack);
        self
    }

    /// Adds a module-level setting, returning the updated module.
    pub fn with_setting(mut self, key: Symbol, value: Expr) -> Self {
        self.settings.push(PatchSetting { key, value });
        self
    }

    /// Attaches a raw view, returning the updated module.
    pub fn with_raw_view(mut self, raw_view: PatchRawView) -> Self {
        self.raw_view = Some(raw_view);
        self
    }

    fn to_topology_module(&self) -> InstrumentTopologyModule {
        let mut module = InstrumentTopologyModule::new(self.id.clone(), self.kind.clone());
        for jack in &self.inputs {
            module = module.with_input(jack.to_topology_jack());
        }
        for jack in &self.outputs {
            module = module.with_output(jack.to_topology_jack());
        }
        for setting in &self.settings {
            module = module.with_setting(setting.key.clone(), setting.value.clone());
        }
        if let Some(raw_view) = &self.raw_view {
            module = module.with_raw(Symbol::new("format"), Expr::Symbol(raw_view.format.clone()));
            for (key, value) in &raw_view.fields {
                module = module.with_raw(key.clone(), value.clone());
            }
        }
        module
    }

    fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            entry("id", Expr::Symbol(self.id.clone())),
            entry("kind", Expr::Symbol(self.kind.clone())),
            entry(
                "inputs",
                Expr::List(self.inputs.iter().map(PatchJack::to_expr).collect()),
            ),
            entry(
                "outputs",
                Expr::List(self.outputs.iter().map(PatchJack::to_expr).collect()),
            ),
            entry("settings", Expr::Map(setting_expr_entries(&self.settings))),
            entry(
                "raw-view",
                self.raw_view
                    .as_ref()
                    .map(PatchRawView::to_expr)
                    .unwrap_or(Expr::Nil),
            ),
        ])
    }

    fn from_expr(expr: &Expr) -> Result<Self> {
        let map = expr_map(expr, "patch module")?;
        let inputs = expr_list(lookup_required(map, "inputs")?, "module inputs")?
            .iter()
            .map(PatchJack::from_expr)
            .collect::<Result<Vec<_>>>()?;
        let outputs = expr_list(lookup_required(map, "outputs")?, "module outputs")?
            .iter()
            .map(PatchJack::from_expr)
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            id: expr_symbol(lookup_required(map, "id")?, "module id")?.clone(),
            kind: expr_symbol(lookup_required(map, "kind")?, "module kind")?.clone(),
            inputs,
            outputs,
            settings: settings_from_expr(lookup_required(map, "settings")?)?,
            raw_view: raw_view_from_expr(lookup_required(map, "raw-view")?)?,
        })
    }
}

/// A single jack on a [`PatchModule`]: its name, signal media, whether a
/// connection is required, and an optional normalled default.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchJack {
    /// The jack name.
    pub name: Symbol,
    /// The signal media carried by the jack.
    pub media: ComponentPortMedia,
    /// Whether the jack must be connected.
    pub required: bool,
    /// The value normalled into the jack when nothing is patched in.
    pub normalled_default: Option<Expr>,
}

impl PatchJack {
    /// Creates a jack with the given name, media, and required flag.
    pub fn new(name: impl Into<String>, media: ComponentPortMedia, required: bool) -> Self {
        Self {
            name: Symbol::new(name.into()),
            media,
            required,
            normalled_default: None,
        }
    }

    /// Creates an audio-rate jack.
    pub fn audio(name: impl Into<String>, required: bool) -> Self {
        Self::new(name, ComponentPortMedia::AudioRate, required)
    }

    /// Creates a control-rate jack.
    pub fn control(name: impl Into<String>, required: bool) -> Self {
        Self::new(name, ComponentPortMedia::ControlRate, required)
    }

    /// Creates a control-voltage jack.
    pub fn cv(name: impl Into<String>, required: bool) -> Self {
        Self::new(name, ComponentPortMedia::ControlVoltage, required)
    }

    /// Creates an event jack.
    pub fn event(name: impl Into<String>, required: bool) -> Self {
        Self::new(name, ComponentPortMedia::Event, required)
    }

    /// Creates a gate jack.
    pub fn gate(name: impl Into<String>, required: bool) -> Self {
        Self::new(name, ComponentPortMedia::Gate, required)
    }

    /// Sets the normalled default value, returning the updated jack.
    pub fn with_normalled_default(mut self, value: Expr) -> Self {
        self.normalled_default = Some(value);
        self
    }

    fn to_topology_jack(&self) -> InstrumentTopologyJack {
        let mut jack = InstrumentTopologyJack::new(
            self.name.clone(),
            topology_port_mode(self.media),
            self.required,
        );
        if let Some(default) = &self.normalled_default {
            jack = jack.with_normalled_default(default.clone());
        }
        jack
    }

    fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            entry("name", Expr::Symbol(self.name.clone())),
            entry("media", Expr::Symbol(self.media.symbol())),
            entry("required", Expr::Bool(self.required)),
            entry(
                "normalled-default",
                self.normalled_default.clone().unwrap_or(Expr::Nil),
            ),
        ])
    }

    fn from_expr(expr: &Expr) -> Result<Self> {
        let map = expr_map(expr, "patch jack")?;
        let media =
            expr_symbol(lookup_required(map, "media")?, "jack media").and_then(|symbol| {
                ComponentPortMedia::from_name(symbol.name.as_ref())
                    .ok_or_else(|| patch_error(format!("unknown jack media {}", symbol.name)))
            })?;
        Ok(Self {
            name: expr_symbol(lookup_required(map, "name")?, "jack name")?.clone(),
            media,
            required: expr_bool(lookup_required(map, "required")?, "jack required")?,
            normalled_default: match lookup_required(map, "normalled-default")? {
                Expr::Nil => None,
                value => Some(value.clone()),
            },
        })
    }
}

/// A patch cord connecting an output endpoint to an input endpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchCord {
    /// The source endpoint (output jack).
    pub from: PatchEndpoint,
    /// The destination endpoint (input jack).
    pub to: PatchEndpoint,
}

impl PatchCord {
    /// Creates a cord from one endpoint to another.
    pub fn new(from: PatchEndpoint, to: PatchEndpoint) -> Self {
        Self { from, to }
    }

    fn to_topology_cord(&self) -> InstrumentTopologyCord {
        InstrumentTopologyCord::new(self.from.to_port_ref(), self.to.to_port_ref())
    }

    fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            entry("from", self.from.to_expr()),
            entry("to", self.to.to_expr()),
        ])
    }

    fn from_expr(expr: &Expr) -> Result<Self> {
        let map = expr_map(expr, "patch cord")?;
        Ok(Self {
            from: PatchEndpoint::from_expr(lookup_required(map, "from")?)?,
            to: PatchEndpoint::from_expr(lookup_required(map, "to")?)?,
        })
    }
}

/// One end of a [`PatchCord`]: a module id paired with a jack name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchEndpoint {
    /// The module id.
    pub module: Symbol,
    /// The jack name on that module.
    pub jack: Symbol,
}

impl PatchEndpoint {
    /// Creates an endpoint referencing the given module and jack.
    pub fn new(module: impl Into<String>, jack: impl Into<String>) -> Self {
        Self {
            module: Symbol::new(module.into()),
            jack: Symbol::new(jack.into()),
        }
    }

    fn to_port_ref(&self) -> PortRef {
        PortRef::new(self.module.clone(), self.jack.clone())
    }

    fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            entry("module", Expr::Symbol(self.module.clone())),
            entry("jack", Expr::Symbol(self.jack.clone())),
        ])
    }

    fn from_expr(expr: &Expr) -> Result<Self> {
        let map = expr_map(expr, "patch endpoint")?;
        Ok(Self {
            module: expr_symbol(lookup_required(map, "module")?, "endpoint module")?.clone(),
            jack: expr_symbol(lookup_required(map, "jack")?, "endpoint jack")?.clone(),
        })
    }
}

/// A key-value setting on a patch or module.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchSetting {
    /// The setting key.
    pub key: Symbol,
    /// The setting value.
    pub value: Expr,
}

/// A raw, format-specific view of a patch or module preserved alongside the
/// structured representation.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchRawView {
    /// The raw format identifier.
    pub format: Symbol,
    /// The raw key-value fields in the given format.
    pub fields: Vec<(Symbol, Expr)>,
}

impl PatchRawView {
    /// Creates an empty raw view in the given format.
    pub fn new(format: Symbol) -> Self {
        Self {
            format,
            fields: Vec::new(),
        }
    }

    /// Adds a raw field, returning the updated view.
    pub fn with_field(mut self, key: Symbol, value: Expr) -> Self {
        self.fields.push((key, value));
        self
    }

    fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            entry("format", Expr::Symbol(self.format.clone())),
            entry("fields", Expr::Map(symbol_expr_entries(&self.fields))),
        ])
    }
}

/// Builds the canonical patch for the subtractive synth algorithm: an input,
/// an oscillator, an amplifier, and an output wired in series from the given
/// preset.
pub fn subtractive_synth_algorithm_patch(preset: &SynthPreset) -> InstrumentPatch {
    InstrumentPatch::new(Symbol::qualified(LIB_NS, "subtractive-synth"))
        .with_module(
            PatchModule::new(Symbol::new("in"), Symbol::new("in"))
                .with_output(PatchJack::control("out", true)),
        )
        .with_module(
            PatchModule::new(Symbol::new("osc"), Symbol::qualified(LIB_NS, "oscillator"))
                .with_input(PatchJack::control("pitch", true))
                .with_input(PatchJack::gate("gate", true).with_normalled_default(Expr::Bool(true)))
                .with_output(PatchJack::audio("audio", true))
                .with_setting(
                    Symbol::new("oscillator"),
                    Expr::Symbol(Symbol::qualified(LIB_NS, preset.oscillator.as_str())),
                ),
        )
        .with_module(
            PatchModule::new(Symbol::new("amp"), Symbol::qualified(LIB_NS, "amplifier"))
                .with_input(PatchJack::audio("audio", true))
                .with_input(
                    PatchJack::control("gain", true)
                        .with_normalled_default(number_f32(preset.amp_gain)),
                )
                .with_output(PatchJack::audio("audio", true)),
        )
        .with_module(
            PatchModule::new(Symbol::new("out"), Symbol::new("out"))
                .with_input(PatchJack::audio("in", true)),
        )
        .with_cord(PatchCord::new(
            PatchEndpoint::new("in", "out"),
            PatchEndpoint::new("osc", "pitch"),
        ))
        .with_cord(PatchCord::new(
            PatchEndpoint::new("osc", "audio"),
            PatchEndpoint::new("amp", "audio"),
        ))
        .with_cord(PatchCord::new(
            PatchEndpoint::new("amp", "audio"),
            PatchEndpoint::new("out", "in"),
        ))
}

fn topology_port_mode(media: ComponentPortMedia) -> PortMode {
    match media {
        ComponentPortMedia::AudioRate | ComponentPortMedia::Trace => PortMode::Stream,
        ComponentPortMedia::ControlVoltage
        | ComponentPortMedia::ControlRate
        | ComponentPortMedia::Event
        | ComponentPortMedia::Metadata
        | ComponentPortMedia::Gate => PortMode::Value,
    }
}

fn entry(name: &'static str, value: Expr) -> (Expr, Expr) {
    (Expr::Symbol(Symbol::qualified(LIB_NS, name)), value)
}

fn setting_expr_entries(settings: &[PatchSetting]) -> Vec<(Expr, Expr)> {
    settings
        .iter()
        .map(|setting| (Expr::Symbol(setting.key.clone()), setting.value.clone()))
        .collect()
}

fn symbol_expr_entries(entries: &[(Symbol, Expr)]) -> Vec<(Expr, Expr)> {
    entries
        .iter()
        .map(|(key, value)| (Expr::Symbol(key.clone()), value.clone()))
        .collect()
}

fn settings_from_expr(expr: &Expr) -> Result<Vec<PatchSetting>> {
    expr_map(expr, "patch settings")?
        .iter()
        .map(|(key, value)| {
            Ok(PatchSetting {
                key: expr_symbol(key, "setting key")?.clone(),
                value: value.clone(),
            })
        })
        .collect()
}

fn raw_view_from_expr(expr: &Expr) -> Result<Option<PatchRawView>> {
    if matches!(expr, Expr::Nil) {
        return Ok(None);
    }
    let map = expr_map(expr, "raw view")?;
    let fields = expr_map(lookup_required(map, "fields")?, "raw view fields")?
        .iter()
        .map(|(key, value)| Ok((expr_symbol(key, "raw field key")?.clone(), value.clone())))
        .collect::<Result<Vec<_>>>()?;
    Ok(Some(PatchRawView {
        format: expr_symbol(lookup_required(map, "format")?, "raw view format")?.clone(),
        fields,
    }))
}

fn lookup_required<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Result<&'a Expr> {
    lookup(entries, name).ok_or_else(|| missing(name))
}

fn lookup<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    entries.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol) if symbol.name.as_ref() == name => Some(value),
        _ => None,
    })
}

fn expr_map<'a>(expr: &'a Expr, context: &str) -> Result<&'a [(Expr, Expr)]> {
    match expr {
        Expr::Map(entries) => Ok(entries),
        _ => Err(patch_error(format!("expected {context} map"))),
    }
}

fn expr_list<'a>(expr: &'a Expr, context: &str) -> Result<&'a [Expr]> {
    match expr {
        Expr::List(items) | Expr::Vector(items) => Ok(items),
        _ => Err(patch_error(format!("expected {context} list"))),
    }
}

fn expr_symbol<'a>(expr: &'a Expr, context: &str) -> Result<&'a Symbol> {
    match expr {
        Expr::Symbol(symbol) => Ok(symbol),
        _ => Err(patch_error(format!("expected {context} symbol"))),
    }
}

fn expr_bool(expr: &Expr, context: &str) -> Result<bool> {
    match expr {
        Expr::Bool(value) => Ok(*value),
        _ => Err(patch_error(format!("expected {context} bool"))),
    }
}

fn is_symbol(symbol: &Symbol, namespace: &str, name: &str) -> bool {
    symbol.namespace.as_deref() == Some(namespace) && symbol.name.as_ref() == name
}

fn missing(name: &str) -> Error {
    patch_error(format!("missing {name}"))
}

fn patch_error(message: impl Into<String>) -> Error {
    Error::Eval(format!("audio synth patch error: {}", message.into()))
}

fn number_f32(value: f32) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "f64"),
        canonical: value.to_string(),
    })
}
