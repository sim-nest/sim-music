use sim_lib_music_core::{
    Channel, LaneId, NoteEvent, Pitch, PlayEvent, Tick, TraceEvent, stable_event_order,
};

use crate::{percent, stable_hash, tick};

/// Direction a step lane advances through its cells.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PolyStepDirection {
    /// Advance from first cell to last and wrap.
    Forward,
    /// Advance from last cell to first and wrap.
    Reverse,
    /// Bounce back and forth between the endpoints.
    PingPong,
}

impl PolyStepDirection {
    /// Returns the stable wire label for the direction.
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Reverse => "reverse",
            Self::PingPong => "ping-pong",
        }
    }
}

/// A single step cell holding pitches and per-step playback parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolyStepCell {
    /// Pitches triggered at the step; empty means a rest.
    pub pitches: Vec<Pitch>,
    /// Note-on duration.
    pub gate: Tick,
    /// Velocity.
    pub velocity: u8,
    /// Probability the step fires, 0-100.
    pub probability: u8,
    /// Number of sub-hits (ratchets) within the step.
    pub ratchet: u8,
    /// Whether the step ties into the next.
    pub tie: bool,
    /// Whether the step slides into the next.
    pub slide: bool,
}

impl PolyStepCell {
    /// Creates a default empty (rest) cell.
    pub fn rest() -> Self {
        Self {
            pitches: Vec::new(),
            gate: tick(90, 480),
            velocity: 96,
            probability: 100,
            ratchet: 1,
            tie: false,
            slide: false,
        }
    }

    /// Creates a single-pitch cell with default parameters.
    pub fn note(pitch: Pitch) -> Self {
        Self {
            pitches: vec![pitch],
            ..Self::rest()
        }
    }

    /// Creates a multi-pitch (chord) cell with default parameters.
    pub fn chord(pitches: Vec<Pitch>) -> Self {
        Self {
            pitches,
            ..Self::rest()
        }
    }

    /// Sets the note-on duration.
    pub fn with_gate(mut self, gate: Tick) -> Self {
        self.gate = gate;
        self
    }

    /// Sets the velocity, clamped to 1-127.
    pub fn with_velocity(mut self, velocity: u8) -> Self {
        self.velocity = velocity.clamp(1, 127);
        self
    }

    /// Sets the fire probability, clamped to 0-100.
    pub fn with_probability(mut self, probability: u8) -> Self {
        self.probability = percent(probability);
        self
    }

    /// Sets the ratchet count, clamped to at least one.
    pub fn with_ratchet(mut self, ratchet: u8) -> Self {
        self.ratchet = ratchet.max(1);
        self
    }

    /// Sets whether the step ties into the next.
    pub fn with_tie(mut self, tie: bool) -> Self {
        self.tie = tie;
        self
    }

    /// Sets whether the step slides into the next.
    pub fn with_slide(mut self, slide: bool) -> Self {
        self.slide = slide;
        self
    }
}

/// A recorded note to write into a lane step (live-record input).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolyStepRecordInput {
    /// Target step index.
    pub step: u64,
    /// Pitch to record.
    pub pitch: Pitch,
    /// Velocity to record.
    pub velocity: u8,
    /// Note-on duration to record.
    pub gate: Tick,
}

impl PolyStepRecordInput {
    /// Creates a record input with a default gate.
    pub fn new(step: u64, pitch: Pitch, velocity: u8) -> Self {
        Self {
            step,
            pitch,
            velocity: velocity.clamp(1, 127),
            gate: tick(90, 480),
        }
    }

    /// Sets the note-on duration to record.
    pub fn with_gate(mut self, gate: Tick) -> Self {
        self.gate = gate;
        self
    }
}

/// One step lane: an ordered set of cells advanced by a direction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolyStepLane {
    /// Lane identifier.
    pub lane_id: LaneId,
    /// Number of cells in the lane.
    pub length: u16,
    /// Traversal direction.
    pub direction: PolyStepDirection,
    /// Output channel.
    pub channel: Channel,
    /// The lane's cells.
    pub steps: Vec<PolyStepCell>,
}

impl PolyStepLane {
    /// Creates a lane of the given length filled with rests.
    pub fn new(lane_id: impl Into<String>, length: u16) -> Self {
        let length = length.max(1);
        Self {
            lane_id: LaneId::new(lane_id),
            length,
            direction: PolyStepDirection::Forward,
            channel: Channel(0),
            steps: vec![PolyStepCell::rest(); usize::from(length)],
        }
    }

    /// Sets the traversal direction.
    pub fn with_direction(mut self, direction: PolyStepDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Sets the output channel.
    pub fn with_channel(mut self, channel: Channel) -> Self {
        self.channel = channel;
        self
    }

    /// Replaces the cell at the given index (modulo length).
    pub fn with_step(mut self, index: usize, cell: PolyStepCell) -> Self {
        self.set_step(index, cell);
        self
    }

    /// Records a note into the lane at the input's step.
    pub fn record_input(&mut self, input: PolyStepRecordInput) {
        let step = input.step as usize % usize::from(self.length);
        self.set_step(
            step,
            PolyStepCell::note(input.pitch)
                .with_gate(input.gate)
                .with_velocity(input.velocity),
        );
    }

    fn set_step(&mut self, index: usize, cell: PolyStepCell) {
        let length = usize::from(self.length);
        if self.steps.len() < length {
            self.steps.resize(length, PolyStepCell::rest());
        }
        self.steps[index % length] = cell;
    }

    fn cell(&self, lane_step: usize) -> &PolyStepCell {
        self.steps
            .get(lane_step % self.steps.len().max(1))
            .unwrap_or_else(|| self.steps.first().expect("poly step lane is non-empty"))
    }
}

/// Configuration for the [`PolyStepPlayer`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolyStepConfig {
    /// Step lanes rendered in parallel.
    pub lanes: Vec<PolyStepLane>,
    /// Time between steps.
    pub rate: Tick,
    /// Number of steps rendered.
    pub steps: u16,
    /// Seed for deterministic probability decisions.
    pub seed: u64,
    /// Lane carrying trace events.
    pub trace_lane_id: LaneId,
}

impl PolyStepConfig {
    /// Creates a config from a seed with no lanes.
    pub fn new(seed: u64) -> Self {
        Self {
            lanes: Vec::new(),
            rate: tick(120, 480),
            steps: 16,
            seed,
            trace_lane_id: LaneId::new("polystep-trace"),
        }
    }

    /// Appends a step lane.
    pub fn with_lane(mut self, lane: PolyStepLane) -> Self {
        self.lanes.push(lane);
        self
    }

    /// Sets the number of rendered steps.
    pub fn with_steps(mut self, steps: u16) -> Self {
        self.steps = steps;
        self
    }

    /// Sets the time between steps.
    pub fn with_rate(mut self, rate: Tick) -> Self {
        self.rate = rate;
        self
    }

    /// Records a note into the lane matching `lane_id`, if present.
    pub fn record_input(&mut self, lane_id: &LaneId, input: PolyStepRecordInput) {
        if let Some(lane) = self.lanes.iter_mut().find(|lane| &lane.lane_id == lane_id) {
            lane.record_input(input);
        }
    }
}

/// Diagnostic record describing one rendered lane step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolyStepTrace {
    /// Lane the step belongs to.
    pub lane_id: LaneId,
    /// Step time.
    pub time: Tick,
    /// Global step index.
    pub step: u64,
    /// Cell index within the lane visited at this step.
    pub lane_step: u16,
    /// Lane traversal direction.
    pub direction: PolyStepDirection,
    /// Whether the cell fired.
    pub emitted: bool,
    /// Number of pitches in the cell.
    pub pitch_count: usize,
    /// Cell fire probability.
    pub probability: u8,
    /// Cell ratchet count.
    pub ratchet: u8,
    /// Whether the cell ties.
    pub tie: bool,
    /// Whether the cell slides.
    pub slide: bool,
}

/// Rendered poly-step output: play events and per-step traces.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PolyStepRender {
    /// Ordered play events.
    pub events: Vec<PlayEvent>,
    /// Per-step diagnostic traces.
    pub traces: Vec<PolyStepTrace>,
}

impl PolyStepRender {
    /// Sorts events and traces into a stable order.
    pub fn stable(mut self) -> Self {
        stable_event_order(&mut self.events);
        self.traces.sort_by(|left, right| {
            left.time
                .cmp(&right.time)
                .then_with(|| left.lane_id.cmp(&right.lane_id))
                .then_with(|| left.step.cmp(&right.step))
                .then_with(|| left.lane_step.cmp(&right.lane_step))
        });
        self
    }
}

/// Polyphonic step sequencer rendering several lanes in parallel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolyStepPlayer {
    /// Configuration driving the render.
    pub config: PolyStepConfig,
}

impl PolyStepPlayer {
    /// Creates a player from its config.
    pub fn new(config: PolyStepConfig) -> Self {
        Self { config }
    }

    /// Renders all lanes across every step.
    pub fn render(&self) -> PolyStepRender {
        let mut render = PolyStepRender::default();
        for step in 0..u64::from(self.config.steps) {
            for (lane_index, lane) in self.config.lanes.iter().enumerate() {
                self.render_lane_step(&mut render, step, lane_index as u64, lane);
            }
        }
        render.stable()
    }

    /// Renders all lanes; alias for [`PolyStepPlayer::render`].
    pub fn freeze(&self) -> PolyStepRender {
        self.render()
    }

    fn render_lane_step(
        &self,
        render: &mut PolyStepRender,
        step: u64,
        lane_index: u64,
        lane: &PolyStepLane,
    ) {
        let lane_step = lane_step(step, lane.length, lane.direction);
        let cell = lane.cell(usize::from(lane_step));
        let time = self.step_time(step);
        let emitted = self.should_emit(cell, step, lane_index, lane_step);
        render.events.push(PlayEvent::Trace(TraceEvent {
            lane_id: self.config.trace_lane_id.clone(),
            time,
            step,
        }));
        render.traces.push(PolyStepTrace {
            lane_id: lane.lane_id.clone(),
            time,
            step,
            lane_step,
            direction: lane.direction,
            emitted,
            pitch_count: cell.pitches.len(),
            probability: cell.probability,
            ratchet: cell.ratchet,
            tie: cell.tie,
            slide: cell.slide,
        });
        if !emitted {
            return;
        }
        let ratchet = u64::from(cell.ratchet.max(1));
        for ratchet_index in 0..ratchet {
            let time = self.ratchet_time(step, ratchet_index, ratchet);
            for pitch in &cell.pitches {
                render.events.push(PlayEvent::Note(NoteEvent {
                    lane_id: lane.lane_id.clone(),
                    time,
                    duration: self.duration(cell),
                    pitch: *pitch,
                    velocity: cell.velocity,
                    channel: lane.channel,
                }));
            }
        }
    }

    fn should_emit(&self, cell: &PolyStepCell, step: u64, lane_index: u64, lane_step: u16) -> bool {
        if cell.pitches.is_empty() || cell.probability == 0 {
            return false;
        }
        if cell.probability >= 100 {
            return true;
        }
        let score = stable_hash(
            self.config.seed ^ (step << 17) ^ (lane_index << 31) ^ (u64::from(lane_step) << 43),
        );
        score % 100 < u64::from(cell.probability)
    }

    fn duration(&self, cell: &PolyStepCell) -> Tick {
        if cell.tie || cell.slide {
            Tick {
                ticks: cell.gate.ticks.max(self.config.rate.ticks),
                tpq: self.config.rate.tpq,
            }
        } else {
            cell.gate
        }
    }

    fn step_time(&self, step: u64) -> Tick {
        Tick {
            ticks: self.config.rate.ticks * step as i64,
            tpq: self.config.rate.tpq,
        }
    }

    fn ratchet_time(&self, step: u64, ratchet_index: u64, ratchets: u64) -> Tick {
        let offset = (self.config.rate.ticks * ratchet_index as i64) / ratchets as i64;
        Tick {
            ticks: self.step_time(step).ticks + offset,
            tpq: self.config.rate.tpq,
        }
    }
}

fn lane_step(step: u64, length: u16, direction: PolyStepDirection) -> u16 {
    let length = length.max(1);
    match direction {
        PolyStepDirection::Forward => (step % u64::from(length)) as u16,
        PolyStepDirection::Reverse => length - 1 - (step % u64::from(length)) as u16,
        PolyStepDirection::PingPong => ping_pong_step(step, length),
    }
}

fn ping_pong_step(step: u64, length: u16) -> u16 {
    if length <= 1 {
        return 0;
    }
    let period = u64::from(length) * 2 - 2;
    let position = step % period;
    if position < u64::from(length) {
        position as u16
    } else {
        (period - position) as u16
    }
}
