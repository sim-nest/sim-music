use sim_kernel::{Expr, Result, Symbol};
use sim_lib_midi_core::MidiEvent;
use sim_lib_stream_core::{StreamItem, StreamPacket};

use crate::{Channel, LaneId, LaneKind, PerformanceIntent, Pitch, Tick, tick_to_kernel_tick};

/// A single scheduled event on a lane, tagged by its content kind.
///
/// Each variant carries a kind-specific payload and corresponds to a
/// [`LaneKind`](crate::LaneKind).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayEvent {
    /// A pitched note event.
    Note(NoteEvent),
    /// A raw MIDI event.
    Midi(MidiPlayEvent),
    /// A bare pitch event.
    Pitch(PitchEvent),
    /// A control-change event.
    Control(ControlEvent),
    /// An audio-frame event.
    Audio(AudioEvent),
    /// A playable-reference event.
    Playable(PlayableEvent),
    /// A performance-intent event.
    Performance(PerformanceEvent),
    /// A diagnostic message event.
    Diagnostic(DiagnosticEvent),
    /// A trace / debugging step event.
    Trace(TraceEvent),
}

impl PlayEvent {
    /// Returns the [`LaneKind`](crate::LaneKind) matching this event's variant.
    pub fn kind(&self) -> LaneKind {
        match self {
            Self::Note(_) => LaneKind::Note,
            Self::Midi(_) => LaneKind::Midi,
            Self::Pitch(_) => LaneKind::Pitch,
            Self::Control(_) => LaneKind::Control,
            Self::Audio(_) => LaneKind::Audio,
            Self::Playable(_) => LaneKind::Playable,
            Self::Performance(_) => LaneKind::Performance,
            Self::Diagnostic(_) => LaneKind::Diagnostic,
            Self::Trace(_) => LaneKind::Trace,
        }
    }

    /// Returns the id of the lane this event belongs to.
    pub fn lane_id(&self) -> &LaneId {
        match self {
            Self::Note(event) => &event.lane_id,
            Self::Midi(event) => &event.lane_id,
            Self::Pitch(event) => &event.lane_id,
            Self::Control(event) => &event.lane_id,
            Self::Audio(event) => &event.lane_id,
            Self::Playable(event) => &event.lane_id,
            Self::Performance(event) => &event.lane_id,
            Self::Diagnostic(event) => &event.lane_id,
            Self::Trace(event) => &event.lane_id,
        }
    }

    /// Returns the start time of this event in ticks.
    pub fn time(&self) -> Tick {
        match self {
            Self::Note(event) => event.time,
            Self::Midi(event) => event.event.time,
            Self::Pitch(event) => event.time,
            Self::Control(event) => event.time,
            Self::Audio(event) => event.time,
            Self::Playable(event) => event.time,
            Self::Performance(event) => event.time,
            Self::Diagnostic(event) => event.time,
            Self::Trace(event) => event.time,
        }
    }

    /// Encodes this event as a `StreamItem` timestamped against `clock`.
    pub fn to_stream_item(&self, clock: Symbol) -> Result<StreamItem> {
        StreamItem::with_ticks(
            StreamPacket::data(play_event_data_kind(), self.to_expr()),
            vec![tick_to_kernel_tick(self.time(), clock)],
        )
    }

    /// Encodes this event as a kernel `Expr` map.
    pub fn to_expr(&self) -> Expr {
        match self {
            Self::Note(event) => event.to_expr(),
            Self::Midi(event) => event.to_expr(),
            Self::Pitch(event) => event.to_expr(),
            Self::Control(event) => event.to_expr(),
            Self::Audio(event) => event.to_expr(),
            Self::Playable(event) => event.to_expr(),
            Self::Performance(event) => event.to_expr(),
            Self::Diagnostic(event) => event.to_expr(),
            Self::Trace(event) => event.to_expr(),
        }
    }
}

/// A pitched note with duration, velocity, and channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteEvent {
    /// Lane the note plays on.
    pub lane_id: LaneId,
    /// Start time in ticks.
    pub time: Tick,
    /// Duration in ticks.
    pub duration: Tick,
    /// Sounding pitch.
    pub pitch: Pitch,
    /// MIDI-style velocity (0-127).
    pub velocity: u8,
    /// Output channel.
    pub channel: Channel,
}

impl NoteEvent {
    /// Encodes this note as a kernel `Expr` map.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            ("event", Expr::Symbol(LaneKind::Note.symbol())),
            ("lane", Expr::String(self.lane_id.0.clone())),
            ("time", tick_expr(self.time)),
            ("duration", tick_expr(self.duration)),
            ("pitch", Expr::String(pitch_label(self.pitch))),
            ("velocity", Expr::String(self.velocity.to_string())),
            ("channel", Expr::String(self.channel.0.to_string())),
        ])
    }
}

/// A raw MIDI event bound to a lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiPlayEvent {
    /// Lane the MIDI event plays on.
    pub lane_id: LaneId,
    /// The wrapped MIDI event, including its own time and payload.
    pub event: MidiEvent,
}

impl MidiPlayEvent {
    /// Encodes this MIDI event as a kernel `Expr` map.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            ("event", Expr::Symbol(LaneKind::Midi.symbol())),
            ("lane", Expr::String(self.lane_id.0.clone())),
            ("time", tick_expr(self.event.time)),
            ("payload", Expr::String(format!("{:?}", self.event.payload))),
        ])
    }
}

/// A bare pitch event without duration or velocity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchEvent {
    /// Lane the pitch plays on.
    pub lane_id: LaneId,
    /// Time in ticks.
    pub time: Tick,
    /// Sounding pitch.
    pub pitch: Pitch,
}

impl PitchEvent {
    /// Encodes this pitch event as a kernel `Expr` map.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            ("event", Expr::Symbol(LaneKind::Pitch.symbol())),
            ("lane", Expr::String(self.lane_id.0.clone())),
            ("time", tick_expr(self.time)),
            ("pitch", Expr::String(pitch_label(self.pitch))),
        ])
    }
}

/// A control-change event setting a named control to a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlEvent {
    /// Lane the control change applies to.
    pub lane_id: LaneId,
    /// Time in ticks.
    pub time: Tick,
    /// Symbol naming the control being changed.
    pub control: Symbol,
    /// New control value.
    pub value: i64,
}

impl ControlEvent {
    /// Encodes this control event as a kernel `Expr` map.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            ("event", Expr::Symbol(LaneKind::Control.symbol())),
            ("lane", Expr::String(self.lane_id.0.clone())),
            ("time", tick_expr(self.time)),
            ("control", Expr::Symbol(self.control.clone())),
            ("value", Expr::String(self.value.to_string())),
        ])
    }
}

/// An audio-frame event covering a span of frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioEvent {
    /// Lane the audio plays on.
    pub lane_id: LaneId,
    /// Time in ticks.
    pub time: Tick,
    /// Number of audio frames.
    pub frames: u32,
}

impl AudioEvent {
    /// Encodes this audio event as a kernel `Expr` map.
    pub fn to_expr(&self) -> Expr {
        timed_count_expr(
            LaneKind::Audio,
            &self.lane_id,
            self.time,
            "frames",
            self.frames,
        )
    }
}

/// A reference to a named playable to trigger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayableEvent {
    /// Lane the playable triggers on.
    pub lane_id: LaneId,
    /// Time in ticks.
    pub time: Tick,
    /// Symbol naming the playable.
    pub playable: Symbol,
}

impl PlayableEvent {
    /// Encodes this playable event as a kernel `Expr` map.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            ("event", Expr::Symbol(LaneKind::Playable.symbol())),
            ("lane", Expr::String(self.lane_id.0.clone())),
            ("time", tick_expr(self.time)),
            ("playable", Expr::Symbol(self.playable.clone())),
        ])
    }
}

/// A performance-intent event tying a rendered time back to its input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerformanceEvent {
    /// Lane the performance event plays on.
    pub lane_id: LaneId,
    /// Symbol identifying the originating source.
    pub source_id: Symbol,
    /// Input (pre-performance) time in ticks.
    pub input_time: Tick,
    /// Rendered (post-performance) time in ticks.
    pub time: Tick,
    /// Performance intent applied to the event.
    pub intent: PerformanceIntent,
}

impl PerformanceEvent {
    /// Encodes this performance event as a kernel `Expr` map.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            ("event", Expr::Symbol(LaneKind::Performance.symbol())),
            ("lane", Expr::String(self.lane_id.0.clone())),
            ("source", Expr::Symbol(self.source_id.clone())),
            ("input-time", tick_expr(self.input_time)),
            ("time", tick_expr(self.time)),
            ("intent", self.intent.to_expr()),
        ])
    }
}

/// A diagnostic message emitted on a lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticEvent {
    /// Lane the diagnostic is reported on.
    pub lane_id: LaneId,
    /// Time in ticks.
    pub time: Tick,
    /// Human-readable diagnostic text.
    pub message: String,
}

impl DiagnosticEvent {
    /// Encodes this diagnostic event as a kernel `Expr` map.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            ("event", Expr::Symbol(LaneKind::Diagnostic.symbol())),
            ("lane", Expr::String(self.lane_id.0.clone())),
            ("time", tick_expr(self.time)),
            ("message", Expr::String(self.message.clone())),
        ])
    }
}

/// A trace / debugging step marker on a lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceEvent {
    /// Lane the trace marker is recorded on.
    pub lane_id: LaneId,
    /// Time in ticks.
    pub time: Tick,
    /// Monotonic step counter.
    pub step: u64,
}

impl TraceEvent {
    /// Encodes this trace event as a kernel `Expr` map.
    pub fn to_expr(&self) -> Expr {
        timed_count_expr(LaneKind::Trace, &self.lane_id, self.time, "step", self.step)
    }
}

/// Returns the qualified data-kind symbol used for encoded play events.
pub fn play_event_data_kind() -> Symbol {
    Symbol::qualified("music/play", "event")
}

/// Sorts events in place into a deterministic order.
///
/// Orders by time, then lane id, then kind, with the encoded `Expr` as a final
/// tie-breaker so equal-timed events on the same lane stay stable.
pub fn stable_event_order(events: &mut [PlayEvent]) {
    events.sort_by(|left, right| {
        left.time()
            .ticks
            .cmp(&right.time().ticks)
            .then_with(|| left.lane_id().cmp(right.lane_id()))
            .then_with(|| left.kind().cmp(&right.kind()))
            .then_with(|| format!("{:?}", left.to_expr()).cmp(&format!("{:?}", right.to_expr())))
    });
}

fn timed_count_expr<T: ToString>(
    kind: LaneKind,
    lane_id: &LaneId,
    time: Tick,
    field: &'static str,
    value: T,
) -> Expr {
    map(vec![
        ("event", Expr::Symbol(kind.symbol())),
        ("lane", Expr::String(lane_id.0.clone())),
        ("time", tick_expr(time)),
        (field, Expr::String(value.to_string())),
    ])
}

fn tick_expr(tick: Tick) -> Expr {
    map(vec![
        ("ticks", Expr::String(tick.ticks.to_string())),
        ("tpq", Expr::String(tick.tpq.to_string())),
    ])
}

fn pitch_label(pitch: Pitch) -> String {
    pitch
        .to_midi()
        .map(|midi| format!("midi:{midi}"))
        .unwrap_or_else(|| format!("semitone:{}", pitch.semitone()))
}

fn map(entries: Vec<(&'static str, Expr)>) -> Expr {
    Expr::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Expr::Symbol(Symbol::new(key)), value))
            .collect(),
    )
}
