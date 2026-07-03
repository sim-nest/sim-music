use sim_kernel::{Error, Expr, Result, Symbol};

use crate::arranger::{
    Arranger, ArrangerPlacement, FilterRef, PitchRemap, PlacementTransform, PlayableRef,
    StretchPolicy, TracePolicy,
};
use crate::arranger_music_expr::{music_from_expr, music_to_expr};
use crate::{LaneId, LaneTarget, Pitch, PitchClass, Time};

const NS: &str = "music/arranger";

impl Arranger {
    /// Encodes the arrangement as a tagged `music/arranger` expression map.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            ("tag", tag_expr("arranger")),
            (
                "lanes",
                Expr::Vector(
                    self.lanes
                        .iter()
                        .map(|lane| Expr::String(lane.0.clone()))
                        .collect(),
                ),
            ),
            (
                "placements",
                Expr::Vector(
                    self.placements
                        .iter()
                        .map(ArrangerPlacement::to_expr)
                        .collect(),
                ),
            ),
        ])
    }

    /// Decodes an arrangement from a tagged `music/arranger` expression map.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let entries = expr_map(expr, "arranger")?;
        expect_tag(entries, "arranger", "arranger")?;
        let lanes = optional_vector(entries, "lanes")?
            .iter()
            .map(|expr| Ok(LaneId::new(expr_string(expr, "lane")?.to_owned())))
            .collect::<Result<Vec<_>>>()?;
        let placements = expr_vector(lookup_required(entries, "placements")?, "placements")?
            .iter()
            .map(ArrangerPlacement::from_expr)
            .collect::<Result<Vec<_>>>()?;
        Self::new(placements, lanes)
    }
}

impl ArrangerPlacement {
    /// Encodes the placement as a tagged `placement` expression map.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            ("tag", tag_expr("placement")),
            ("id", Expr::Symbol(self.id.clone())),
            ("playable", self.playable.to_expr()),
            ("at", time_expr(self.at)),
            (
                "duration",
                self.duration.map(time_expr).unwrap_or(Expr::Nil),
            ),
            ("lane", Expr::String(self.lane.0.clone())),
            (
                "targets",
                Expr::Vector(self.targets.iter().map(lane_target_expr).collect()),
            ),
            ("stretch", self.stretch.to_expr()),
            (
                "transform",
                Expr::Vector(
                    self.transform
                        .iter()
                        .map(PlacementTransform::to_expr)
                        .collect(),
                ),
            ),
            ("remap-pitch", self.remap_pitch.to_expr()),
            (
                "filter",
                self.filter
                    .as_ref()
                    .map(FilterRef::to_expr)
                    .unwrap_or(Expr::Nil),
            ),
            (
                "seed",
                self.seed
                    .map(|seed| Expr::String(seed.to_string()))
                    .unwrap_or(Expr::Nil),
            ),
            ("trace", Expr::Symbol(self.trace.symbol())),
        ])
    }

    /// Decodes a placement from a tagged `placement` expression map.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let entries = expr_map(expr, "arranger placement")?;
        expect_tag(entries, "placement", "arranger placement")?;
        let duration = match lookup_required(entries, "duration")? {
            Expr::Nil => None,
            expr => Some(time_from_expr(expr)?),
        };
        let placement = Self {
            id: expr_symbol(lookup_required(entries, "id")?, "placement id")?,
            playable: PlayableRef::from_expr(lookup_required(entries, "playable")?)?,
            at: time_from_expr(lookup_required(entries, "at")?)?,
            duration,
            lane: LaneId::new(expr_string(lookup_required(entries, "lane")?, "lane")?.to_owned()),
            targets: expr_vector(lookup_required(entries, "targets")?, "placement targets")?
                .iter()
                .map(lane_target_from_expr)
                .collect::<Result<Vec<_>>>()?,
            stretch: StretchPolicy::from_expr(lookup_required(entries, "stretch")?)?,
            transform: expr_vector(lookup_required(entries, "transform")?, "transform")?
                .iter()
                .map(PlacementTransform::from_expr)
                .collect::<Result<Vec<_>>>()?,
            remap_pitch: PitchRemap::from_expr(lookup_required(entries, "remap-pitch")?)?,
            filter: match lookup_required(entries, "filter")? {
                Expr::Nil => None,
                expr => Some(FilterRef::from_expr(expr)?),
            },
            seed: match lookup_required(entries, "seed")? {
                Expr::Nil => None,
                expr => Some(expr_u64(expr, "placement seed")?),
            },
            trace: TracePolicy::from_expr(lookup_required(entries, "trace")?)?,
        };
        placement.validate()?;
        Ok(placement)
    }
}

impl PlayableRef {
    /// Encodes the reference as a tagged `playable-ref` expression map.
    pub fn to_expr(&self) -> Expr {
        match self {
            Self::Inline(music) => map(vec![
                ("tag", tag_expr("playable-ref")),
                ("kind", tag_expr("inline")),
                ("value", music_to_expr(music)),
            ]),
            Self::Symbol(symbol) => map(vec![
                ("tag", tag_expr("playable-ref")),
                ("kind", tag_expr("symbol")),
                ("value", Expr::Symbol(symbol.clone())),
            ]),
        }
    }

    /// Decodes the reference from a tagged `playable-ref` expression map.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let entries = expr_map(expr, "playable ref")?;
        expect_tag(entries, "playable-ref", "playable ref")?;
        match symbol_name(lookup_required(entries, "kind")?, "playable ref kind")? {
            "inline" => Ok(Self::inline(music_from_expr(lookup_required(
                entries, "value",
            )?)?)),
            "symbol" => Ok(Self::symbol(expr_symbol(
                lookup_required(entries, "value")?,
                "playable symbol",
            )?)),
            _ => Err(Error::Eval("playable ref kind is invalid".to_owned())),
        }
    }
}

impl FilterRef {
    /// Encodes the filter as a tagged `filter` expression map.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            ("tag", tag_expr("filter")),
            ("id", Expr::Symbol(self.id.clone())),
            (
                "keep-lanes",
                Expr::Vector(
                    self.keep_lanes
                        .iter()
                        .map(|lane| Expr::String(lane.0.clone()))
                        .collect(),
                ),
            ),
        ])
    }

    /// Decodes the filter from a tagged `filter` expression map.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let entries = expr_map(expr, "filter")?;
        expect_tag(entries, "filter", "filter")?;
        Ok(Self {
            id: expr_symbol(lookup_required(entries, "id")?, "filter id")?,
            keep_lanes: expr_vector(lookup_required(entries, "keep-lanes")?, "keep lanes")?
                .iter()
                .map(|expr| Ok(LaneId::new(expr_string(expr, "keep lane")?.to_owned())))
                .collect::<Result<Vec<_>>>()?,
        })
    }
}

impl StretchPolicy {
    fn to_expr(&self) -> Expr {
        match self {
            Self::None => map(vec![
                ("tag", tag_expr("stretch")),
                ("kind", tag_expr("none")),
            ]),
            Self::TempoRatio(ratio) => map(vec![
                ("tag", tag_expr("stretch")),
                ("kind", tag_expr("tempo-ratio")),
                ("value", time_expr(*ratio)),
            ]),
            Self::TimeRatio(ratio) => map(vec![
                ("tag", tag_expr("stretch")),
                ("kind", tag_expr("time-ratio")),
                ("value", time_expr(*ratio)),
            ]),
            Self::FitToDuration => map(vec![
                ("tag", tag_expr("stretch")),
                ("kind", tag_expr("fit")),
            ]),
        }
    }

    fn from_expr(expr: &Expr) -> Result<Self> {
        let entries = expr_map(expr, "stretch policy")?;
        expect_tag(entries, "stretch", "stretch policy")?;
        match symbol_name(lookup_required(entries, "kind")?, "stretch kind")? {
            "none" => Ok(Self::None),
            "tempo-ratio" => Ok(Self::TempoRatio(time_from_expr(lookup_required(
                entries, "value",
            )?)?)),
            "time-ratio" => Ok(Self::TimeRatio(time_from_expr(lookup_required(
                entries, "value",
            )?)?)),
            "fit" => Ok(Self::FitToDuration),
            _ => Err(Error::Eval("stretch policy kind is invalid".to_owned())),
        }
    }
}

impl PlacementTransform {
    fn to_expr(&self) -> Expr {
        match self {
            Self::TransposeSemitones(semitones) => {
                value_transform("transpose-semitones", *semitones)
            }
            Self::TransposeOctaves(octaves) => value_transform("transpose-octaves", *octaves),
            Self::InvertAroundPitch(axis) => map(vec![
                ("tag", tag_expr("transform")),
                ("kind", tag_expr("invert-pitch")),
                ("value", pitch_expr(*axis)),
            ]),
            Self::InvertAroundPitchClass(axis) => value_transform("invert-pitch-class", axis.0),
            Self::Retrograde => map(vec![
                ("tag", tag_expr("transform")),
                ("kind", tag_expr("retrograde")),
            ]),
        }
    }

    fn from_expr(expr: &Expr) -> Result<Self> {
        let entries = expr_map(expr, "placement transform")?;
        expect_tag(entries, "transform", "placement transform")?;
        match symbol_name(lookup_required(entries, "kind")?, "transform kind")? {
            "transpose-semitones" => Ok(Self::TransposeSemitones(expr_i32(
                lookup_required(entries, "value")?,
                "transpose semitones",
            )?)),
            "transpose-octaves" => Ok(Self::TransposeOctaves(expr_i16(
                lookup_required(entries, "value")?,
                "transpose octaves",
            )?)),
            "invert-pitch" => Ok(Self::InvertAroundPitch(pitch_from_expr(lookup_required(
                entries, "value",
            )?)?)),
            "invert-pitch-class" => Ok(Self::InvertAroundPitchClass(
                PitchClass::new(expr_u8(lookup_required(entries, "value")?, "pitch class")?)
                    .map_err(|_| Error::Eval("pitch class is invalid".to_owned()))?,
            )),
            "retrograde" => Ok(Self::Retrograde),
            _ => Err(Error::Eval(
                "placement transform kind is invalid".to_owned(),
            )),
        }
    }
}

impl PitchRemap {
    fn to_expr(&self) -> Expr {
        match self {
            Self::None => map(vec![
                ("tag", tag_expr("pitch-remap")),
                ("kind", tag_expr("none")),
            ]),
            Self::Chromatic(semitones) => value_remap("chromatic", *semitones),
            Self::PitchClass { from, to } => map(vec![
                ("tag", tag_expr("pitch-remap")),
                ("kind", tag_expr("pitch-class")),
                ("from", Expr::String(from.0.to_string())),
                ("to", Expr::String(to.0.to_string())),
            ]),
            Self::DrumKey(items) => map(vec![
                ("tag", tag_expr("pitch-remap")),
                ("kind", tag_expr("drum-key")),
                (
                    "items",
                    Expr::Vector(items.iter().map(drum_key_expr).collect()),
                ),
            ]),
            Self::ScaleDegree(symbol) => symbolic_remap_expr("scale-degree", symbol),
            Self::ChordTone(symbol) => symbolic_remap_expr("chord-tone", symbol),
            Self::Tuning(symbol) => symbolic_remap_expr("tuning", symbol),
            Self::Vector(symbol) => symbolic_remap_expr("vector", symbol),
            Self::Matrix(symbol) => symbolic_remap_expr("matrix", symbol),
            Self::Callable(symbol) => symbolic_remap_expr("callable", symbol),
        }
    }

    fn from_expr(expr: &Expr) -> Result<Self> {
        let entries = expr_map(expr, "pitch remap")?;
        expect_tag(entries, "pitch-remap", "pitch remap")?;
        match symbol_name(lookup_required(entries, "kind")?, "pitch remap kind")? {
            "none" => Ok(Self::None),
            "chromatic" => Ok(Self::Chromatic(expr_i32(
                lookup_required(entries, "value")?,
                "chromatic remap",
            )?)),
            "pitch-class" => Ok(Self::PitchClass {
                from: PitchClass::new(expr_u8(lookup_required(entries, "from")?, "from")?)
                    .map_err(|_| Error::Eval("source pitch class is invalid".to_owned()))?,
                to: PitchClass::new(expr_u8(lookup_required(entries, "to")?, "to")?)
                    .map_err(|_| Error::Eval("target pitch class is invalid".to_owned()))?,
            }),
            "drum-key" => expr_vector(lookup_required(entries, "items")?, "drum key items")?
                .iter()
                .map(drum_key_from_expr)
                .collect::<Result<Vec<_>>>()
                .map(Self::DrumKey),
            "scale-degree" => symbolic_remap(entries, Self::ScaleDegree),
            "chord-tone" => symbolic_remap(entries, Self::ChordTone),
            "tuning" => symbolic_remap(entries, Self::Tuning),
            "vector" => symbolic_remap(entries, Self::Vector),
            "matrix" => symbolic_remap(entries, Self::Matrix),
            "callable" => symbolic_remap(entries, Self::Callable),
            _ => Err(Error::Eval("pitch remap kind is invalid".to_owned())),
        }
    }
}

impl TracePolicy {
    /// Returns the `music/arranger` symbol that names this trace policy.
    pub fn symbol(self) -> Symbol {
        match self {
            Self::Off => tag("trace-off"),
            Self::Diagnostics => tag("trace-diagnostics"),
            Self::Full => tag("trace-full"),
        }
    }

    fn from_expr(expr: &Expr) -> Result<Self> {
        match symbol_name(expr, "trace policy")? {
            "trace-off" => Ok(Self::Off),
            "trace-diagnostics" => Ok(Self::Diagnostics),
            "trace-full" => Ok(Self::Full),
            _ => Err(Error::Eval("trace policy is invalid".to_owned())),
        }
    }
}

impl PartialEq for Arranger {
    fn eq(&self, other: &Self) -> bool {
        self.to_expr().canonical_eq(&other.to_expr())
    }
}

impl Eq for Arranger {}

impl PartialEq for ArrangerPlacement {
    fn eq(&self, other: &Self) -> bool {
        self.to_expr().canonical_eq(&other.to_expr())
    }
}

impl Eq for ArrangerPlacement {}

impl PartialEq for PlayableRef {
    fn eq(&self, other: &Self) -> bool {
        self.to_expr().canonical_eq(&other.to_expr())
    }
}

impl Eq for PlayableRef {}

fn time_expr(time: Time) -> Expr {
    map(vec![
        ("numer", Expr::String(time.numer().to_string())),
        ("denom", Expr::String(time.denom().to_string())),
    ])
}

fn time_from_expr(expr: &Expr) -> Result<Time> {
    let entries = expr_map(expr, "time")?;
    let denominator = expr_i64(lookup_required(entries, "denom")?, "time denominator")?;
    if denominator == 0 {
        return Err(Error::Eval("time denominator cannot be zero".to_owned()));
    }
    Ok(Time::new(
        expr_i64(lookup_required(entries, "numer")?, "time numerator")?,
        denominator,
    ))
}

fn pitch_expr(pitch: Pitch) -> Expr {
    Expr::String(
        pitch
            .to_midi()
            .map(|midi| format!("midi:{midi}"))
            .unwrap_or_else(|| format!("semitone:{}", pitch.semitone())),
    )
}

fn pitch_from_expr(expr: &Expr) -> Result<Pitch> {
    let value = expr_string(expr, "pitch")?;
    if let Some(midi) = value.strip_prefix("midi:") {
        return Ok(Pitch::from_midi(
            midi.parse::<u8>()
                .map_err(|_| Error::Eval("MIDI pitch is invalid".to_owned()))?,
        ));
    }
    if let Some(semitone) = value.strip_prefix("semitone:") {
        return Ok(Pitch::from_semitone(semitone.parse::<i32>().map_err(
            |_| Error::Eval("semitone pitch is invalid".to_owned()),
        )?));
    }
    crate::parse_pitch(value).map_err(|_| Error::Eval("pitch is invalid".to_owned()))
}

fn lane_target_expr(target: &LaneTarget) -> Expr {
    match target {
        LaneTarget::Instrument(symbol) => target_expr("instrument", symbol),
        LaneTarget::Stream(symbol) => target_expr("stream", symbol),
        LaneTarget::Control(symbol) => target_expr("control", symbol),
        LaneTarget::None => map(vec![("kind", tag_expr("none"))]),
    }
}

fn lane_target_from_expr(expr: &Expr) -> Result<LaneTarget> {
    let entries = expr_map(expr, "lane target")?;
    match symbol_name(lookup_required(entries, "kind")?, "lane target kind")? {
        "instrument" => Ok(LaneTarget::Instrument(expr_symbol(
            lookup_required(entries, "symbol")?,
            "target symbol",
        )?)),
        "stream" => Ok(LaneTarget::Stream(expr_symbol(
            lookup_required(entries, "symbol")?,
            "target symbol",
        )?)),
        "control" => Ok(LaneTarget::Control(expr_symbol(
            lookup_required(entries, "symbol")?,
            "target symbol",
        )?)),
        "none" => Ok(LaneTarget::None),
        _ => Err(Error::Eval("lane target kind is invalid".to_owned())),
    }
}

fn value_transform<T: ToString>(kind: &'static str, value: T) -> Expr {
    map(vec![
        ("tag", tag_expr("transform")),
        ("kind", tag_expr(kind)),
        ("value", Expr::String(value.to_string())),
    ])
}

fn value_remap<T: ToString>(kind: &'static str, value: T) -> Expr {
    map(vec![
        ("tag", tag_expr("pitch-remap")),
        ("kind", tag_expr(kind)),
        ("value", Expr::String(value.to_string())),
    ])
}

fn drum_key_expr((from, to): &(u8, u8)) -> Expr {
    map(vec![
        ("from", Expr::String(from.to_string())),
        ("to", Expr::String(to.to_string())),
    ])
}

fn drum_key_from_expr(expr: &Expr) -> Result<(u8, u8)> {
    let item = expr_map(expr, "drum key item")?;
    Ok((
        expr_u8(lookup_required(item, "from")?, "drum key from")?,
        expr_u8(lookup_required(item, "to")?, "drum key to")?,
    ))
}

fn symbolic_remap_expr(kind: &'static str, symbol: &Symbol) -> Expr {
    map(vec![
        ("tag", tag_expr("pitch-remap")),
        ("kind", tag_expr(kind)),
        ("symbol", Expr::Symbol(symbol.clone())),
    ])
}

fn symbolic_remap(
    entries: &[(Expr, Expr)],
    build: impl FnOnce(Symbol) -> PitchRemap,
) -> Result<PitchRemap> {
    Ok(build(expr_symbol(
        lookup_required(entries, "symbol")?,
        "pitch remap symbol",
    )?))
}

fn target_expr(kind: &'static str, symbol: &Symbol) -> Expr {
    map(vec![
        ("kind", tag_expr(kind)),
        ("symbol", Expr::Symbol(symbol.clone())),
    ])
}

fn map(entries: Vec<(&'static str, Expr)>) -> Expr {
    Expr::Map(
        entries
            .into_iter()
            .map(|(key, value)| (field(key), value))
            .collect(),
    )
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym(NS, name)
}

fn tag(name: &'static str) -> Symbol {
    Symbol::qualified(NS, name)
}

fn tag_expr(name: &'static str) -> Expr {
    Expr::Symbol(tag(name))
}

fn expr_map<'a>(expr: &'a Expr, context: &str) -> Result<&'a [(Expr, Expr)]> {
    match expr {
        Expr::Map(entries) => Ok(entries),
        _ => Err(Error::Eval(format!("{context} must be a map"))),
    }
}

fn expr_vector<'a>(expr: &'a Expr, context: &str) -> Result<&'a [Expr]> {
    match expr {
        Expr::Vector(items) => Ok(items),
        _ => Err(Error::Eval(format!("{context} must be a vector"))),
    }
}

fn optional_vector<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Result<&'a [Expr]> {
    match lookup(entries, name) {
        Some(expr) => expr_vector(expr, name),
        None => Ok(&[]),
    }
}

fn lookup_required<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Result<&'a Expr> {
    lookup(entries, name).ok_or_else(|| Error::Eval(format!("arranger field is missing: {name}")))
}

fn lookup<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    entries.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol)
            if symbol.namespace.as_deref() == Some(NS) && symbol.name.as_ref() == name =>
        {
            Some(value)
        }
        _ => None,
    })
}

fn expect_tag(entries: &[(Expr, Expr)], name: &'static str, context: &str) -> Result<()> {
    match lookup(entries, "tag") {
        Some(Expr::Symbol(symbol))
            if symbol.namespace.as_deref() == Some(NS) && symbol.name.as_ref() == name =>
        {
            Ok(())
        }
        Some(_) => Err(Error::Eval(format!("{context} tag is invalid"))),
        None => Err(Error::Eval(format!("{context} tag is missing"))),
    }
}

fn symbol_name<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::Symbol(symbol) if symbol.namespace.as_deref() == Some(NS) => Ok(symbol.name.as_ref()),
        _ => Err(Error::Eval(format!("{context} must be an arranger symbol"))),
    }
}

fn expr_symbol(expr: &Expr, context: &str) -> Result<Symbol> {
    match expr {
        Expr::Symbol(symbol) => Ok(symbol.clone()),
        _ => Err(Error::Eval(format!("{context} must be a symbol"))),
    }
}

fn expr_string<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::String(value) => Ok(value),
        _ => Err(Error::Eval(format!("{context} must be a string"))),
    }
}

fn expr_i16(expr: &Expr, context: &str) -> Result<i16> {
    parse_number(expr, context)
}

fn expr_i32(expr: &Expr, context: &str) -> Result<i32> {
    parse_number(expr, context)
}

fn expr_i64(expr: &Expr, context: &str) -> Result<i64> {
    parse_number(expr, context)
}

fn expr_u8(expr: &Expr, context: &str) -> Result<u8> {
    parse_number(expr, context)
}

fn expr_u64(expr: &Expr, context: &str) -> Result<u64> {
    parse_number(expr, context)
}

fn parse_number<T>(expr: &Expr, context: &str) -> Result<T>
where
    T: std::str::FromStr,
{
    let text = match expr {
        Expr::String(value) => value.as_str(),
        Expr::Number(value) => value.canonical.as_str(),
        _ => return Err(Error::Eval(format!("{context} must be a number"))),
    };
    text.parse()
        .map_err(|_| Error::Eval(format!("{context} is invalid")))
}
