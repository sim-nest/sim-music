use sim_kernel::{Error, Expr, NumberLiteral, Result, Symbol};

use crate::{
    AdsrSettings, LfoSettings, ModSource, ModTarget, ModulationMatrix, ModulationRoute,
    OscillatorKind, TempoSync,
};

const LIB_NS: &str = "audio-synth";

/// A complete sound preset for the subtractive synth: oscillator, amplitude
/// envelope, LFO, modulation routing, and polyphony and tone settings.
#[derive(Clone, Debug, PartialEq)]
pub struct SynthPreset {
    /// The preset name.
    pub name: String,
    /// The oscillator waveform kind.
    pub oscillator: OscillatorKind,
    /// The wavetable samples used when the oscillator is wavetable-based.
    pub wavetable: Vec<f32>,
    /// The amplitude ADSR envelope.
    pub amp_envelope: AdsrSettings,
    /// The low-frequency oscillator settings.
    pub lfo: LfoSettings,
    /// The modulation routing matrix.
    pub modulation: ModulationMatrix,
    /// The maximum number of simultaneous voices.
    pub max_voices: usize,
    /// The output amplitude gain.
    pub amp_gain: f32,
    /// The filter cutoff frequency in hertz.
    pub filter_cutoff_hz: f32,
    /// The pulse width for pulse-capable oscillators.
    pub pulse_width: f32,
}

impl SynthPreset {
    /// Renders the preset as a tagged map expression.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                field("tag"),
                Expr::Symbol(Symbol::qualified(LIB_NS, "preset")),
            ),
            (field("name"), Expr::String(self.name.clone())),
            (
                field("oscillator"),
                Expr::Symbol(Symbol::qualified(LIB_NS, self.oscillator.as_str())),
            ),
            (
                field("wavetable"),
                Expr::Vector(self.wavetable.iter().copied().map(number_f32).collect()),
            ),
            (field("amp-envelope"), envelope_to_expr(self.amp_envelope)),
            (field("lfo"), lfo_to_expr(self.lfo)),
            (
                field("modulation"),
                Expr::Vector(
                    self.modulation
                        .routes()
                        .iter()
                        .copied()
                        .map(route_to_expr)
                        .collect(),
                ),
            ),
            (field("max-voices"), number_usize(self.max_voices)),
            (field("amp-gain"), number_f32(self.amp_gain)),
            (field("filter-cutoff-hz"), number_f32(self.filter_cutoff_hz)),
            (field("pulse-width"), number_f32(self.pulse_width)),
        ])
    }

    /// Parses a preset from a tagged map expression, erroring on a missing tag
    /// or malformed fields.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let map = expr_map(expr, "synth preset")?;
        match lookup(map, "tag") {
            Some(Expr::Symbol(symbol)) if is_symbol(symbol, LIB_NS, "preset") => {}
            Some(_) => return Err(Error::Eval("audio synth preset tag is invalid".to_owned())),
            None => return Err(missing("tag")),
        }
        let oscillator =
            symbol_name(lookup_required(map, "oscillator")?, "oscillator").and_then(|name| {
                OscillatorKind::from_name(name)
                    .ok_or_else(|| Error::Eval(format!("unknown audio synth oscillator: {name}")))
            })?;
        let wavetable = expr_vector(lookup_required(map, "wavetable")?, "wavetable")?
            .iter()
            .map(|expr| expr_f32(expr, "wavetable sample"))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            name: expr_string(lookup_required(map, "name")?, "name")?.to_owned(),
            oscillator,
            wavetable,
            amp_envelope: envelope_from_expr(lookup_required(map, "amp-envelope")?)?,
            lfo: lfo_from_expr(lookup_required(map, "lfo")?)?,
            modulation: modulation_from_expr(lookup_required(map, "modulation")?)?,
            max_voices: expr_usize(lookup_required(map, "max-voices")?, "max-voices")?,
            amp_gain: expr_f32(lookup_required(map, "amp-gain")?, "amp-gain")?,
            filter_cutoff_hz: expr_f32(
                lookup_required(map, "filter-cutoff-hz")?,
                "filter-cutoff-hz",
            )?,
            pulse_width: expr_f32(lookup_required(map, "pulse-width")?, "pulse-width")?,
        })
    }

    /// Returns a monophonic PolyBLEP lead preset with an LFO-to-pitch vibrato
    /// route.
    pub fn mono_polyblep_lead() -> Self {
        let mut modulation = ModulationMatrix::default();
        modulation.push(ModulationRoute {
            source: ModSource::Lfo1,
            target: ModTarget::OscPitchSemitones,
            amount: 0.05,
        });
        Self {
            name: "mono-polyblep-lead".to_owned(),
            max_voices: 1,
            modulation,
            ..Self::default()
        }
    }
}

impl Default for SynthPreset {
    fn default() -> Self {
        Self {
            name: "init-polyblep".to_owned(),
            oscillator: OscillatorKind::PolyBlepSaw,
            wavetable: Vec::new(),
            amp_envelope: AdsrSettings::default(),
            lfo: LfoSettings::default(),
            modulation: ModulationMatrix::default(),
            max_voices: 8,
            amp_gain: 0.25,
            filter_cutoff_hz: 8_000.0,
            pulse_width: 0.5,
        }
    }
}

fn envelope_to_expr(settings: AdsrSettings) -> Expr {
    Expr::Map(vec![
        (field("attack-s"), number_f32(settings.attack_s)),
        (field("decay-s"), number_f32(settings.decay_s)),
        (field("sustain-level"), number_f32(settings.sustain_level)),
        (field("release-s"), number_f32(settings.release_s)),
    ])
}

fn envelope_from_expr(expr: &Expr) -> Result<AdsrSettings> {
    let map = expr_map(expr, "amp envelope")?;
    Ok(AdsrSettings {
        attack_s: expr_f32(lookup_required(map, "attack-s")?, "attack-s")?,
        decay_s: expr_f32(lookup_required(map, "decay-s")?, "decay-s")?,
        sustain_level: expr_f32(lookup_required(map, "sustain-level")?, "sustain-level")?,
        release_s: expr_f32(lookup_required(map, "release-s")?, "release-s")?,
    })
}

fn lfo_to_expr(settings: LfoSettings) -> Expr {
    Expr::Map(vec![
        (
            field("waveform"),
            Expr::Symbol(Symbol::qualified(LIB_NS, settings.waveform.as_str())),
        ),
        (field("rate-hz"), number_f32(settings.rate_hz)),
        (field("depth"), number_f32(settings.depth)),
        (
            field("tempo-sync-beats"),
            settings
                .tempo_sync
                .map(|sync| number_f32(sync.beats_per_cycle))
                .unwrap_or(Expr::Nil),
        ),
    ])
}

fn lfo_from_expr(expr: &Expr) -> Result<LfoSettings> {
    let map = expr_map(expr, "lfo")?;
    let waveform =
        symbol_name(lookup_required(map, "waveform")?, "lfo waveform").and_then(|name| {
            OscillatorKind::from_name(name)
                .ok_or_else(|| Error::Eval(format!("unknown audio synth lfo waveform: {name}")))
        })?;
    let tempo_sync = match lookup_required(map, "tempo-sync-beats")? {
        Expr::Nil => None,
        other => Some(TempoSync {
            beats_per_cycle: expr_f32(other, "tempo-sync-beats")?,
        }),
    };
    Ok(LfoSettings {
        waveform,
        rate_hz: expr_f32(lookup_required(map, "rate-hz")?, "rate-hz")?,
        depth: expr_f32(lookup_required(map, "depth")?, "depth")?,
        tempo_sync,
    })
}

fn route_to_expr(route: ModulationRoute) -> Expr {
    Expr::Map(vec![
        (
            field("source"),
            Expr::Symbol(Symbol::qualified(LIB_NS, route.source.as_str())),
        ),
        (
            field("target"),
            Expr::Symbol(Symbol::qualified(LIB_NS, route.target.as_str())),
        ),
        (field("amount"), number_f32(route.amount)),
    ])
}

fn modulation_from_expr(expr: &Expr) -> Result<ModulationMatrix> {
    let routes = expr_vector(expr, "modulation routes")?
        .iter()
        .map(route_from_expr)
        .collect::<Result<Vec<_>>>()?;
    Ok(ModulationMatrix::new(routes))
}

fn route_from_expr(expr: &Expr) -> Result<ModulationRoute> {
    let map = expr_map(expr, "modulation route")?;
    let source =
        symbol_name(lookup_required(map, "source")?, "modulation source").and_then(|name| {
            ModSource::from_name(name)
                .ok_or_else(|| Error::Eval(format!("unknown modulation source: {name}")))
        })?;
    let target =
        symbol_name(lookup_required(map, "target")?, "modulation target").and_then(|name| {
            ModTarget::from_name(name)
                .ok_or_else(|| Error::Eval(format!("unknown modulation target: {name}")))
        })?;
    Ok(ModulationRoute {
        source,
        target,
        amount: expr_f32(lookup_required(map, "amount")?, "modulation amount")?,
    })
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym(LIB_NS, name)
}

fn number_f32(value: f32) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "f64"),
        canonical: value.to_string(),
    })
}

fn number_usize(value: usize) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}

fn expr_map<'a>(expr: &'a Expr, context: &str) -> Result<&'a [(Expr, Expr)]> {
    match expr {
        Expr::Map(entries) => Ok(entries),
        other => Err(Error::Eval(format!(
            "expected {context} map, found {}",
            expr_kind(other)
        ))),
    }
}

fn expr_vector<'a>(expr: &'a Expr, context: &str) -> Result<&'a [Expr]> {
    match expr {
        Expr::Vector(items) => Ok(items),
        other => Err(Error::Eval(format!(
            "expected {context} vector, found {}",
            expr_kind(other)
        ))),
    }
}

fn expr_string<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::String(text) => Ok(text),
        other => Err(Error::Eval(format!(
            "expected {context} string, found {}",
            expr_kind(other)
        ))),
    }
}

fn symbol_name<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::Symbol(symbol) if symbol.namespace.as_deref() == Some(LIB_NS) => {
            Ok(symbol.name.as_ref())
        }
        Expr::String(text) => Ok(text),
        other => Err(Error::Eval(format!(
            "expected {context} audio synth symbol, found {}",
            expr_kind(other)
        ))),
    }
}

fn expr_f32(expr: &Expr, context: &str) -> Result<f32> {
    let text = match expr {
        Expr::Number(number) => number.canonical.as_str(),
        Expr::String(text) => text,
        other => {
            return Err(Error::Eval(format!(
                "expected {context} number, found {}",
                expr_kind(other)
            )));
        }
    };
    text.parse::<f32>()
        .map_err(|_| Error::Eval(format!("expected {context} f32 number, found {text}")))
}

fn expr_usize(expr: &Expr, context: &str) -> Result<usize> {
    let text = match expr {
        Expr::Number(number) => number.canonical.as_str(),
        Expr::String(text) => text,
        other => {
            return Err(Error::Eval(format!(
                "expected {context} integer, found {}",
                expr_kind(other)
            )));
        }
    };
    text.parse::<usize>()
        .map_err(|_| Error::Eval(format!("expected {context} usize number, found {text}")))
}

fn lookup_required<'a>(map: &'a [(Expr, Expr)], name: &str) -> Result<&'a Expr> {
    lookup(map, name).ok_or_else(|| missing(name))
}

fn lookup<'a>(map: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    map.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol) if is_symbol(symbol, LIB_NS, name) => Some(value),
        _ => None,
    })
}

fn is_symbol(symbol: &Symbol, namespace: &str, name: &str) -> bool {
    symbol.namespace.as_deref() == Some(namespace) && symbol.name.as_ref() == name
}

fn missing(field: &str) -> Error {
    Error::Eval(format!("audio synth preset field is missing: {field}"))
}

use sim_value::kind::expr_kind;
