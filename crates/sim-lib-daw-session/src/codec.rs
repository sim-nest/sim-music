use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_audio_graph_core::PortUri;
use sim_lib_plugin_core::{PluginFormat, PluginId, PluginState};

use crate::{
    ClipSource, DawBus, DawClip, DawSession, DawTrack, DawTrackKind, DawTransport, PluginChain,
    PluginSlot, RecordingMetadata,
    expr_util::{
        expect_tag, expr_bool, expr_f32, expr_f64, expr_map, expr_string, expr_symbol, expr_u16,
        expr_u32, expr_u64, expr_vector, field, lookup_required, number_f32, number_f64,
        number_u16, number_u32, number_u64, tag,
    },
    model::{
        instrument_from_expr, instrument_to_expr, optional_expr_vector, route_from_expr,
        route_to_expr,
    },
};

impl DawSession {
    /// Encodes the full session -- patch, instruments, routes, tracks, buses,
    /// transport, and recording metadata -- as a tagged expression map.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            (field("tag"), tag("session")),
            (field("id"), Expr::Symbol(self.id.clone())),
            (field("name"), Expr::String(self.name.clone())),
            (field("sample-rate-hz"), number_u32(self.sample_rate_hz)),
            (field("patch"), self.patch.to_expr()),
            (
                field("instruments"),
                Expr::Vector(
                    self.instrument_instances
                        .iter()
                        .map(instrument_to_expr)
                        .collect(),
                ),
            ),
            (
                field("routes"),
                Expr::Vector(self.routes.iter().map(route_to_expr).collect()),
            ),
            (
                field("tracks"),
                Expr::Vector(self.tracks.iter().map(track_to_expr).collect()),
            ),
            (
                field("buses"),
                Expr::Vector(self.buses.iter().map(bus_to_expr).collect()),
            ),
            (field("transport"), transport_to_expr(self.transport)),
            (field("recording"), recording_to_expr(&self.recording)),
        ])
    }

    /// Alias for [`DawSession::to_expr`] naming the save direction.
    pub fn save_expr(&self) -> Expr {
        self.to_expr()
    }

    /// Decodes a session from its expression form, validating tags, the sample
    /// rate, and every nested record.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let map = expr_map(expr, "DAW session")?;
        expect_tag(map, "session", "DAW session")?;
        let sample_rate_hz = expr_u32(lookup_required(map, "sample-rate-hz")?, "sample rate")?;
        if sample_rate_hz == 0 {
            return Err(Error::Eval(
                "DAW session sample rate must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            id: expr_symbol(lookup_required(map, "id")?, "session id")?,
            name: expr_string(lookup_required(map, "name")?, "session name")?.to_owned(),
            sample_rate_hz,
            patch: sim_lib_audio_graph_core::Patch::from_expr(lookup_required(map, "patch")?)?,
            instrument_instances: optional_expr_vector(map, "instruments", "instruments")?
                .iter()
                .map(instrument_from_expr)
                .collect::<Result<Vec<_>>>()?,
            routes: optional_expr_vector(map, "routes", "routes")?
                .iter()
                .map(route_from_expr)
                .collect::<Result<Vec<_>>>()?,
            tracks: expr_vector(lookup_required(map, "tracks")?, "tracks")?
                .iter()
                .map(track_from_expr)
                .collect::<Result<Vec<_>>>()?,
            buses: expr_vector(lookup_required(map, "buses")?, "buses")?
                .iter()
                .map(bus_from_expr)
                .collect::<Result<Vec<_>>>()?,
            transport: transport_from_expr(lookup_required(map, "transport")?)?,
            recording: recording_from_expr(lookup_required(map, "recording")?)?,
        })
    }

    /// Alias for [`DawSession::from_expr`] naming the load direction.
    pub fn load_expr(expr: &Expr) -> Result<Self> {
        Self::from_expr(expr)
    }
}

fn track_to_expr(track: &DawTrack) -> Expr {
    Expr::Map(vec![
        (field("tag"), tag("track")),
        (field("id"), Expr::Symbol(track.id.clone())),
        (field("name"), Expr::String(track.name.clone())),
        (
            field("kind"),
            Expr::Symbol(Symbol::qualified("daw-track-kind", track.kind.as_str())),
        ),
        (field("channels"), number_u16(track.channels)),
        (
            field("bus"),
            track
                .bus
                .as_ref()
                .map(|bus| Expr::Symbol(bus.clone()))
                .unwrap_or(Expr::Nil),
        ),
        (
            field("clips"),
            Expr::Vector(track.clips.iter().map(clip_to_expr).collect()),
        ),
        (field("plugin-chain"), chain_to_expr(&track.plugin_chain)),
        (field("armed"), Expr::Bool(track.armed)),
        (field("muted"), Expr::Bool(track.muted)),
        (field("solo"), Expr::Bool(track.solo)),
    ])
}

fn track_from_expr(expr: &Expr) -> Result<DawTrack> {
    let map = expr_map(expr, "DAW track")?;
    expect_tag(map, "track", "DAW track")?;
    Ok(DawTrack {
        id: expr_symbol(lookup_required(map, "id")?, "track id")?,
        name: expr_string(lookup_required(map, "name")?, "track name")?.to_owned(),
        kind: track_kind_from_expr(lookup_required(map, "kind")?)?,
        channels: expr_u16(lookup_required(map, "channels")?, "track channels")?,
        bus: match lookup_required(map, "bus")? {
            Expr::Nil => None,
            expr => Some(expr_symbol(expr, "track bus")?),
        },
        clips: expr_vector(lookup_required(map, "clips")?, "track clips")?
            .iter()
            .map(clip_from_expr)
            .collect::<Result<Vec<_>>>()?,
        plugin_chain: chain_from_expr(lookup_required(map, "plugin-chain")?)?,
        armed: expr_bool(lookup_required(map, "armed")?, "track armed")?,
        muted: expr_bool(lookup_required(map, "muted")?, "track muted")?,
        solo: expr_bool(lookup_required(map, "solo")?, "track solo")?,
    })
}

fn track_kind_from_expr(expr: &Expr) -> Result<DawTrackKind> {
    match expr {
        Expr::Symbol(symbol) if symbol.namespace.as_deref() == Some("daw-track-kind") => {
            DawTrackKind::parse_name(symbol.name.as_ref())
        }
        Expr::String(text) => DawTrackKind::parse_name(text),
        _ => Err(Error::Eval("DAW track kind is invalid".to_owned())),
    }
}

fn bus_to_expr(bus: &DawBus) -> Expr {
    Expr::Map(vec![
        (field("tag"), tag("bus")),
        (field("id"), Expr::Symbol(bus.id.clone())),
        (field("name"), Expr::String(bus.name.clone())),
        (field("channels"), number_u16(bus.channels)),
    ])
}

fn bus_from_expr(expr: &Expr) -> Result<DawBus> {
    let map = expr_map(expr, "DAW bus")?;
    expect_tag(map, "bus", "DAW bus")?;
    let channels = expr_u16(lookup_required(map, "channels")?, "bus channels")?;
    if channels == 0 {
        return Err(Error::Eval(
            "DAW bus channel count must be greater than zero".to_owned(),
        ));
    }
    Ok(DawBus {
        id: expr_symbol(lookup_required(map, "id")?, "bus id")?,
        name: expr_string(lookup_required(map, "name")?, "bus name")?.to_owned(),
        channels,
    })
}

fn clip_to_expr(clip: &DawClip) -> Expr {
    Expr::Map(vec![
        (field("tag"), tag("clip")),
        (field("id"), Expr::Symbol(clip.id.clone())),
        (field("start-frame"), number_u64(clip.start_frame)),
        (field("frames"), number_u64(clip.frames)),
        (field("source"), source_to_expr(&clip.source)),
        (field("gain"), number_f32(clip.gain)),
    ])
}

fn clip_from_expr(expr: &Expr) -> Result<DawClip> {
    let map = expr_map(expr, "DAW clip")?;
    expect_tag(map, "clip", "DAW clip")?;
    let frames = expr_u64(lookup_required(map, "frames")?, "clip frames")?;
    if frames == 0 {
        return Err(Error::Eval(
            "DAW clip frame count must be greater than zero".to_owned(),
        ));
    }
    let source = source_from_expr(lookup_required(map, "source")?)?;
    source.validate()?;
    let gain = expr_f32(lookup_required(map, "gain")?, "clip gain")?;
    Ok(DawClip {
        id: expr_symbol(lookup_required(map, "id")?, "clip id")?,
        start_frame: expr_u64(lookup_required(map, "start-frame")?, "clip start")?,
        frames,
        source,
        gain,
    })
}

fn source_to_expr(source: &ClipSource) -> Expr {
    let (kind, value) = match source {
        ClipSource::Silence => ("silence", Expr::Nil),
        ClipSource::Constant(value) => ("constant", number_f32(*value)),
        ClipSource::PatchNode(node) => ("patch-node", Expr::String(node.clone())),
        ClipSource::Arranger(arranger) => ("arranger", arranger.to_expr()),
    };
    Expr::Map(vec![
        (field("tag"), tag("clip-source")),
        (
            field("kind"),
            Expr::Symbol(Symbol::qualified("daw-clip-source", kind)),
        ),
        (field("value"), value),
    ])
}

fn source_from_expr(expr: &Expr) -> Result<ClipSource> {
    let map = expr_map(expr, "DAW clip source")?;
    expect_tag(map, "clip-source", "DAW clip source")?;
    let Expr::Symbol(kind) = lookup_required(map, "kind")? else {
        return Err(Error::Eval("DAW clip source kind is invalid".to_owned()));
    };
    if kind.namespace.as_deref() != Some("daw-clip-source") {
        return Err(Error::Eval("DAW clip source kind is invalid".to_owned()));
    }
    match kind.name.as_ref() {
        "silence" => Ok(ClipSource::Silence),
        "constant" => Ok(ClipSource::Constant(expr_f32(
            lookup_required(map, "value")?,
            "clip source value",
        )?)),
        "patch-node" => ClipSource::patch_node(
            expr_string(lookup_required(map, "value")?, "clip source node")?.to_owned(),
        ),
        "arranger" => Ok(ClipSource::Arranger(
            sim_lib_music_core::Arranger::from_expr(lookup_required(map, "value")?)?,
        )),
        other => Err(Error::Eval(format!("unknown DAW clip source: {other}"))),
    }
}

fn chain_to_expr(chain: &PluginChain) -> Expr {
    Expr::Vector(chain.slots.iter().map(slot_to_expr).collect())
}

fn chain_from_expr(expr: &Expr) -> Result<PluginChain> {
    Ok(PluginChain::new(
        expr_vector(expr, "plugin chain")?
            .iter()
            .map(slot_from_expr)
            .collect::<Result<Vec<_>>>()?,
    ))
}

fn slot_to_expr(slot: &PluginSlot) -> Expr {
    Expr::Map(vec![
        (field("tag"), tag("plugin-slot")),
        (field("id"), Expr::Symbol(slot.id.clone())),
        (
            field("format"),
            Expr::String(slot.plugin.format.as_str().to_owned()),
        ),
        (
            field("stable-id"),
            Expr::String(slot.plugin.stable_id.clone()),
        ),
        (field("state"), slot.state.to_expr()),
        (field("bypassed"), Expr::Bool(slot.bypassed)),
    ])
}

fn slot_from_expr(expr: &Expr) -> Result<PluginSlot> {
    let map = expr_map(expr, "plugin slot")?;
    expect_tag(map, "plugin-slot", "plugin slot")?;
    let format = plugin_format(expr_string(
        lookup_required(map, "format")?,
        "plugin format",
    )?)?;
    Ok(PluginSlot {
        id: expr_symbol(lookup_required(map, "id")?, "plugin slot id")?,
        plugin: PluginId::new(
            format,
            expr_string(lookup_required(map, "stable-id")?, "plugin stable id")?.to_owned(),
        )?,
        state: PluginState::from_expr(lookup_required(map, "state")?)?,
        bypassed: expr_bool(lookup_required(map, "bypassed")?, "bypassed")?,
    })
}

fn plugin_format(text: &str) -> Result<PluginFormat> {
    match text {
        "clap" => Ok(PluginFormat::Clap),
        "lv2" => Ok(PluginFormat::Lv2),
        "vst3" => Ok(PluginFormat::Vst3),
        "sim" => Ok(PluginFormat::Sim),
        _ => Err(Error::Eval(format!("unknown plugin format: {text}"))),
    }
}

fn transport_to_expr(transport: DawTransport) -> Expr {
    Expr::Map(vec![
        (field("tag"), tag("transport")),
        (field("playing"), Expr::Bool(transport.playing)),
        (field("sample-pos"), number_u64(transport.sample_pos)),
        (field("tempo-bpm"), number_f64(transport.tempo_bpm)),
    ])
}

fn transport_from_expr(expr: &Expr) -> Result<DawTransport> {
    let map = expr_map(expr, "DAW transport")?;
    expect_tag(map, "transport", "DAW transport")?;
    DawTransport::new(
        expr_bool(lookup_required(map, "playing")?, "transport playing")?,
        expr_u64(lookup_required(map, "sample-pos")?, "transport sample-pos")?,
        expr_f64(lookup_required(map, "tempo-bpm")?, "transport tempo")?,
    )
}

fn recording_to_expr(recording: &RecordingMetadata) -> Expr {
    Expr::Map(vec![
        (field("tag"), tag("recording")),
        (
            field("armed-tracks"),
            Expr::Vector(
                recording
                    .armed_tracks
                    .iter()
                    .cloned()
                    .map(Expr::Symbol)
                    .collect(),
            ),
        ),
        (
            field("input-ports"),
            Expr::Vector(
                recording
                    .input_ports
                    .iter()
                    .map(|port| Expr::String(port.to_string()))
                    .collect(),
            ),
        ),
        (
            field("take-label"),
            Expr::String(recording.take_label.clone()),
        ),
    ])
}

fn recording_from_expr(expr: &Expr) -> Result<RecordingMetadata> {
    let map = expr_map(expr, "DAW recording metadata")?;
    expect_tag(map, "recording", "DAW recording metadata")?;
    let armed_tracks = expr_vector(lookup_required(map, "armed-tracks")?, "armed tracks")?
        .iter()
        .map(|expr| expr_symbol(expr, "armed track"))
        .collect::<Result<Vec<_>>>()?;
    let input_ports = expr_vector(lookup_required(map, "input-ports")?, "input ports")?
        .iter()
        .map(|expr| expr_string(expr, "input port")?.parse::<PortUri>())
        .collect::<Result<Vec<_>>>()?;
    Ok(RecordingMetadata {
        armed_tracks,
        input_ports,
        take_label: expr_string(lookup_required(map, "take-label")?, "take label")?.to_owned(),
    })
}
