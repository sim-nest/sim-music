use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_stream_core::{
    BufferPolicy, ClockDomain, LatencyClass, StreamDirection, StreamEnvelope, StreamMedia,
    StreamMetadata, StreamValue, TransportProfile,
};

use crate::{
    AtomRef, LaneDescriptor, LaneId, LaneKind, LaneTarget, Music, MusicObject, NoteEvent,
    PlayEvent, TempoMapRef, Time, TimeRange, time_to_tick,
};

/// Stream of rendered play items produced by a [`Playable`].
pub type PlayStream = StreamValue;

/// Rendering context handed to a [`Playable`] during prepare/render/freeze.
///
/// Carries the transport identity, tempo map, clock resolution, time window,
/// and any upstream events that a render run depends on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayContext {
    /// Symbol identifying the transport that drives this render.
    pub transport: Symbol,
    /// Reference to the tempo map used to relate musical and wall-clock time.
    pub tempo: TempoMapRef,
    /// Audio sample rate in hertz.
    pub sample_rate: u32,
    /// Pulses (ticks) per quarter note for tick-domain conversion.
    pub ppq: u32,
    /// Time window the render is clipped to.
    pub range: TimeRange,
    /// Deterministic seed for any randomized rendering.
    pub seed: u64,
    /// Capability tokens the render site advertises.
    pub capabilities: Vec<String>,
    /// Hint describing where the render is expected to execute.
    pub site: SiteHint,
    /// Events fed in from upstream stages of a chain.
    pub upstream: Vec<PlayEvent>,
}

impl PlayContext {
    /// Creates a context over `range` with offline transport defaults.
    pub fn new(range: TimeRange) -> Self {
        Self {
            transport: Symbol::qualified("music/transport", "offline"),
            tempo: TempoMapRef::default(),
            sample_rate: 48_000,
            ppq: range.start.tpq,
            range,
            seed: 0,
            capabilities: Vec::new(),
            site: SiteHint::LocalCoroutine,
            upstream: Vec::new(),
        }
    }

    /// Builds stream metadata for a source stream of `item_count` data items.
    pub fn stream_metadata(&self, id: Symbol, item_count: usize) -> Result<StreamMetadata> {
        Ok(StreamMetadata::new(
            id,
            StreamMedia::Data,
            StreamDirection::Source,
            ClockDomain::MidiTick.symbol(),
            BufferPolicy::bounded(item_count.max(1))?,
        ))
    }
}

/// Hint about the execution site a render is expected to run on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteHint {
    /// Same-thread cooperative coroutine.
    LocalCoroutine,
    /// Dedicated worker thread.
    Thread,
    /// Separate operating-system process.
    Process,
    /// WebAssembly guest in a browser.
    BrowserWasm,
    /// Browser audio worklet thread.
    AudioWorklet,
    /// Node reachable over the local network.
    Lan,
}

impl SiteHint {
    /// Returns the qualified symbol naming this site hint.
    pub fn symbol(self) -> Symbol {
        match self {
            Self::LocalCoroutine => Symbol::qualified("site", "local-coroutine"),
            Self::Thread => Symbol::qualified("site", "thread"),
            Self::Process => Symbol::qualified("site", "process"),
            Self::BrowserWasm => Symbol::qualified("site", "browser-wasm"),
            Self::AudioWorklet => Symbol::qualified("site", "audio-worklet"),
            Self::Lan => Symbol::qualified("site", "lan"),
        }
    }
}

/// Static description of a [`Playable`]: its identity, lanes, and clocking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayableDescriptor {
    /// Symbol identifying the playable.
    pub id: Symbol,
    /// Output lanes the playable produces.
    pub lanes: Vec<LaneDescriptor>,
    /// Clock domain the rendered events are timed in.
    pub clock_domain: ClockDomain,
    /// Latency class the playable targets.
    pub latency_class: LatencyClass,
    /// Object shape exposed for protocol dispatch.
    pub shape: PlayableShape,
}

/// Shape record describing the playable protocol surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayableShape {
    /// Symbol naming the shape.
    pub symbol: Symbol,
    /// Field/method names the shape exposes.
    pub fields: Vec<String>,
}

impl PlayableShape {
    /// Returns the canonical shape for music playable objects.
    pub fn music_object() -> Self {
        Self {
            symbol: Symbol::qualified("music/shape", "playable"),
            fields: vec![
                "describe".to_owned(),
                "prepare".to_owned(),
                "render-range".to_owned(),
                "render-preview".to_owned(),
                "freeze".to_owned(),
                "as-shape".to_owned(),
            ],
        }
    }

    /// Encodes the shape as an expression map.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                Expr::Symbol(Symbol::new("shape")),
                Expr::Symbol(self.symbol.clone()),
            ),
            (
                Expr::Symbol(Symbol::new("fields")),
                Expr::List(self.fields.iter().cloned().map(Expr::String).collect()),
            ),
        ])
    }

    /// Decodes a shape from an expression map produced by [`PlayableShape::to_expr`].
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let Expr::Map(entries) = expr else {
            return Err(Error::Eval("playable shape must be a map".to_owned()));
        };
        let symbol = entries
            .iter()
            .find_map(|(key, value)| match (key, value) {
                (Expr::Symbol(key), Expr::Symbol(symbol)) if key.name.as_ref() == "shape" => {
                    Some(symbol.clone())
                }
                _ => None,
            })
            .ok_or_else(|| Error::Eval("playable shape missing shape field".to_owned()))?;
        let fields = entries
            .iter()
            .find_map(|(key, value)| match (key, value) {
                (Expr::Symbol(key), Expr::List(fields)) if key.name.as_ref() == "fields" => {
                    Some(fields)
                }
                _ => None,
            })
            .ok_or_else(|| Error::Eval("playable shape missing fields".to_owned()))?
            .iter()
            .map(|field| match field {
                Expr::String(value) => Ok(value.clone()),
                _ => Err(Error::Eval("playable shape field must be text".to_owned())),
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { symbol, fields })
    }
}

/// Fully rendered snapshot of a [`Playable`] with a content hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenPlayable {
    /// Descriptor of the frozen playable.
    pub descriptor: PlayableDescriptor,
    /// Rendered events in stable order.
    pub events: Vec<PlayEvent>,
    /// Stable hash of the rendered content.
    pub content_hash: String,
}

/// Object that can describe, render, and freeze itself into play events.
pub trait Playable {
    /// Returns the static descriptor for this playable.
    fn describe(&self) -> Result<PlayableDescriptor>;

    /// Prepares the playable for rendering under `cx`; defaults to a no-op.
    fn prepare(&mut self, _cx: &PlayContext) -> Result<()> {
        Ok(())
    }

    /// Renders the playable over the context's time range into a stream.
    fn render_range(&self, cx: &PlayContext) -> Result<PlayStream>;

    /// Renders a preview stream; defaults to [`Playable::render_range`].
    fn render_preview(&self, cx: &PlayContext) -> Result<PlayStream> {
        self.render_range(cx)
    }

    /// Renders and captures the playable into a [`FrozenPlayable`].
    fn freeze(&self, cx: &PlayContext) -> Result<FrozenPlayable>;

    /// Returns the object shape; defaults to [`PlayableShape::music_object`].
    fn as_shape(&self) -> PlayableShape {
        PlayableShape::music_object()
    }
}

impl Playable for Music {
    fn describe(&self) -> Result<PlayableDescriptor> {
        default_music_descriptor(Symbol::qualified(
            "music/playable",
            self.kind().to_ascii_lowercase(),
        ))
    }

    fn render_range(&self, cx: &PlayContext) -> Result<PlayStream> {
        let mut events = render_music_events(self, cx)?;
        crate::stable_event_order(&mut events);
        let items = events
            .iter()
            .map(|event| event.to_stream_item(ClockDomain::MidiTick.symbol()))
            .collect::<Result<Vec<_>>>()?;
        let metadata = cx.stream_metadata(
            Symbol::qualified("music/play-stream", self.kind()),
            items.len(),
        )?;
        Ok(StreamValue::pull(metadata, items))
    }

    fn freeze(&self, cx: &PlayContext) -> Result<FrozenPlayable> {
        let mut events = render_music_events(self, cx)?;
        crate::stable_event_order(&mut events);
        let descriptor = self.describe()?;
        let content_hash = stable_content_hash(&events, cx);
        Ok(FrozenPlayable {
            descriptor,
            events,
            content_hash,
        })
    }
}

/// Renders a music object into clipped, stably ordered note play events.
///
/// Walks the object's voices, clips each note to the context range, and
/// prepends any upstream events from `cx`.
pub fn render_music_events(object: &dyn MusicObject, cx: &PlayContext) -> Result<Vec<PlayEvent>> {
    let mut atoms = Vec::new();
    object.voices(Time::from_integer(0), &mut atoms);
    let mut events = cx.upstream.clone();
    let note_lane = LaneId::new("notes");
    for atom in atoms {
        if let AtomRef::Note(note) = atom.atom {
            let onset = time_to_tick(atom.onset, cx.ppq).map_err(music_err)?;
            let duration = time_to_tick(note.duration, cx.ppq).map_err(music_err)?;
            let Some((time, duration)) = cx.range.clip_span(onset, duration) else {
                continue;
            };
            events.push(PlayEvent::Note(NoteEvent {
                lane_id: note_lane.clone(),
                time,
                duration,
                pitch: note.pitch,
                velocity: note.velocity,
                channel: note.channel,
            }));
        }
    }
    crate::stable_event_order(&mut events);
    Ok(events)
}

/// Drains a play stream into transport envelopes over a memory-local profile.
pub fn stream_envelopes(stream: &PlayStream) -> Result<Vec<StreamEnvelope>> {
    let metadata = stream.metadata().clone();
    let items = stream.take_packets(usize::MAX)?;
    items
        .iter()
        .enumerate()
        .map(|(sequence, item)| {
            StreamEnvelope::from_item_with_profile(
                &metadata,
                sequence as u64,
                item,
                TransportProfile::memory_local(),
            )
        })
        .collect()
}

fn default_music_descriptor(id: Symbol) -> Result<PlayableDescriptor> {
    Ok(PlayableDescriptor {
        id,
        lanes: vec![
            LaneDescriptor::new(
                LaneId::new("notes"),
                LaneKind::Note,
                LaneTarget::Instrument(Symbol::qualified("music/target", "default")),
                0,
            )
            .map_err(music_err)?,
        ],
        clock_domain: ClockDomain::MidiTick,
        latency_class: LatencyClass::Interactive,
        shape: PlayableShape::music_object(),
    })
}

fn stable_content_hash(events: &[PlayEvent], cx: &PlayContext) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in format!("{events:?}:{}", cx.seed).bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn music_err(err: crate::MusicError) -> Error {
    Error::Eval(err.to_string())
}
