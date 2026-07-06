use std::collections::BTreeMap;

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_value::access;

use sim_lib_stream_core::{
    BufferPolicy, ClockDomain, StreamCassette, StreamDirection, StreamItem, StreamMedia,
    StreamMetadata, StreamPacket, StreamStats, TransportProfile,
};

use crate::freeze::stable_hash;
use crate::{
    Channel, LaneId, Music, NoteEvent, PerformanceEvent, PerformanceIntent, PerformanceNoteKey,
    PianoRoll, Pitch, PlayContext, PlayEvent, Tick, stable_event_order, tick_to_kernel_tick,
};

/// A captured recording of performance events with a content-addressed cassette.
///
/// Stores the raw [`PerformanceEvent`](crate::PerformanceEvent)s plus a derived
/// stream cassette and a stable content hash, and can replay, convert to notes, or
/// render to [`Music`](crate::Music).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerformanceTake {
    /// Symbol of the source that captured the take.
    pub source_id: Symbol,
    /// Symbol identifying the take.
    pub take_id: Symbol,
    /// Captured performance events in order.
    pub events: Vec<PerformanceEvent>,
    cassette: StreamCassette,
    content_hash: String,
}

impl PerformanceTake {
    /// Builds a take from events, deriving its cassette and content hash.
    pub fn new(source_id: Symbol, take_id: Symbol, events: Vec<PerformanceEvent>) -> Result<Self> {
        let cassette = performance_cassette_from_events(&source_id, &take_id, &events)?;
        let content_hash = performance_cassette_hash(&cassette);
        Ok(Self {
            source_id,
            take_id,
            events,
            cassette,
            content_hash,
        })
    }

    /// Returns the stream cassette derived from the take's events.
    pub fn cassette(&self) -> &StreamCassette {
        &self.cassette
    }

    /// Returns the stable content hash of the take's cassette.
    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }

    /// Decodes the take's events back from its cassette.
    pub fn replay_events(&self) -> Result<Vec<PerformanceEvent>> {
        performance_events_from_cassette(&self.cassette)
    }

    /// Recomputes the content hash from the cassette-replayed events.
    pub fn replay_content_hash(&self) -> Result<String> {
        let events = self.replay_events()?;
        let cassette = performance_cassette_from_events(&self.source_id, &self.take_id, &events)?;
        Ok(performance_cassette_hash(&cassette))
    }

    /// Pairs note-on/note-off events into timed [`NoteEvent`](crate::NoteEvent)s.
    pub fn note_events(&self) -> Result<Vec<NoteEvent>> {
        performance_note_events(&self.events)
    }

    /// Returns the take's notes wrapped as [`PlayEvent`](crate::PlayEvent)s.
    pub fn play_events(&self) -> Result<Vec<PlayEvent>> {
        Ok(self
            .note_events()?
            .into_iter()
            .map(PlayEvent::Note)
            .collect())
    }

    /// Renders the take into a piano-roll [`Music`](crate::Music) clip.
    pub fn as_clip(&self) -> Result<Music> {
        Ok(Music::PianoRoll(PianoRoll::from_performance_take(self)?))
    }

    /// Returns `cx` with the take's play events merged into its upstream, ordered.
    pub fn player_chain_context(&self, cx: &PlayContext) -> Result<PlayContext> {
        let mut next = cx.clone();
        next.upstream.extend(self.play_events()?);
        stable_event_order(&mut next.upstream);
        Ok(next)
    }
}

impl PerformanceEvent {
    /// Decodes a performance event from an [`Expr`] map.
    ///
    /// Validates the `event` kind symbol and reads the lane, source, input time,
    /// time, and intent fields.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let Expr::Map(entries) = expr else {
            return Err(Error::Eval("performance event must be a map".to_owned()));
        };
        let kind = symbol_field(entries, "event")?;
        if *kind != crate::LaneKind::Performance.symbol() {
            return Err(Error::Eval(format!(
                "performance event has wrong kind {}",
                kind.as_qualified_str()
            )));
        }
        Ok(Self {
            lane_id: LaneId::new(string_field(entries, "lane")?),
            source_id: symbol_field(entries, "source")?.clone(),
            input_time: tick_field(entries, "input-time")?,
            time: tick_field(entries, "time")?,
            intent: PerformanceIntent::from_expr(field(entries, "intent")?)?,
        })
    }
}

/// Returns the qualified data-kind symbol for performance-event stream packets.
pub fn performance_event_data_kind() -> Symbol {
    Symbol::qualified("music/performance", "event")
}

fn performance_cassette_from_events(
    source_id: &Symbol,
    take_id: &Symbol,
    events: &[PerformanceEvent],
) -> Result<StreamCassette> {
    let metadata = performance_metadata(source_id, take_id, events.len())?;
    let items = events
        .iter()
        .map(|event| {
            StreamItem::with_ticks(
                StreamPacket::data(performance_event_data_kind(), event.to_expr()),
                vec![tick_to_kernel_tick(
                    event.time,
                    ClockDomain::MidiTick.symbol(),
                )],
            )
        })
        .collect::<Result<Vec<_>>>()?;
    StreamCassette::from_items(
        metadata,
        items,
        TransportProfile::memory_local(),
        StreamStats {
            yielded: events.len() as u64,
            closed: true,
            ..StreamStats::default()
        },
    )
}

fn performance_metadata(
    source_id: &Symbol,
    take_id: &Symbol,
    event_count: usize,
) -> Result<StreamMetadata> {
    Ok(StreamMetadata::new(
        Symbol::qualified(
            "music/performance-cassette",
            format!("{}:{}", source_id.name, take_id.name),
        ),
        StreamMedia::Data,
        StreamDirection::Source,
        ClockDomain::MidiTick.symbol(),
        BufferPolicy::bounded(event_count.max(1))?,
    ))
}

fn performance_events_from_cassette(cassette: &StreamCassette) -> Result<Vec<PerformanceEvent>> {
    cassette
        .items()?
        .into_iter()
        .map(|item| match item.packet() {
            StreamPacket::Data(packet) if packet.kind == performance_event_data_kind() => {
                PerformanceEvent::from_expr(&packet.payload)
            }
            _ => Err(Error::Eval(
                "stream cassette item is not a performance event".to_owned(),
            )),
        })
        .collect()
}

fn performance_cassette_hash(cassette: &StreamCassette) -> String {
    stable_hash("performance-cassette", &cassette.to_expr())
}

fn performance_note_events(events: &[PerformanceEvent]) -> Result<Vec<NoteEvent>> {
    let mut state = PerformanceClipState::default();
    for event in events {
        state.observe(event)?;
    }
    state.finish()
}

#[derive(Clone, Debug)]
struct OpenPerformanceNote {
    lane_id: LaneId,
    pitch: Pitch,
    velocity: u8,
    channel: Channel,
    started_at: Tick,
    released_while_sustained: bool,
}

#[derive(Clone, Debug, Default)]
struct PerformanceClipState {
    sustain_pedal: bool,
    active: BTreeMap<PerformanceNoteKey, OpenPerformanceNote>,
    notes: Vec<NoteEvent>,
}

impl PerformanceClipState {
    fn observe(&mut self, event: &PerformanceEvent) -> Result<()> {
        match &event.intent {
            PerformanceIntent::NoteOn {
                pitch,
                velocity,
                channel,
            } => {
                let key = PerformanceNoteKey::new(*channel, *pitch);
                if self.active.contains_key(&key) {
                    self.close_note(key, event.time)?;
                }
                self.active.insert(
                    key,
                    OpenPerformanceNote {
                        lane_id: event.lane_id.clone(),
                        pitch: *pitch,
                        velocity: *velocity,
                        channel: *channel,
                        started_at: event.time,
                        released_while_sustained: false,
                    },
                );
            }
            PerformanceIntent::NoteOff { pitch, channel, .. } => {
                let key = PerformanceNoteKey::new(*channel, *pitch);
                if self.sustain_pedal {
                    if let Some(note) = self.active.get_mut(&key) {
                        note.released_while_sustained = true;
                    }
                } else {
                    self.close_note(key, event.time)?;
                }
            }
            PerformanceIntent::Sustain { down, .. } => {
                self.sustain_pedal = *down;
                if !down {
                    let released = self
                        .active
                        .iter()
                        .filter_map(|(key, note)| note.released_while_sustained.then_some(*key))
                        .collect::<Vec<_>>();
                    for key in released {
                        self.close_note(key, event.time)?;
                    }
                }
            }
            PerformanceIntent::Panic => {
                let keys = self.active.keys().copied().collect::<Vec<_>>();
                for key in keys {
                    self.close_note(key, event.time)?;
                }
                self.sustain_pedal = false;
            }
            PerformanceIntent::Aftertouch { .. }
            | PerformanceIntent::PitchBend { .. }
            | PerformanceIntent::Parameter { .. } => {}
        }
        Ok(())
    }

    fn close_note(&mut self, key: PerformanceNoteKey, end: Tick) -> Result<()> {
        let Some(note) = self.active.remove(&key) else {
            return Ok(());
        };
        let end = end.quantize(note.started_at.tpq);
        if end.ticks < note.started_at.ticks {
            return Err(Error::Eval(
                "performance note-off precedes note-on".to_owned(),
            ));
        }
        self.notes.push(NoteEvent {
            lane_id: note.lane_id,
            time: note.started_at,
            duration: Tick::new(end.ticks - note.started_at.ticks, note.started_at.tpq)
                .map_err(music_err)?,
            pitch: note.pitch,
            velocity: note.velocity,
            channel: note.channel,
        });
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<NoteEvent>> {
        if !self.active.is_empty() {
            return Err(Error::Eval(
                "cannot convert performance take with held notes".to_owned(),
            ));
        }
        self.notes.sort_by(|left, right| {
            left.time
                .ticks
                .cmp(&right.time.ticks)
                .then_with(|| left.lane_id.cmp(&right.lane_id))
                .then_with(|| left.pitch.cmp(&right.pitch))
        });
        Ok(self.notes)
    }
}

fn field<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Result<&'a Expr> {
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

fn string_field<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Result<&'a str> {
    access::entry_required_str(entries, name, "string field")
}

fn symbol_field<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Result<&'a Symbol> {
    access::entry_required_sym(entries, name, "symbol field")
}

fn i64_field(entries: &[(Expr, Expr)], name: &str) -> Result<i64> {
    string_field(entries, name)?
        .parse::<i64>()
        .map_err(|err| Error::Eval(format!("invalid {name}: {err}")))
}

fn u32_field(entries: &[(Expr, Expr)], name: &str) -> Result<u32> {
    string_field(entries, name)?
        .parse::<u32>()
        .map_err(|err| Error::Eval(format!("invalid {name}: {err}")))
}

fn tick_field(entries: &[(Expr, Expr)], name: &str) -> Result<Tick> {
    let Expr::Map(entries) = field(entries, name)? else {
        return Err(Error::Eval(format!("{name} field must be a tick map")));
    };
    Tick::new(i64_field(entries, "ticks")?, u32_field(entries, "tpq")?).map_err(music_err)
}

fn music_err(err: impl std::fmt::Display) -> Error {
    Error::Eval(err.to_string())
}
