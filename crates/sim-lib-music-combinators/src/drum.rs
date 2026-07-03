use sim_lib_music_core::{
    Channel, LaneId, NoteEvent, Pitch, PlayEvent, Tick, TraceEvent, stable_event_order,
};

/// Half-open step span during which a pattern is active.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PatternRegion {
    /// First active step.
    pub start_step: u64,
    /// One past the last active step.
    pub end_step: u64,
}

impl PatternRegion {
    /// Creates a region, clamping the end to be at least the start.
    pub fn new(start_step: u64, end_step: u64) -> Self {
        Self {
            start_step,
            end_step: end_step.max(start_step),
        }
    }

    /// Reports whether a step falls within the region.
    pub fn contains(self, step: u64) -> bool {
        step >= self.start_step && step < self.end_step
    }
}

/// Region-based automation deciding which steps a pattern plays.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PatternAutomation {
    /// Regions where the pattern is active; empty means always active.
    pub active_regions: Vec<PatternRegion>,
}

impl PatternAutomation {
    /// Returns automation that is active on every step.
    pub fn always() -> Self {
        Self::default()
    }

    /// Returns automation active only within the given regions.
    pub fn active_in(regions: Vec<PatternRegion>) -> Self {
        Self {
            active_regions: regions,
        }
    }

    /// Reports whether the pattern is active at the given step.
    pub fn is_active(&self, step: u64) -> bool {
        self.active_regions.is_empty()
            || self
                .active_regions
                .iter()
                .any(|region| region.contains(step))
    }
}

/// Diagnostic record describing one drum hit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrumStepTrace {
    /// Name of the player that produced the hit.
    pub player: &'static str,
    /// Lane the hit belongs to.
    pub lane_id: LaneId,
    /// Hit time.
    pub time: Tick,
    /// Step index.
    pub step: u64,
    /// Drum sound name.
    pub sound: String,
    /// MIDI key the sound resolved to.
    pub key: u8,
    /// Hit velocity.
    pub velocity: u8,
    /// Whether the step produced a hit.
    pub active: bool,
}

/// Rendered drum-pattern output: play events and per-hit traces.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DrumPatternRender {
    /// Ordered play events.
    pub events: Vec<PlayEvent>,
    /// Per-hit diagnostic traces.
    pub traces: Vec<DrumStepTrace>,
}

impl DrumPatternRender {
    /// Sorts events and traces into a stable order.
    pub fn stable(mut self) -> Self {
        stable_event_order(&mut self.events);
        self.traces.sort_by(|left, right| {
            left.time
                .cmp(&right.time)
                .then_with(|| left.lane_id.cmp(&right.lane_id))
                .then_with(|| left.step.cmp(&right.step))
                .then_with(|| left.sound.cmp(&right.sound))
        });
        self
    }
}

pub(crate) struct DrumHit<'a> {
    pub player: &'static str,
    pub lane_id: &'a LaneId,
    pub trace_lane_id: &'a LaneId,
    pub time: Tick,
    pub duration: Tick,
    pub step: u64,
    pub sound: &'a str,
    pub key: u8,
    pub velocity: u8,
    pub channel: Channel,
}

pub(crate) fn push_drum_hit(render: &mut DrumPatternRender, hit: DrumHit<'_>) {
    render.events.push(PlayEvent::Note(NoteEvent {
        lane_id: hit.lane_id.clone(),
        time: hit.time,
        duration: hit.duration,
        pitch: Pitch::from_midi(hit.key),
        velocity: hit.velocity,
        channel: hit.channel,
    }));
    render.events.push(PlayEvent::Trace(TraceEvent {
        lane_id: hit.trace_lane_id.clone(),
        time: hit.time,
        step: hit.step,
    }));
    render.traces.push(DrumStepTrace {
        player: hit.player,
        lane_id: hit.lane_id.clone(),
        time: hit.time,
        step: hit.step,
        sound: hit.sound.to_owned(),
        key: hit.key,
        velocity: hit.velocity,
        active: true,
    });
}

pub(crate) fn tick(ticks: i64, tpq: u32) -> Tick {
    Tick {
        ticks,
        tpq: tpq.max(1),
    }
}

pub(crate) fn percent(value: u8) -> u8 {
    value.min(100)
}

pub(crate) fn stable_hash(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51afd7ed558ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ceb9fe1a85ec53);
    value ^ (value >> 33)
}
