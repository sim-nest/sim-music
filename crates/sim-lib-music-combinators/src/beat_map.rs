use sim_lib_music_core::{Channel, LaneId, Tick};
use sim_lib_sound_gm::DrumKeyMap;

use crate::{
    DrumHit, DrumPatternRender, PatternAutomation, PatternRegion, percent, push_drum_hit,
    stable_hash, tick,
};

/// One drum voice in a beat map: a sound name and its output lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeatMapLane {
    /// Drum sound name resolved against the kit.
    pub sound: String,
    /// Lane carrying the voice's events.
    pub lane_id: LaneId,
}

impl BeatMapLane {
    /// Creates a lane from a sound name and lane id.
    pub fn new(sound: impl Into<String>, lane_id: impl Into<String>) -> Self {
        Self {
            sound: sound.into(),
            lane_id: LaneId::new(lane_id),
        }
    }
}

/// Configuration for the generative [`BeatMapPlayer`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeatMapConfig {
    /// X coordinate on the pattern morph grid.
    pub x: u8,
    /// Y coordinate on the pattern morph grid.
    pub y: u8,
    /// Probability density of hits, 0-100.
    pub density: u8,
    /// Pattern complexity, 0-100.
    pub complexity: u8,
    /// Swing amount, 0-100.
    pub swing: u8,
    /// Fill intensity near phrase ends, 0-100.
    pub fill: u8,
    /// Whether lanes are mirrored across the lane order.
    pub mirror_lanes: bool,
    /// Seed for deterministic generation.
    pub seed: u64,
    /// Time between steps.
    pub rate: Tick,
    /// Note-on duration per hit.
    pub gate: Tick,
    /// Number of steps rendered.
    pub steps: u16,
    /// Output channel.
    pub channel: Channel,
    /// Drum key map resolving sound names to MIDI keys.
    pub kit: DrumKeyMap,
    /// Drum voices to render.
    pub lanes: Vec<BeatMapLane>,
    /// Region automation gating which steps are active.
    pub automation: PatternAutomation,
}

impl BeatMapConfig {
    /// Creates a config from a seed with a default 3-voice kit.
    pub fn new(seed: u64) -> Self {
        Self {
            x: 50,
            y: 50,
            density: 45,
            complexity: 35,
            swing: 0,
            fill: 0,
            mirror_lanes: false,
            seed,
            rate: tick(120, 480),
            gate: tick(90, 480),
            steps: 16,
            channel: Channel(9),
            kit: DrumKeyMap::gm(),
            lanes: vec![
                BeatMapLane::new("kick", "beat-kick"),
                BeatMapLane::new("snare", "beat-snare"),
                BeatMapLane::new("closed-hat", "beat-hat"),
            ],
            automation: PatternAutomation::always(),
        }
    }

    /// Adds an active region to the pattern automation.
    pub fn with_region(mut self, region: PatternRegion) -> Self {
        self.automation.active_regions.push(region);
        self
    }
}

/// Generative drum-pattern player driven by a [`BeatMapConfig`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeatMapPlayer {
    /// Configuration driving the render.
    pub config: BeatMapConfig,
    trace_lane_id: LaneId,
}

impl BeatMapPlayer {
    /// Creates a player from its config.
    pub fn new(config: BeatMapConfig) -> Self {
        Self {
            config,
            trace_lane_id: LaneId::new("beat-map-trace"),
        }
    }

    /// Renders the drum pattern across all active steps and lanes.
    pub fn render(&self) -> DrumPatternRender {
        let mut render = DrumPatternRender::default();
        for step in 0..self.config.steps {
            let step = u64::from(step);
            if !self.config.automation.is_active(step) {
                continue;
            }
            for (lane_index, lane) in self.config.lanes.iter().enumerate() {
                self.render_lane(&mut render, lane, lane_index, step, false);
                if self.config.mirror_lanes {
                    let mirror_index = self.config.lanes.len().saturating_sub(lane_index + 1);
                    self.render_lane(&mut render, lane, mirror_index, step, true);
                }
            }
        }
        render.stable()
    }

    /// Renders the pattern; alias for [`BeatMapPlayer::render`].
    pub fn freeze(&self) -> DrumPatternRender {
        self.render()
    }

    fn render_lane(
        &self,
        render: &mut DrumPatternRender,
        lane: &BeatMapLane,
        lane_index: usize,
        step: u64,
        mirrored: bool,
    ) {
        if !self.hit(step, lane_index, mirrored) {
            return;
        }
        let key = self.config.kit.remap(&lane.sound, 36);
        let time = self.step_time(step);
        push_drum_hit(
            render,
            DrumHit {
                player: "beat-map",
                lane_id: &lane.lane_id,
                trace_lane_id: &self.trace_lane_id,
                time,
                duration: self.config.gate,
                step,
                sound: &lane.sound,
                key,
                velocity: self.velocity(step, lane_index, mirrored),
                channel: self.config.channel,
            },
        );
    }

    fn hit(&self, step: u64, lane_index: usize, mirrored: bool) -> bool {
        let density = u16::from(percent(self.config.density));
        let complexity = u16::from(percent(self.config.complexity));
        let fill = if is_fill_step(step, self.config.steps) {
            u16::from(percent(self.config.fill)) / 2
        } else {
            0
        };
        let lane_bias = ((lane_index as u16 * 13 + complexity / 4) % 21).min(complexity);
        let threshold = (density + lane_bias + fill).min(100);
        let score = self.score(step, lane_index, mirrored) % 100;
        score < u64::from(threshold)
    }

    fn velocity(&self, step: u64, lane_index: usize, mirrored: bool) -> u8 {
        let accent = if step.is_multiple_of(4) { 18 } else { 0 };
        let human = (self.score(step, lane_index, mirrored) % 18) as u8;
        68u8.saturating_add(accent)
            .saturating_add(percent(self.config.complexity) / 5)
            .saturating_add(human)
            .min(127)
    }

    fn step_time(&self, step: u64) -> Tick {
        let mut ticks = self.config.rate.ticks * step as i64;
        if step % 2 == 1 {
            ticks += self.config.rate.ticks * i64::from(percent(self.config.swing)) / 200;
        }
        Tick {
            ticks,
            tpq: self.config.rate.tpq,
        }
    }

    fn score(&self, step: u64, lane_index: usize, mirrored: bool) -> u64 {
        let mirror = u64::from(mirrored);
        stable_hash(
            self.config.seed
                ^ (step << 17)
                ^ ((lane_index as u64) << 9)
                ^ (u64::from(self.config.x) << 25)
                ^ (u64::from(self.config.y) << 33)
                ^ (mirror << 41),
        )
    }
}

fn is_fill_step(step: u64, steps: u16) -> bool {
    let steps = u64::from(steps.max(1));
    step + 4 >= steps || step % 16 >= 12
}
