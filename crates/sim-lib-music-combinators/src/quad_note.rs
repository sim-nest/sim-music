use sim_lib_music_core::{
    Channel, LaneId, NoteEvent, Pitch, PlayEvent, Tick, TraceEvent, stable_event_order,
};
use sim_lib_pitch_scale::Scale;

use crate::{percent, stable_hash, tick};

/// Maximum number of streams a quad-note player renders.
pub const QUAD_NOTE_MAX_STREAMS: usize = 4;

/// Rhythmic feel applied to a stream's step hits.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum QuadNoteRhythm {
    /// Uniform hit probability across steps.
    Even,
    /// Favor downbeats, thin out off-beats.
    Sparse,
    /// Favor off-beat steps.
    Syncopated,
}

impl QuadNoteRhythm {
    /// Returns the stable wire label for the rhythm.
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::Even => "even",
            Self::Sparse => "sparse",
            Self::Syncopated => "syncopated",
        }
    }

    fn threshold(self, step: u64, density: u8) -> u8 {
        let density = percent(density);
        match self {
            Self::Even => density,
            Self::Sparse if step.is_multiple_of(4) => density,
            Self::Sparse => density / 3,
            Self::Syncopated if step % 4 == 1 || step % 4 == 3 => density,
            Self::Syncopated => density / 2,
        }
    }
}

/// Harmonic spacing between streams expressed as scale-degree offsets.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum QuadNoteHarmonicRelation {
    /// All streams share the same degree.
    Unison,
    /// Streams stack in thirds.
    Thirds,
    /// Streams stack in fifths.
    Fifths,
    /// Streams spread across a fixed degree set.
    Spread,
}

impl QuadNoteHarmonicRelation {
    /// Returns the stable wire label for the relation.
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::Unison => "unison",
            Self::Thirds => "thirds",
            Self::Fifths => "fifths",
            Self::Spread => "spread",
        }
    }

    fn degree_offset(self, stream_index: usize) -> usize {
        match self {
            Self::Unison => 0,
            Self::Thirds => stream_index * 2,
            Self::Fifths => stream_index * 4,
            Self::Spread => [0, 2, 4, 6][stream_index.min(QUAD_NOTE_MAX_STREAMS - 1)],
        }
    }
}

/// Inclusive pitch window a stream draws notes from.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct QuadNotePitchRange {
    /// Lowest pitch.
    pub low: Pitch,
    /// Highest pitch.
    pub high: Pitch,
}

impl QuadNotePitchRange {
    /// Creates a range, normalizing low/high order.
    pub fn new(low: Pitch, high: Pitch) -> Self {
        if low.semitone() <= high.semitone() {
            Self { low, high }
        } else {
            Self {
                low: high,
                high: low,
            }
        }
    }

    fn choose_scale(self, scale: Scale, degree: usize, score: u64) -> Pitch {
        let target_class = scale.pitch_at_degree(degree);
        let candidates = self.scale_candidates(scale);
        let matching = candidates
            .iter()
            .copied()
            .filter(|pitch| pitch.class == target_class)
            .collect::<Vec<_>>();
        choose_pitch(
            if matching.is_empty() {
                &candidates
            } else {
                &matching
            },
            self.low,
            score,
        )
    }

    fn choose_chromatic(self, score: u64) -> Pitch {
        let low = self.low.semitone();
        let high = self.high.semitone();
        let span = (high - low + 1).max(1) as u64;
        Pitch::from_semitone(low + (score % span) as i32)
    }

    fn scale_candidates(self, scale: Scale) -> Vec<Pitch> {
        (self.low.semitone()..=self.high.semitone())
            .map(Pitch::from_semitone)
            .filter(|pitch| scale.degree_of(pitch.class).is_some())
            .collect()
    }
}

/// Inclusive velocity window a stream chooses from.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct QuadNoteVelocityRange {
    /// Lowest velocity.
    pub low: u8,
    /// Highest velocity.
    pub high: u8,
}

impl QuadNoteVelocityRange {
    /// Creates a range, clamping to 1-127 and normalizing order.
    pub fn new(low: u8, high: u8) -> Self {
        let low = low.clamp(1, 127);
        let high = high.clamp(1, 127);
        Self {
            low: low.min(high),
            high: low.max(high),
        }
    }

    fn choose(self, score: u64) -> u8 {
        let span = u16::from(self.high - self.low) + 1;
        self.low + (score % u64::from(span)) as u8
    }
}

/// Configuration for one note stream within a quad-note player.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuadNoteStreamConfig {
    /// Lane carrying the stream's events.
    pub lane_id: LaneId,
    /// Whether pitches are constrained to the configured scale.
    pub scale_lock: bool,
    /// Pitch window the stream draws from.
    pub pitch_range: QuadNotePitchRange,
    /// Hit density, 0-100.
    pub density: u8,
    /// Rhythmic feel.
    pub rhythm: QuadNoteRhythm,
    /// Velocity window.
    pub velocity_range: QuadNoteVelocityRange,
    /// Per-stream seed mixed into the master seed.
    pub seed: u64,
    /// Output channel.
    pub channel: Channel,
}

impl QuadNoteStreamConfig {
    /// Creates a stream config from a lane id and seed with defaults.
    pub fn new(lane_id: impl Into<String>, seed: u64) -> Self {
        Self {
            lane_id: LaneId::new(lane_id),
            scale_lock: true,
            pitch_range: QuadNotePitchRange::new(Pitch::from_midi(48), Pitch::from_midi(84)),
            density: 50,
            rhythm: QuadNoteRhythm::Even,
            velocity_range: QuadNoteVelocityRange::new(72, 104),
            seed,
            channel: Channel(0),
        }
    }

    /// Sets whether pitches are locked to the scale.
    pub fn with_scale_lock(mut self, scale_lock: bool) -> Self {
        self.scale_lock = scale_lock;
        self
    }

    /// Sets the pitch window.
    pub fn with_pitch_range(mut self, pitch_range: QuadNotePitchRange) -> Self {
        self.pitch_range = pitch_range;
        self
    }

    /// Sets the hit density, clamped to 0-100.
    pub fn with_density(mut self, density: u8) -> Self {
        self.density = percent(density);
        self
    }

    /// Sets the rhythmic feel.
    pub fn with_rhythm(mut self, rhythm: QuadNoteRhythm) -> Self {
        self.rhythm = rhythm;
        self
    }

    /// Sets the velocity window.
    pub fn with_velocity_range(mut self, velocity_range: QuadNoteVelocityRange) -> Self {
        self.velocity_range = velocity_range;
        self
    }

    /// Sets the output channel.
    pub fn with_channel(mut self, channel: Channel) -> Self {
        self.channel = channel;
        self
    }
}

/// Configuration for the [`QuadNotePlayer`] multi-stream generator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuadNoteConfig {
    /// Scale shared by all scale-locked streams.
    pub scale: Scale,
    /// Harmonic spacing between streams.
    pub harmonic_relation: QuadNoteHarmonicRelation,
    /// Master seed mixed with each stream's seed.
    pub master_seed: u64,
    /// Time between steps.
    pub rate: Tick,
    /// Number of steps rendered.
    pub steps: u16,
    /// Note-on duration per hit.
    pub note_length: Tick,
    /// Stream configs (capped at [`QUAD_NOTE_MAX_STREAMS`]).
    pub streams: Vec<QuadNoteStreamConfig>,
    /// Lane carrying trace events.
    pub trace_lane_id: LaneId,
}

impl QuadNoteConfig {
    /// Creates a config from a scale and master seed with no streams.
    pub fn new(scale: Scale, master_seed: u64) -> Self {
        Self {
            scale,
            harmonic_relation: QuadNoteHarmonicRelation::Thirds,
            master_seed,
            rate: tick(120, 480),
            steps: 16,
            note_length: tick(90, 480),
            streams: Vec::new(),
            trace_lane_id: LaneId::new("quad-note-trace"),
        }
    }

    /// Sets the harmonic relation between streams.
    pub fn with_harmonic_relation(mut self, harmonic_relation: QuadNoteHarmonicRelation) -> Self {
        self.harmonic_relation = harmonic_relation;
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

    /// Sets the note-on duration per hit.
    pub fn with_note_length(mut self, note_length: Tick) -> Self {
        self.note_length = note_length;
        self
    }

    /// Appends a stream config.
    pub fn with_stream(mut self, stream: QuadNoteStreamConfig) -> Self {
        self.streams.push(stream);
        self
    }
}

/// Diagnostic record describing one stream's step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuadNoteTrace {
    /// Lane the stream belongs to.
    pub lane_id: LaneId,
    /// Step time.
    pub time: Tick,
    /// Step index.
    pub step: u64,
    /// Index of the stream within the player.
    pub stream_index: usize,
    /// Whether the step fired a note.
    pub emitted: bool,
    /// Rhythmic feel of the stream.
    pub rhythm: QuadNoteRhythm,
    /// Whether the stream is scale-locked.
    pub scale_lock: bool,
    /// Stream hit density.
    pub density: u8,
    /// Degree offset contributed by the harmonic relation.
    pub relation_degree_offset: usize,
    /// Pitch played, if any.
    pub pitch: Option<Pitch>,
    /// Velocity played, if any.
    pub velocity: Option<u8>,
}

/// Rendered quad-note output: play events and per-step traces.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuadNoteRender {
    /// Ordered play events.
    pub events: Vec<PlayEvent>,
    /// Per-step diagnostic traces.
    pub traces: Vec<QuadNoteTrace>,
}

impl QuadNoteRender {
    /// Sorts events and traces into a stable order.
    pub fn stable(mut self) -> Self {
        stable_event_order(&mut self.events);
        self.traces.sort_by(|left, right| {
            left.time
                .cmp(&right.time)
                .then_with(|| left.stream_index.cmp(&right.stream_index))
                .then_with(|| left.lane_id.cmp(&right.lane_id))
                .then_with(|| left.step.cmp(&right.step))
        });
        self
    }
}

/// Multi-stream note generator driven by a [`QuadNoteConfig`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuadNotePlayer {
    /// Configuration driving the render.
    pub config: QuadNoteConfig,
}

impl QuadNotePlayer {
    /// Creates a player from its config.
    pub fn new(config: QuadNoteConfig) -> Self {
        Self { config }
    }

    /// Renders every stream across all steps.
    pub fn render(&self) -> QuadNoteRender {
        let mut render = QuadNoteRender::default();
        for step in 0..u64::from(self.config.steps) {
            for (stream_index, stream) in self
                .config
                .streams
                .iter()
                .take(QUAD_NOTE_MAX_STREAMS)
                .enumerate()
            {
                self.render_stream_step(&mut render, step, stream_index, stream);
            }
        }
        render.stable()
    }

    /// Renders every stream; alias for [`QuadNotePlayer::render`].
    pub fn freeze(&self) -> QuadNoteRender {
        self.render()
    }

    fn render_stream_step(
        &self,
        render: &mut QuadNoteRender,
        step: u64,
        stream_index: usize,
        stream: &QuadNoteStreamConfig,
    ) {
        let time = self.step_time(step);
        let relation_degree_offset = self.config.harmonic_relation.degree_offset(stream_index);
        let emitted = self.should_emit(stream, step, stream_index);
        let (pitch, velocity) = emitted
            .then(|| {
                (
                    self.pitch_for_step(stream, step, stream_index),
                    stream
                        .velocity_range
                        .choose(self.score(stream, step, stream_index, 2)),
                )
            })
            .map_or((None, None), |(pitch, velocity)| {
                (Some(pitch), Some(velocity))
            });
        render.events.push(PlayEvent::Trace(TraceEvent {
            lane_id: self.config.trace_lane_id.clone(),
            time,
            step,
        }));
        render.traces.push(QuadNoteTrace {
            lane_id: stream.lane_id.clone(),
            time,
            step,
            stream_index,
            emitted,
            rhythm: stream.rhythm,
            scale_lock: stream.scale_lock,
            density: stream.density,
            relation_degree_offset,
            pitch,
            velocity,
        });
        let (Some(pitch), Some(velocity)) = (pitch, velocity) else {
            return;
        };
        render.events.push(PlayEvent::Note(NoteEvent {
            lane_id: stream.lane_id.clone(),
            time,
            duration: self.config.note_length,
            pitch,
            velocity,
            channel: stream.channel,
        }));
    }

    fn should_emit(&self, stream: &QuadNoteStreamConfig, step: u64, stream_index: usize) -> bool {
        let threshold = stream.rhythm.threshold(step, stream.density);
        threshold > 0 && self.score(stream, step, stream_index, 0) % 100 < u64::from(threshold)
    }

    fn pitch_for_step(
        &self,
        stream: &QuadNoteStreamConfig,
        step: u64,
        stream_index: usize,
    ) -> Pitch {
        let score = self.score(stream, step, stream_index, 1);
        if !stream.scale_lock {
            return stream.pitch_range.choose_chromatic(score);
        }
        let degree = 1
            + melodic_motion(step, score)
            + self.config.harmonic_relation.degree_offset(stream_index);
        stream
            .pitch_range
            .choose_scale(self.config.scale, degree, score >> 8)
    }

    fn score(
        &self,
        stream: &QuadNoteStreamConfig,
        step: u64,
        stream_index: usize,
        salt: u64,
    ) -> u64 {
        let stream_seed = stable_hash(
            self.config.master_seed ^ stream.seed ^ ((stream_index as u64) << 23) ^ (salt << 47),
        );
        stable_hash(stream_seed ^ (step << 17) ^ (salt << 41))
    }

    fn step_time(&self, step: u64) -> Tick {
        Tick {
            ticks: self.config.rate.ticks * step as i64,
            tpq: self.config.rate.tpq,
        }
    }
}

fn melodic_motion(step: u64, score: u64) -> usize {
    if step.is_multiple_of(4) {
        return 0;
    }
    const MOTION: [usize; 8] = [0, 1, 2, 4, 3, 5, 1, 6];
    MOTION[(score as usize) % MOTION.len()]
}

fn choose_pitch(candidates: &[Pitch], fallback: Pitch, score: u64) -> Pitch {
    if candidates.is_empty() {
        return fallback;
    }
    candidates[(score as usize) % candidates.len()]
}
