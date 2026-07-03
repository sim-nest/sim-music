use sim_lib_music_core::{
    Channel, LaneId, NoteEvent, Pitch, PitchClass, PlayEvent, Tick, TraceEvent, stable_event_order,
};
use sim_lib_pitch_scale::Scale;

use crate::{percent, stable_hash, tick};

/// Inclusive octave span the bassline may choose pitches from.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BasslineOctaveRange {
    /// Lowest octave.
    pub low: i16,
    /// Highest octave.
    pub high: i16,
}

impl BasslineOctaveRange {
    /// Creates a range, normalizing low/high order.
    pub fn new(low: i16, high: i16) -> Self {
        Self {
            low: low.min(high),
            high: low.max(high),
        }
    }

    fn choose(self, score: u64) -> i16 {
        let span = (self.high - self.low + 1).max(1) as u64;
        self.low + (score % span) as i16
    }
}

/// A chord root that applies over a half-open span of steps.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BasslineChordSpan {
    /// Root pitch class the bassline follows over the span.
    pub root: PitchClass,
    /// First step the span covers.
    pub start_step: u64,
    /// One past the last step the span covers.
    pub end_step: u64,
}

impl BasslineChordSpan {
    /// Creates a span, ensuring it covers at least one step.
    pub fn new(root: PitchClass, start_step: u64, end_step: u64) -> Self {
        Self {
            root,
            start_step,
            end_step: end_step.max(start_step + 1),
        }
    }

    fn contains(self, step: u64) -> bool {
        step >= self.start_step && step < self.end_step
    }
}

/// Where a step's root pitch came from.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BasslineRootSource {
    /// Root supplied by a chord-follow span.
    ChordFollow,
    /// Root taken from the configured held root.
    HeldRoot,
}

impl BasslineRootSource {
    /// Returns the stable wire label for the source.
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::ChordFollow => "chord-follow",
            Self::HeldRoot => "held-root",
        }
    }
}

/// Configuration for the generative [`BasslinePlayer`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasslineConfig {
    /// Scale the bassline pitches are drawn from.
    pub scale: Scale,
    /// Root used when no chord-follow span applies.
    pub held_root: Pitch,
    /// Probability density of note hits, 0-100.
    pub density: u8,
    /// Octave span pitches may fall in.
    pub octave_range: BasslineOctaveRange,
    /// Default note length.
    pub note_length: Tick,
    /// Accent every Nth step; zero disables accents.
    pub accent_every: u16,
    /// Velocity boost applied on accented steps.
    pub accent: u8,
    /// Probability of a slide (legato) note, 0-100.
    pub slide: u8,
    /// Probability of a ghost (quieter) note, 0-100.
    pub ghost_notes: u8,
    /// Seed for deterministic generation.
    pub seed: u64,
    /// Time between steps.
    pub rate: Tick,
    /// Number of steps rendered.
    pub steps: u16,
    /// Base velocity.
    pub velocity: u8,
    /// Output channel.
    pub channel: Channel,
    /// Lane carrying note events.
    pub lane_id: LaneId,
    /// Lane carrying trace events.
    pub trace_lane_id: LaneId,
}

impl BasslineConfig {
    /// Creates a config from a scale, held root, and seed with defaults.
    pub fn new(scale: Scale, held_root: Pitch, seed: u64) -> Self {
        Self {
            scale,
            held_root,
            density: 65,
            octave_range: BasslineOctaveRange::new(held_root.octave, held_root.octave + 1),
            note_length: tick(90, 480),
            accent_every: 4,
            accent: 24,
            slide: 0,
            ghost_notes: 0,
            seed,
            rate: tick(120, 480),
            steps: 16,
            velocity: 82,
            channel: Channel(0),
            lane_id: LaneId::new("bassline"),
            trace_lane_id: LaneId::new("bassline-trace"),
        }
    }

    /// Sets the note density, clamped to 0-100.
    pub fn with_density(mut self, density: u8) -> Self {
        self.density = percent(density);
        self
    }

    /// Sets the octave range.
    pub fn with_octave_range(mut self, range: BasslineOctaveRange) -> Self {
        self.octave_range = range;
        self
    }

    /// Sets the number of rendered steps.
    pub fn with_steps(mut self, steps: u16) -> Self {
        self.steps = steps;
        self
    }

    /// Sets the default note length.
    pub fn with_note_length(mut self, note_length: Tick) -> Self {
        self.note_length = note_length;
        self
    }
}

/// Diagnostic record describing one rendered bassline step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasslineStepTrace {
    /// Lane the step belongs to.
    pub lane_id: LaneId,
    /// Step time.
    pub time: Tick,
    /// Step index.
    pub step: u64,
    /// Where the root pitch came from.
    pub source: BasslineRootSource,
    /// Root pitch class used.
    pub root: PitchClass,
    /// Pitch played.
    pub pitch: Pitch,
    /// Velocity played.
    pub velocity: u8,
    /// Whether the step was accented.
    pub accent: bool,
    /// Whether the step slides into the next.
    pub slide: bool,
    /// Whether the step is a ghost note.
    pub ghost: bool,
}

/// Rendered bassline output: play events and per-step traces.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BasslineRender {
    /// Ordered play events.
    pub events: Vec<PlayEvent>,
    /// Per-step diagnostic traces.
    pub traces: Vec<BasslineStepTrace>,
}

impl BasslineRender {
    /// Sorts events and traces into a stable order.
    pub fn stable(mut self) -> Self {
        stable_event_order(&mut self.events);
        self.traces.sort_by(|left, right| {
            left.time
                .cmp(&right.time)
                .then_with(|| left.lane_id.cmp(&right.lane_id))
                .then_with(|| left.step.cmp(&right.step))
                .then_with(|| left.pitch.cmp(&right.pitch))
        });
        self
    }
}

/// Generative bassline player driven by a [`BasslineConfig`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasslinePlayer {
    /// Configuration driving the render.
    pub config: BasslineConfig,
}

impl BasslinePlayer {
    /// Creates a player from its config.
    pub fn new(config: BasslineConfig) -> Self {
        Self { config }
    }

    /// Renders the bassline, following chord-root spans where they apply.
    pub fn render(&self, chord_follow: &[BasslineChordSpan]) -> BasslineRender {
        let mut render = BasslineRender::default();
        for step in 0..u64::from(self.config.steps) {
            let score = self.score(step, 0);
            if !self.hit(step, score) {
                continue;
            }
            self.render_step(&mut render, chord_follow, step, score);
        }
        render.stable()
    }

    /// Renders the bassline; alias for [`BasslinePlayer::render`].
    pub fn freeze(&self, chord_follow: &[BasslineChordSpan]) -> BasslineRender {
        self.render(chord_follow)
    }

    fn render_step(
        &self,
        render: &mut BasslineRender,
        chord_follow: &[BasslineChordSpan],
        step: u64,
        score: u64,
    ) {
        let (root, source) = self.root_for_step(chord_follow, step);
        let accent = self.accent(step);
        let ghost = !accent && self.percent_hit(step, 1, self.config.ghost_notes);
        let slide = self.percent_hit(step, 2, self.config.slide);
        let pitch = self.pitch_for_step(root, step, score);
        let velocity = self.velocity(accent, ghost, step);
        let time = self.step_time(step);
        let duration = self.duration(slide);
        render.events.push(PlayEvent::Note(NoteEvent {
            lane_id: self.config.lane_id.clone(),
            time,
            duration,
            pitch,
            velocity,
            channel: self.config.channel,
        }));
        render.events.push(PlayEvent::Trace(TraceEvent {
            lane_id: self.config.trace_lane_id.clone(),
            time,
            step,
        }));
        render.traces.push(BasslineStepTrace {
            lane_id: self.config.lane_id.clone(),
            time,
            step,
            source,
            root,
            pitch,
            velocity,
            accent,
            slide,
            ghost,
        });
    }

    fn root_for_step(
        &self,
        chord_follow: &[BasslineChordSpan],
        step: u64,
    ) -> (PitchClass, BasslineRootSource) {
        chord_follow
            .iter()
            .find(|span| span.contains(step))
            .map(|span| (span.root, BasslineRootSource::ChordFollow))
            .unwrap_or((self.config.held_root.class, BasslineRootSource::HeldRoot))
    }

    fn hit(&self, step: u64, score: u64) -> bool {
        step == 0 || score % 100 < u64::from(percent(self.config.density))
    }

    fn accent(&self, step: u64) -> bool {
        self.config.accent_every > 0 && step.is_multiple_of(u64::from(self.config.accent_every))
    }

    fn pitch_for_step(&self, root: PitchClass, step: u64, score: u64) -> Pitch {
        let root_degree = self
            .config
            .scale
            .degree_of(root)
            .or_else(|| self.config.scale.degree_of(self.config.held_root.class))
            .unwrap_or(1);
        let offset = melodic_offset(step, score);
        Pitch {
            class: self.config.scale.pitch_at_degree(root_degree + offset),
            octave: self.config.octave_range.choose(score >> 8),
        }
    }

    fn velocity(&self, accent: bool, ghost: bool, step: u64) -> u8 {
        let jitter = (self.score(step, 3) % 9) as u8;
        let mut velocity = self.config.velocity.saturating_add(jitter);
        if accent {
            velocity = velocity.saturating_add(self.config.accent);
        }
        if ghost {
            velocity = (u16::from(velocity) * 55 / 100) as u8;
        }
        velocity.clamp(1, 127)
    }

    fn duration(&self, slide: bool) -> Tick {
        if slide {
            Tick {
                ticks: self.config.note_length.ticks.max(self.config.rate.ticks),
                tpq: self.config.rate.tpq,
            }
        } else {
            self.config.note_length
        }
    }

    fn step_time(&self, step: u64) -> Tick {
        Tick {
            ticks: self.config.rate.ticks * step as i64,
            tpq: self.config.rate.tpq,
        }
    }

    fn percent_hit(&self, step: u64, lane: u64, threshold: u8) -> bool {
        threshold > 0 && self.score(step, lane) % 100 < u64::from(percent(threshold))
    }

    fn score(&self, step: u64, lane: u64) -> u64 {
        stable_hash(self.config.seed ^ (step << 17) ^ (lane << 41))
    }
}

fn melodic_offset(step: u64, score: u64) -> usize {
    if step.is_multiple_of(4) {
        return 0;
    }
    const OFFSETS: [usize; 8] = [0, 2, 4, 1, 5, 3, 6, 4];
    OFFSETS[(score as usize) % OFFSETS.len()]
}
