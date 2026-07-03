use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_stream_core::{ClockDomain, LatencyClass, RateContract};
use sim_lib_topology::{PlacementNodeProfile, SiteId};

use crate::{LaneId, LaneTarget, PlayEvent};

/// How a chain device combines its output with the events flowing through it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlayerMode {
    /// Pass input through and append generated events.
    Through,
    /// Drop input and emit only generated events.
    Replace,
    /// Filter input by lane, dropping matched events.
    Filter,
    /// Route input while generating side events without merging them.
    Sidechain,
    /// Drop input and emit self-clocked generated events.
    SelfClocked,
}

impl PlayerMode {
    /// Returns the stable wire label for this mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_music_core::PlayerMode;
    ///
    /// assert_eq!(PlayerMode::Through.wire_label(), "through");
    /// assert_eq!(PlayerMode::SelfClocked.wire_label(), "self_clocked");
    /// ```
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::Through => "through",
            Self::Replace => "replace",
            Self::Filter => "filter",
            Self::Sidechain => "sidechain",
            Self::SelfClocked => "self_clocked",
        }
    }

    /// Returns the qualified symbol naming this mode.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("music/player-mode", self.wire_label())
    }
}

/// Stable identifier for a device within a player chain.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerDeviceId(pub String);

impl PlayerDeviceId {
    /// Creates a device id from any string-like value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl AsRef<str> for PlayerDeviceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Typed value for a player parameter snapshot entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParamValue {
    /// Boolean parameter.
    Bool(bool),
    /// Signed 64-bit integer parameter.
    I64(i64),
    /// Text parameter.
    Text(String),
    /// Symbol parameter.
    Symbol(Symbol),
}

impl ParamValue {
    fn to_expr(&self) -> Expr {
        match self {
            Self::Bool(value) => Expr::Bool(*value),
            Self::I64(value) => Expr::String(value.to_string()),
            Self::Text(value) => Expr::String(value.clone()),
            Self::Symbol(value) => Expr::Symbol(value.clone()),
        }
    }
}

/// Sorted, deduplicated snapshot of named player parameters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParamSnapshot {
    /// Parameter entries sorted by key with duplicates removed.
    pub entries: Vec<(String, ParamValue)>,
}

impl ParamSnapshot {
    /// Builds a snapshot, sorting entries by key and dropping duplicate keys.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_music_core::{ParamSnapshot, ParamValue};
    ///
    /// let snapshot = ParamSnapshot::new(vec![
    ///     ("gain".to_owned(), ParamValue::I64(1)),
    ///     ("attack".to_owned(), ParamValue::Bool(true)),
    /// ]);
    /// assert_eq!(snapshot.entries[0].0, "attack");
    /// assert_eq!(snapshot.entries[1].0, "gain");
    /// ```
    pub fn new(mut entries: Vec<(String, ParamValue)>) -> Self {
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        entries.dedup_by(|left, right| left.0 == right.0);
        Self { entries }
    }

    /// Encodes the snapshot as an expression map keyed by parameter name.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(
            self.entries
                .iter()
                .map(|(key, value)| (Expr::Symbol(Symbol::new(key.clone())), value.to_expr()))
                .collect(),
        )
    }
}

/// Placement of a chain device: where it runs and under what rate profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainPlacement {
    /// Site the device is placed on.
    pub site: SiteId,
    /// Node profile describing the device's rate contract and pinning.
    pub profile: PlacementNodeProfile,
}

impl ChainPlacement {
    /// Creates a placement for `site` with the given node profile.
    pub fn new(site: impl Into<SiteId>, profile: PlacementNodeProfile) -> Self {
        Self {
            site: site.into(),
            profile,
        }
    }

    /// Returns the default local-coroutine placement at MIDI-tick rate.
    pub fn local_coroutine() -> Self {
        Self::new(
            "local-coroutine",
            PlacementNodeProfile::new(RateContract::midi_tick(), false),
        )
    }

    /// Encodes the placement, flattening its rate contract, as an expression map.
    pub fn to_expr(&self) -> Expr {
        let rate = self.profile.rate_contract();
        Expr::Map(vec![
            (
                Expr::Symbol(Symbol::new("site")),
                Expr::Symbol(self.site.as_symbol().clone()),
            ),
            (
                Expr::Symbol(Symbol::new("clock-domain")),
                Expr::Symbol(rate.clock_domain().symbol()),
            ),
            (
                Expr::Symbol(Symbol::new("latency-class")),
                Expr::Symbol(rate.latency_class().symbol()),
            ),
            (
                Expr::Symbol(Symbol::new("nominal-rate-hz")),
                Expr::String(
                    rate.nominal_rate_hz()
                        .map(|rate| rate.to_string())
                        .unwrap_or_else(|| "none".to_owned()),
                ),
            ),
            (
                Expr::Symbol(Symbol::new("realtime-pin")),
                Expr::Bool(self.profile.realtime_pin()),
            ),
        ])
    }

    /// Decodes a placement from a map produced by [`ChainPlacement::to_expr`].
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let Expr::Map(entries) = expr else {
            return Err(Error::Eval("chain placement must be a map".to_owned()));
        };
        let site = symbol_field(entries, "site")?.to_string();
        let clock_domain = ClockDomain::from_symbol(symbol_field(entries, "clock-domain")?)?;
        let latency_class = LatencyClass::from_symbol(symbol_field(entries, "latency-class")?)?;
        let nominal_rate_hz =
            match string_field(entries, "nominal-rate-hz")? {
                "none" => None,
                value => Some(value.parse::<u32>().map_err(|err| {
                    Error::Eval(format!("invalid placement nominal-rate-hz: {err}"))
                })?),
            };
        let realtime_pin = bool_field(entries, "realtime-pin")?;
        Ok(Self::new(
            site,
            PlacementNodeProfile::new(
                RateContract::new(clock_domain, latency_class, nominal_rate_hz),
                realtime_pin,
            ),
        ))
    }
}

/// Pairing of a device id with its resolved placement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacedChainDevice {
    /// Identifier of the placed device.
    pub device_id: PlayerDeviceId,
    /// Placement assigned to the device.
    pub placement: ChainPlacement,
}

/// Plan listing the placement of every device in a chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainPlacementPlan {
    /// Placed devices, sorted by device id.
    pub devices: Vec<PlacedChainDevice>,
}

impl ChainPlacementPlan {
    /// Builds a plan, sorting devices by id for stable output.
    pub fn new(mut devices: Vec<PlacedChainDevice>) -> Self {
        devices.sort_by(|left, right| left.device_id.cmp(&right.device_id));
        Self { devices }
    }

    /// Encodes the plan as an expression list of device/placement maps.
    pub fn to_expr(&self) -> Expr {
        Expr::List(
            self.devices
                .iter()
                .map(|device| {
                    Expr::Map(vec![
                        (
                            Expr::Symbol(Symbol::new("device")),
                            Expr::String(device.device_id.0.clone()),
                        ),
                        (
                            Expr::Symbol(Symbol::new("placement")),
                            device.placement.to_expr(),
                        ),
                    ])
                })
                .collect(),
        )
    }

    /// Decodes a plan from a list produced by [`ChainPlacementPlan::to_expr`].
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let Expr::List(items) = expr else {
            return Err(Error::Eval(
                "chain placement plan must be a list".to_owned(),
            ));
        };
        let mut devices = Vec::new();
        for item in items {
            let Expr::Map(entries) = item else {
                return Err(Error::Eval(
                    "chain placement plan item must be a map".to_owned(),
                ));
            };
            devices.push(PlacedChainDevice {
                device_id: PlayerDeviceId::new(string_field(entries, "device")?),
                placement: ChainPlacement::from_expr(field(entries, "placement")?)?,
            });
        }
        Ok(Self::new(devices))
    }
}

/// Descriptor of a chain's output target and its rate contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerTargetDescriptor {
    /// Symbol identifying the target.
    pub id: Symbol,
    /// Lane target the chain output is routed to.
    pub target: LaneTarget,
    /// Rate contract the target runs under.
    pub rate_contract: RateContract,
}

impl PlayerTargetDescriptor {
    /// Builds an instrument target descriptor at MIDI-tick rate.
    pub fn instrument(id: impl Into<String>) -> Self {
        let id = Symbol::qualified("music/target", id.into());
        Self {
            id: id.clone(),
            target: LaneTarget::Instrument(id),
            rate_contract: RateContract::midi_tick(),
        }
    }

    /// Returns the target's clock domain from its rate contract.
    pub fn clock_domain(&self) -> ClockDomain {
        self.rate_contract.clock_domain()
    }

    /// Returns the target's latency class from its rate contract.
    pub fn latency_class(&self) -> LatencyClass {
        self.rate_contract.latency_class()
    }
}

/// A single processing stage in a player chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainDevice {
    /// Identifier of this device.
    pub id: PlayerDeviceId,
    /// Symbol of the player backing this device.
    pub player: Symbol,
    /// Mode controlling how the device combines with its input.
    pub mode: PlayerMode,
    /// Sort order of the device within the chain.
    pub order: u32,
    /// Whether the device passes input through untouched.
    pub bypass: bool,
    /// Whether the device is muted (excluded from rendering).
    pub mute: bool,
    /// Whether the device is soloed.
    pub solo: bool,
    /// Whether the device is enabled.
    pub enabled: bool,
    /// Parameter snapshot for the device.
    pub params: ParamSnapshot,
    /// Events the device contributes to the chain.
    pub generated: Vec<PlayEvent>,
    /// Lane ids the device filters out, kept sorted for binary search.
    pub filter_lanes: Vec<LaneId>,
    /// Optional lane the device reroutes events onto.
    pub route_lane: Option<LaneId>,
    /// Placement of the device on a site.
    pub placement: ChainPlacement,
}

impl ChainDevice {
    /// Creates an enabled device with default flags and local placement.
    pub fn new(id: impl Into<String>, player: Symbol, mode: PlayerMode, order: u32) -> Self {
        Self {
            id: PlayerDeviceId::new(id),
            player,
            mode,
            order,
            bypass: false,
            mute: false,
            solo: false,
            enabled: true,
            params: ParamSnapshot::default(),
            generated: Vec::new(),
            filter_lanes: Vec::new(),
            route_lane: None,
            placement: ChainPlacement::local_coroutine(),
        }
    }

    /// Sets the device's generated events.
    pub fn with_generated(mut self, generated: Vec<PlayEvent>) -> Self {
        self.generated = generated;
        self
    }

    /// Sets the filter lanes, keeping them sorted for lookup.
    pub fn with_filter_lanes(mut self, lanes: Vec<LaneId>) -> Self {
        self.filter_lanes = lanes;
        self.filter_lanes.sort();
        self
    }

    /// Sets the lane that events are rerouted onto.
    pub fn with_route_lane(mut self, lane: LaneId) -> Self {
        self.route_lane = Some(lane);
        self
    }

    /// Sets the device's placement.
    pub fn with_placement(mut self, placement: ChainPlacement) -> Self {
        self.placement = placement;
        self
    }

    /// Marks the device as bypassed.
    pub fn bypassed(mut self) -> Self {
        self.bypass = true;
        self
    }
}

pub(crate) fn field<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Result<&'a Expr> {
    entries
        .iter()
        .find_map(|(key, value)| match key {
            Expr::Symbol(symbol) if symbol.namespace.is_none() && symbol.name.as_ref() == name => {
                Some(value)
            }
            _ => None,
        })
        .ok_or_else(|| Error::Eval(format!("missing {name} field")))
}

pub(crate) fn string_field<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Result<&'a str> {
    match field(entries, name)? {
        Expr::String(value) => Ok(value),
        _ => Err(Error::Eval(format!("{name} field must be text"))),
    }
}

pub(crate) fn symbol_field<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Result<&'a Symbol> {
    match field(entries, name)? {
        Expr::Symbol(value) => Ok(value),
        _ => Err(Error::Eval(format!("{name} field must be a symbol"))),
    }
}

pub(crate) fn bool_field(entries: &[(Expr, Expr)], name: &str) -> Result<bool> {
    match field(entries, name)? {
        Expr::Bool(value) => Ok(*value),
        _ => Err(Error::Eval(format!("{name} field must be a boolean"))),
    }
}
