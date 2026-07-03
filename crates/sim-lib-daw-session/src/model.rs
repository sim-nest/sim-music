use sim_kernel::{Error, Result, Symbol};
use sim_lib_audio_graph_core::{Patch, PortUri};
mod clip;
mod instrument;
mod plugin;
mod track;

pub use clip::{ClipSource, DawClip};
pub use instrument::{
    DawInstrumentInstance, DawInstrumentKind, DawSessionRoute, DawSessionRouteKind,
    instrument_session_fixture, instrument_session_fixture_names,
    instrument_session_render_smoke_command,
};
pub(crate) use instrument::{
    instrument_from_expr, instrument_to_expr, optional_expr_vector, route_from_expr, route_to_expr,
};
pub use plugin::{PluginChain, PluginSlot};
pub use track::{DawTrack, DawTrackKind};

/// Stable session data for a headless DAW workspace.
#[derive(Clone, Debug, PartialEq)]
pub struct DawSession {
    pub(crate) id: Symbol,
    pub(crate) name: String,
    pub(crate) sample_rate_hz: u32,
    pub(crate) patch: Patch,
    pub(crate) instrument_instances: Vec<DawInstrumentInstance>,
    pub(crate) routes: Vec<DawSessionRoute>,
    pub(crate) tracks: Vec<DawTrack>,
    pub(crate) buses: Vec<DawBus>,
    pub(crate) transport: DawTransport,
    pub(crate) recording: RecordingMetadata,
}

/// Mix bus metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DawBus {
    pub(crate) id: Symbol,
    pub(crate) name: String,
    pub(crate) channels: u16,
}

/// Playback transport metadata.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DawTransport {
    pub(crate) playing: bool,
    pub(crate) sample_pos: u64,
    pub(crate) tempo_bpm: f64,
}

/// Recording state and armed inputs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecordingMetadata {
    pub(crate) armed_tracks: Vec<Symbol>,
    pub(crate) input_ports: Vec<PortUri>,
    pub(crate) take_label: String,
}

impl DawSession {
    /// Creates an empty session with a single stereo `master` bus.
    ///
    /// Fails if the id or name is empty or the sample rate is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_daw_session::DawSession;
    ///
    /// let session = DawSession::new("set", "My Set", 48_000).unwrap();
    /// assert_eq!(session.sample_rate_hz(), 48_000);
    /// assert_eq!(session.buses().len(), 1);
    /// ```
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        sample_rate_hz: u32,
    ) -> Result<Self> {
        let id = symbol(id, "session id")?;
        let name = non_empty(name.into(), "session name")?;
        if sample_rate_hz == 0 {
            return Err(Error::Eval(
                "DAW session sample rate must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            id,
            name,
            sample_rate_hz,
            patch: Patch {
                nodes: Vec::new(),
                cables: Vec::new(),
            },
            instrument_instances: Vec::new(),
            routes: Vec::new(),
            tracks: Vec::new(),
            buses: vec![DawBus::new("master", "Master", 2)?],
            transport: DawTransport::default(),
            recording: RecordingMetadata::default(),
        })
    }

    /// Returns the session with its audio graph patch replaced.
    pub fn with_patch(mut self, patch: Patch) -> Self {
        self.patch = patch;
        self
    }

    /// Returns the session with its transport state replaced.
    pub fn with_transport(mut self, transport: DawTransport) -> Self {
        self.transport = transport;
        self
    }

    /// Returns the session with its recording metadata replaced.
    pub fn with_recording(mut self, recording: RecordingMetadata) -> Self {
        self.recording = recording;
        self
    }

    /// Appends a track, rejecting a duplicate track id.
    pub fn add_track(&mut self, track: DawTrack) -> Result<()> {
        if self.tracks.iter().any(|existing| existing.id == track.id) {
            return Err(Error::Eval(format!("duplicate DAW track id: {}", track.id)));
        }
        self.tracks.push(track);
        Ok(())
    }

    /// Registers an instrument instance, rejecting a duplicate id and requiring
    /// its graph node to already exist in the patch.
    pub fn add_instrument_instance(&mut self, instrument: DawInstrumentInstance) -> Result<()> {
        if self
            .instrument_instances
            .iter()
            .any(|existing| existing.id == instrument.id)
        {
            return Err(Error::Eval(format!(
                "duplicate DAW instrument id: {}",
                instrument.id
            )));
        }
        self.ensure_patch_node(&instrument.graph_node_id)?;
        self.instrument_instances.push(instrument);
        Ok(())
    }

    /// Adds a session route, requiring its target graph node to exist.
    pub fn add_route(&mut self, route: DawSessionRoute) -> Result<()> {
        self.ensure_patch_node(&route.target_node_id)?;
        self.routes.push(route);
        Ok(())
    }

    /// Appends a mix bus, rejecting a duplicate bus id.
    pub fn add_bus(&mut self, bus: DawBus) -> Result<()> {
        if self.buses.iter().any(|existing| existing.id == bus.id) {
            return Err(Error::Eval(format!("duplicate DAW bus id: {}", bus.id)));
        }
        self.buses.push(bus);
        Ok(())
    }

    /// Returns the session id symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the human-readable session name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the session sample rate in hertz.
    pub fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    /// Returns the underlying audio graph patch.
    pub fn patch(&self) -> &Patch {
        &self.patch
    }

    /// Returns the registered instrument instances.
    pub fn instrument_instances(&self) -> &[DawInstrumentInstance] {
        &self.instrument_instances
    }

    /// Returns the session routes.
    pub fn routes(&self) -> &[DawSessionRoute] {
        &self.routes
    }

    /// Returns the session tracks.
    pub fn tracks(&self) -> &[DawTrack] {
        &self.tracks
    }

    /// Returns the mix buses (always including `master`).
    pub fn buses(&self) -> &[DawBus] {
        &self.buses
    }

    /// Returns the current transport state.
    pub fn transport(&self) -> DawTransport {
        self.transport
    }

    /// Returns the recording metadata.
    pub fn recording(&self) -> &RecordingMetadata {
        &self.recording
    }

    fn ensure_patch_node(&self, node_id: &str) -> Result<()> {
        if self.patch.nodes.iter().any(|node| node.id == node_id) {
            Ok(())
        } else {
            Err(Error::Eval(format!(
                "DAW session route references unknown graph node: {node_id}"
            )))
        }
    }
}

impl DawBus {
    /// Creates a mix bus, rejecting an empty id/name or zero channel count.
    pub fn new(id: impl Into<String>, name: impl Into<String>, channels: u16) -> Result<Self> {
        if channels == 0 {
            return Err(Error::Eval(
                "DAW bus channel count must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            id: symbol(id, "bus id")?,
            name: non_empty(name.into(), "bus name")?,
            channels,
        })
    }

    /// Returns the bus id symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the bus name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the bus channel count.
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

impl Default for DawTransport {
    fn default() -> Self {
        Self {
            playing: false,
            sample_pos: 0,
            tempo_bpm: 120.0,
        }
    }
}

impl DawTransport {
    /// Creates a transport, requiring a finite, positive tempo.
    pub fn new(playing: bool, sample_pos: u64, tempo_bpm: f64) -> Result<Self> {
        if !tempo_bpm.is_finite() || tempo_bpm <= 0.0 {
            return Err(Error::Eval(
                "DAW transport tempo must be finite and positive".to_owned(),
            ));
        }
        Ok(Self {
            playing,
            sample_pos,
            tempo_bpm,
        })
    }

    /// Returns whether the transport is playing.
    pub fn playing(self) -> bool {
        self.playing
    }

    /// Returns the transport playhead position in samples.
    pub fn sample_pos(self) -> u64 {
        self.sample_pos
    }

    /// Returns the transport tempo in beats per minute.
    pub fn tempo_bpm(self) -> f64 {
        self.tempo_bpm
    }
}

impl RecordingMetadata {
    /// Creates recording metadata with the given take label and no armed inputs.
    pub fn new(take_label: impl Into<String>) -> Self {
        Self {
            take_label: take_label.into(),
            ..Self::default()
        }
    }

    /// Returns the metadata with an additional armed track symbol.
    pub fn with_armed_track(mut self, track: Symbol) -> Self {
        self.armed_tracks.push(track);
        self
    }

    /// Returns the metadata with an additional input port.
    pub fn with_input_port(mut self, port: PortUri) -> Self {
        self.input_ports.push(port);
        self
    }

    /// Returns the armed track symbols.
    pub fn armed_tracks(&self) -> &[Symbol] {
        &self.armed_tracks
    }

    /// Returns the configured input ports.
    pub fn input_ports(&self) -> &[PortUri] {
        &self.input_ports
    }

    /// Returns the take label.
    pub fn take_label(&self) -> &str {
        &self.take_label
    }
}

pub(super) fn symbol(value: impl Into<String>, label: &str) -> Result<Symbol> {
    let value = non_empty(value.into(), label)?;
    Ok(Symbol::new(value))
}

pub(super) fn non_empty(value: String, label: &str) -> Result<String> {
    if value.trim().is_empty() {
        return Err(Error::Eval(format!("DAW {label} must not be empty")));
    }
    Ok(value)
}
