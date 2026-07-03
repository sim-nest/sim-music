use sim_kernel::Symbol;
use sim_lib_music_core::{
    Channel, ControlEvent, LaneId, NoteEvent, Pitch, PlayEvent, Tick, TraceEvent,
    stable_event_order,
};

/// Direction in which an arpeggiator traverses its note sequence.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArpDirection {
    /// Ascend from the first note to the last.
    Up,
    /// Descend from the last note to the first.
    Down,
    /// Ascend then descend without repeating the endpoints.
    UpDown,
    /// Preserve the order in which notes were supplied.
    AsPlayed,
}

/// Ordering applied to input notes before arpeggiation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NoteOrderPolicy {
    /// Keep the supplied input order.
    Input,
    /// Sort notes by ascending pitch.
    PitchAscending,
    /// Sort notes by descending pitch.
    PitchDescending,
}

/// Per-step behavior within an arpeggiator pattern.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArpStepKind {
    /// Trigger the next note in the sequence.
    Play,
    /// Hold the previous note without re-triggering.
    Tie,
    /// Emit silence for this step.
    Rest,
}

/// Identifier for one of the two engines in a dual arpeggiator.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArpEngineSlot {
    /// The first engine.
    A,
    /// The second engine.
    B,
}

/// Origin of an arpeggiator trace event.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArpTraceSource {
    /// Produced by the named engine slot.
    Engine(ArpEngineSlot),
    /// Produced by a note routed past both engines unchanged.
    PassThrough,
    /// Produced by the [`crate::ArpLab`] anchor/movement renderer.
    Lab,
}

/// Action recorded by an arpeggiator trace step.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArpTraceAction {
    /// A note was triggered.
    Played,
    /// A note was held over from the previous step.
    Tied,
    /// The step was silent.
    Rested,
    /// A note bypassed the engines unchanged.
    PassedThrough,
    /// An anchor note was held for the whole render.
    HeldAnchor,
}

/// A single note fed into an arpeggiator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArpInputNote {
    /// Pitch of the note.
    pub pitch: Pitch,
    /// MIDI velocity, 1-127.
    pub velocity: u8,
    /// Output channel.
    pub channel: Channel,
}

impl ArpInputNote {
    /// Creates an input note from pitch, velocity, and channel.
    pub fn new(pitch: Pitch, velocity: u8, channel: Channel) -> Self {
        Self {
            pitch,
            velocity,
            channel,
        }
    }

    /// Creates an input note from a raw MIDI note number.
    pub fn from_midi(midi: u8, velocity: u8, channel: Channel) -> Self {
        Self::new(Pitch::from_midi(midi), velocity, channel)
    }
}

/// Gate and mask state emitted at one arpeggiator step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateMaskFrame {
    /// Lane the frame belongs to.
    pub lane_id: LaneId,
    /// Step time.
    pub time: Tick,
    /// Configured gate length for the step.
    pub gate_ticks: Tick,
    /// Whether the gate (note on) is open.
    pub gate_open: bool,
    /// Whether the mask (note passes through) is open.
    pub mask_open: bool,
}

impl GateMaskFrame {
    /// Renders this frame as paired gate and mask control events.
    pub fn to_control_events(&self) -> [PlayEvent; 2] {
        [
            PlayEvent::Control(ControlEvent {
                lane_id: self.lane_id.clone(),
                time: self.time,
                control: Symbol::qualified("music/control", "gate"),
                value: if self.gate_open { 1 } else { 0 },
            }),
            PlayEvent::Control(ControlEvent {
                lane_id: self.lane_id.clone(),
                time: self.time,
                control: Symbol::qualified("music/control", "mask"),
                value: if self.mask_open { 1 } else { 0 },
            }),
        ]
    }
}

/// Diagnostic record describing one arpeggiator step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArpStepTrace {
    /// Origin of the trace.
    pub source: ArpTraceSource,
    /// Lane the trace belongs to.
    pub lane_id: LaneId,
    /// Step time.
    pub time: Tick,
    /// Step index within the render.
    pub step: u64,
    /// Action taken at the step.
    pub action: ArpTraceAction,
    /// Pitch involved, if any.
    pub pitch: Option<Pitch>,
    /// Whether the gate was open.
    pub gate_open: bool,
    /// Whether the mask was open.
    pub mask_open: bool,
}

/// Rendered arpeggiator output: play events, gate/mask frames, and traces.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArpRender {
    /// Ordered play events (notes, controls, traces).
    pub events: Vec<PlayEvent>,
    /// Gate and mask frames per step.
    pub gate_masks: Vec<GateMaskFrame>,
    /// Per-step diagnostic traces.
    pub traces: Vec<ArpStepTrace>,
}

impl ArpRender {
    /// Sorts events, gate/mask frames, and traces into a stable order.
    pub fn stable(mut self) -> Self {
        stable_event_order(&mut self.events);
        self.gate_masks.sort_by(|left, right| {
            left.time
                .cmp(&right.time)
                .then_with(|| left.lane_id.cmp(&right.lane_id))
        });
        self.traces.sort_by(|left, right| {
            left.time
                .cmp(&right.time)
                .then_with(|| left.lane_id.cmp(&right.lane_id))
                .then_with(|| left.step.cmp(&right.step))
        });
        self
    }

    /// Collects the pitches of every note event in render order.
    pub fn note_pitches(&self) -> Vec<Pitch> {
        self.events
            .iter()
            .filter_map(|event| match event {
                PlayEvent::Note(note) => Some(note.pitch),
                _ => None,
            })
            .collect()
    }
}

/// Configuration for a single arpeggiator engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArpEngineConfig {
    /// Lane that carries the engine's note events.
    pub lane_id: LaneId,
    /// Lane that carries the engine's trace events.
    pub trace_lane_id: LaneId,
    /// Lane that carries the engine's gate/mask control events.
    pub gate_mask_lane_id: LaneId,
    /// Traversal direction.
    pub direction: ArpDirection,
    /// Number of octaves the sequence spans.
    pub octave_range: u8,
    /// Time between steps.
    pub rate: Tick,
    /// Note-on duration per step.
    pub gate: Tick,
    /// Per-step pattern of play/tie/rest actions.
    pub steps: Vec<ArpStepKind>,
    /// Length of the repeating pattern in steps.
    pub pattern_len: usize,
    /// Ordering applied to input notes.
    pub note_order: NoteOrderPolicy,
}

impl ArpEngineConfig {
    /// Creates an engine with default direction, octave range, and pattern.
    pub fn new(lane_id: impl Into<String>, rate: Tick, gate: Tick) -> Self {
        let lane_id = LaneId::new(lane_id);
        Self {
            trace_lane_id: LaneId::new(format!("{}-trace", lane_id.0)),
            gate_mask_lane_id: LaneId::new(format!("{}-gate-mask", lane_id.0)),
            lane_id,
            direction: ArpDirection::Up,
            octave_range: 1,
            rate,
            gate,
            steps: vec![ArpStepKind::Play],
            pattern_len: 1,
            note_order: NoteOrderPolicy::PitchAscending,
        }
    }

    /// Sets the traversal direction.
    pub fn with_direction(mut self, direction: ArpDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Sets the octave range, clamped to at least one.
    pub fn with_octave_range(mut self, octave_range: u8) -> Self {
        self.octave_range = octave_range.max(1);
        self
    }

    /// Sets the step pattern and its repeat length.
    pub fn with_pattern(mut self, steps: Vec<ArpStepKind>, pattern_len: usize) -> Self {
        self.steps = if steps.is_empty() {
            vec![ArpStepKind::Play]
        } else {
            steps
        };
        self.pattern_len = pattern_len.max(1);
        self
    }

    /// Sets the input-note ordering policy.
    pub fn with_note_order(mut self, note_order: NoteOrderPolicy) -> Self {
        self.note_order = note_order;
        self
    }

    pub(crate) fn step_kind(&self, step: usize) -> ArpStepKind {
        let pattern_len = self.pattern_len.max(1);
        let slot = step % pattern_len;
        self.steps
            .get(slot % self.steps.len().max(1))
            .copied()
            .unwrap_or(ArpStepKind::Play)
    }
}

/// Optional low/high pitch bounds used to route notes in a key split.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyRange {
    /// Inclusive lower bound, or `None` for unbounded below.
    pub low: Option<Pitch>,
    /// Inclusive upper bound, or `None` for unbounded above.
    pub high: Option<Pitch>,
}

impl KeyRange {
    /// Returns an unbounded range that accepts every pitch.
    pub fn all() -> Self {
        Self {
            low: None,
            high: None,
        }
    }

    /// Creates a range from optional low and high bounds.
    pub fn new(low: Option<Pitch>, high: Option<Pitch>) -> Self {
        Self { low, high }
    }

    /// Reports whether a pitch falls within the bounds.
    pub fn contains(&self, pitch: Pitch) -> bool {
        self.low
            .is_none_or(|low| pitch.semitone() >= low.semitone())
            && self
                .high
                .is_none_or(|high| pitch.semitone() <= high.semitone())
    }
}

/// Routing rule that splits the keyboard between two engines at a pitch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeySplitConfig {
    /// Pitch at which the keyboard is divided.
    pub split_pitch: Pitch,
    /// Engine assigned to notes at or below the split.
    pub lower_engine: ArpEngineSlot,
    /// Engine assigned to notes above the split.
    pub upper_engine: ArpEngineSlot,
    /// Range gating notes routed to the lower engine.
    pub lower_range: KeyRange,
    /// Range gating notes routed to the upper engine.
    pub upper_range: KeyRange,
    /// Whether notes outside both ranges pass through unchanged.
    pub pass_through_outside_ranges: bool,
}

impl KeySplitConfig {
    /// Creates a split at the given pitch with default engine assignments.
    pub fn new(split_pitch: Pitch) -> Self {
        Self {
            split_pitch,
            lower_engine: ArpEngineSlot::A,
            upper_engine: ArpEngineSlot::B,
            lower_range: KeyRange::new(None, Some(split_pitch)),
            upper_range: KeyRange::new(Some(split_pitch.transpose(1)), None),
            pass_through_outside_ranges: true,
        }
    }

    fn route(&self, note: &ArpInputNote) -> Option<ArpEngineSlot> {
        if note.pitch.semitone() <= self.split_pitch.semitone()
            && self.lower_range.contains(note.pitch)
        {
            Some(self.lower_engine)
        } else if note.pitch.semitone() > self.split_pitch.semitone()
            && self.upper_range.contains(note.pitch)
        {
            Some(self.upper_engine)
        } else {
            None
        }
    }
}

/// How a [`DualArpeggiator`] combines its two engines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DualArpMode {
    /// Both engines arpeggiate the same input in parallel.
    Parallel,
    /// Input is split between engines by the given key-split rule.
    KeySplit(KeySplitConfig),
}

/// Two arpeggiator engines combined under a [`DualArpMode`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DualArpeggiator {
    /// First engine.
    pub engine_a: ArpEngineConfig,
    /// Second engine.
    pub engine_b: ArpEngineConfig,
    /// Combination mode.
    pub mode: DualArpMode,
    /// Lane used for pass-through notes in key-split mode.
    pub pass_through_lane_id: LaneId,
}

impl DualArpeggiator {
    /// Creates a dual arpeggiator from two engines and a mode.
    pub fn new(engine_a: ArpEngineConfig, engine_b: ArpEngineConfig, mode: DualArpMode) -> Self {
        Self {
            engine_a,
            engine_b,
            mode,
            pass_through_lane_id: LaneId::new("arp-pass-through"),
        }
    }

    /// Renders `step_count` steps starting at tick zero.
    pub fn render(&self, notes: &[ArpInputNote], step_count: usize) -> ArpRender {
        self.render_from(zero_tick(self.engine_a.rate.tpq), notes, step_count)
    }

    /// Renders the arpeggiation; alias for [`DualArpeggiator::render`].
    pub fn freeze(&self, notes: &[ArpInputNote], step_count: usize) -> ArpRender {
        self.render(notes, step_count)
    }

    /// Renders `step_count` steps starting at the given tick.
    pub fn render_from(&self, start: Tick, notes: &[ArpInputNote], step_count: usize) -> ArpRender {
        let mut render = ArpRender::default();
        match &self.mode {
            DualArpMode::Parallel => {
                render.extend(render_engine(
                    ArpTraceSource::Engine(ArpEngineSlot::A),
                    &self.engine_a,
                    notes,
                    start,
                    step_count,
                ));
                render.extend(render_engine(
                    ArpTraceSource::Engine(ArpEngineSlot::B),
                    &self.engine_b,
                    notes,
                    start,
                    step_count,
                ));
            }
            DualArpMode::KeySplit(split) => {
                let mut a_notes = Vec::new();
                let mut b_notes = Vec::new();
                for note in notes {
                    match split.route(note) {
                        Some(ArpEngineSlot::A) => a_notes.push(note.clone()),
                        Some(ArpEngineSlot::B) => b_notes.push(note.clone()),
                        None if split.pass_through_outside_ranges => render_pass_through(
                            &mut render,
                            &self.pass_through_lane_id,
                            note,
                            start,
                        ),
                        None => {}
                    }
                }
                render.extend(render_engine(
                    ArpTraceSource::Engine(ArpEngineSlot::A),
                    &self.engine_a,
                    &a_notes,
                    start,
                    step_count,
                ));
                render.extend(render_engine(
                    ArpTraceSource::Engine(ArpEngineSlot::B),
                    &self.engine_b,
                    &b_notes,
                    start,
                    step_count,
                ));
            }
        }
        render.stable()
    }
}

impl ArpRender {
    /// Appends another render's events, gate/mask frames, and traces.
    pub fn extend(&mut self, other: ArpRender) {
        self.events.extend(other.events);
        self.gate_masks.extend(other.gate_masks);
        self.traces.extend(other.traces);
    }
}

pub(crate) fn render_engine(
    source: ArpTraceSource,
    config: &ArpEngineConfig,
    notes: &[ArpInputNote],
    start: Tick,
    step_count: usize,
) -> ArpRender {
    let sequence = ordered_sequence(config, notes);
    if sequence.is_empty() {
        return ArpRender::default();
    }

    let mut render = ArpRender::default();
    let mut note_cursor = 0usize;
    let mut last_pitch = None;
    for step in 0..step_count {
        let time = start + config.rate.mul_int(step as i64);
        match config.step_kind(step) {
            ArpStepKind::Play => {
                let selected = &sequence[note_cursor % sequence.len()];
                note_cursor += 1;
                last_pitch = Some(selected.pitch);
                render.events.push(PlayEvent::Note(NoteEvent {
                    lane_id: config.lane_id.clone(),
                    time,
                    duration: config.gate,
                    pitch: selected.pitch,
                    velocity: selected.velocity,
                    channel: selected.channel,
                }));
                push_gate_mask(&mut render, config, time, true, true);
                push_trace(
                    &mut render,
                    ArpStepTrace {
                        source,
                        lane_id: config.trace_lane_id.clone(),
                        time,
                        step: step as u64,
                        action: ArpTraceAction::Played,
                        pitch: Some(selected.pitch),
                        gate_open: true,
                        mask_open: true,
                    },
                );
            }
            ArpStepKind::Tie => {
                push_gate_mask(&mut render, config, time, true, true);
                push_trace(
                    &mut render,
                    ArpStepTrace {
                        source,
                        lane_id: config.trace_lane_id.clone(),
                        time,
                        step: step as u64,
                        action: ArpTraceAction::Tied,
                        pitch: last_pitch,
                        gate_open: true,
                        mask_open: true,
                    },
                );
            }
            ArpStepKind::Rest => {
                push_gate_mask(&mut render, config, time, false, false);
                push_trace(
                    &mut render,
                    ArpStepTrace {
                        source,
                        lane_id: config.trace_lane_id.clone(),
                        time,
                        step: step as u64,
                        action: ArpTraceAction::Rested,
                        pitch: None,
                        gate_open: false,
                        mask_open: false,
                    },
                );
            }
        }
    }
    render.stable()
}

fn ordered_sequence(config: &ArpEngineConfig, notes: &[ArpInputNote]) -> Vec<ArpInputNote> {
    let mut base = notes.to_vec();
    match config.note_order {
        NoteOrderPolicy::Input => {}
        NoteOrderPolicy::PitchAscending => base.sort_by_key(|note| note.pitch.semitone()),
        NoteOrderPolicy::PitchDescending => {
            base.sort_by_key(|note| std::cmp::Reverse(note.pitch.semitone()));
        }
    }

    let mut expanded = Vec::new();
    for octave in 0..config.octave_range.max(1) {
        let transpose = i32::from(octave) * 12;
        expanded.extend(base.iter().cloned().map(|mut note| {
            note.pitch = note.pitch.transpose(transpose);
            note
        }));
    }

    match config.direction {
        ArpDirection::Up | ArpDirection::AsPlayed => expanded,
        ArpDirection::Down => {
            expanded.reverse();
            expanded
        }
        ArpDirection::UpDown => {
            if expanded.len() <= 2 {
                return expanded;
            }
            let mut sequence = expanded.clone();
            sequence.extend(expanded[1..expanded.len() - 1].iter().rev().cloned());
            sequence
        }
    }
}

fn render_pass_through(render: &mut ArpRender, lane_id: &LaneId, note: &ArpInputNote, start: Tick) {
    render.events.push(PlayEvent::Note(NoteEvent {
        lane_id: lane_id.clone(),
        time: start,
        duration: Tick {
            ticks: start.tpq as i64,
            tpq: start.tpq,
        },
        pitch: note.pitch,
        velocity: note.velocity,
        channel: note.channel,
    }));
    push_trace(
        render,
        ArpStepTrace {
            source: ArpTraceSource::PassThrough,
            lane_id: lane_id.clone(),
            time: start,
            step: 0,
            action: ArpTraceAction::PassedThrough,
            pitch: Some(note.pitch),
            gate_open: true,
            mask_open: true,
        },
    );
}

fn push_gate_mask(
    render: &mut ArpRender,
    config: &ArpEngineConfig,
    time: Tick,
    gate_open: bool,
    mask_open: bool,
) {
    let frame = GateMaskFrame {
        lane_id: config.gate_mask_lane_id.clone(),
        time,
        gate_ticks: config.gate,
        gate_open,
        mask_open,
    };
    render.events.extend(frame.to_control_events());
    render.gate_masks.push(frame);
}

pub(crate) fn push_trace(render: &mut ArpRender, trace: ArpStepTrace) {
    render.events.push(PlayEvent::Trace(TraceEvent {
        lane_id: trace.lane_id.clone(),
        time: trace.time,
        step: trace.step,
    }));
    render.traces.push(trace);
}

fn zero_tick(tpq: u32) -> Tick {
    Tick {
        ticks: 0,
        tpq: tpq.max(1),
    }
}
