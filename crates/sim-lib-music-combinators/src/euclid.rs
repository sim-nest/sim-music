use sim_lib_music_core::{Channel, LaneId, Tick};
use sim_lib_sound_gm::DrumKeyMap;

use crate::{DrumHit, DrumPatternRender, PatternAutomation, push_drum_hit, tick};

/// One Euclidean rhythm voice: a drum sound and its pulse distribution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EuclidLane {
    /// Drum sound name resolved against the kit.
    pub sound: String,
    /// Lane carrying the voice's events.
    pub lane_id: LaneId,
    /// Number of pulses spread across the steps.
    pub pulses: u16,
    /// Length of the Euclidean cycle.
    pub steps: u16,
    /// Rotation offset of the pulse pattern.
    pub rotation: u16,
    /// Accent every Nth pulse; zero disables accents.
    pub accent_every: u16,
    /// Base velocity.
    pub velocity: u8,
    /// Velocity used on accented pulses.
    pub accent_velocity: u8,
    /// Output channel.
    pub channel: Channel,
}

impl EuclidLane {
    /// Creates a lane with `pulses` spread over `steps` and defaults.
    pub fn new(sound: impl Into<String>, pulses: u16, steps: u16) -> Self {
        let sound = sound.into();
        Self {
            lane_id: LaneId::new(format!("euclid-{sound}")),
            sound,
            pulses,
            steps: steps.max(1),
            rotation: 0,
            accent_every: 0,
            velocity: 86,
            accent_velocity: 112,
            channel: Channel(9),
        }
    }

    /// Sets the pattern rotation offset.
    pub fn with_rotation(mut self, rotation: u16) -> Self {
        self.rotation = rotation;
        self
    }

    /// Sets the accent interval in pulses.
    pub fn with_accent_every(mut self, accent_every: u16) -> Self {
        self.accent_every = accent_every;
        self
    }

    /// Overrides the lane id.
    pub fn with_lane(mut self, lane_id: impl Into<String>) -> Self {
        self.lane_id = LaneId::new(lane_id);
        self
    }
}

/// Configuration for the [`EuclideanPlayer`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EuclidConfig {
    /// Time between steps.
    pub rate: Tick,
    /// Note-on duration per hit.
    pub gate: Tick,
    /// Total number of steps rendered.
    pub step_count: u16,
    /// Drum key map resolving sound names to MIDI keys.
    pub kit: DrumKeyMap,
    /// Euclidean voices to render.
    pub lanes: Vec<EuclidLane>,
    /// Region automation gating which steps are active.
    pub automation: PatternAutomation,
}

impl EuclidConfig {
    /// Creates a config for the given step count with an empty lane set.
    pub fn new(step_count: u16) -> Self {
        Self {
            rate: tick(120, 480),
            gate: tick(90, 480),
            step_count,
            kit: DrumKeyMap::gm(),
            lanes: Vec::new(),
            automation: PatternAutomation::always(),
        }
    }

    /// Appends a Euclidean voice to the config.
    pub fn with_lane(mut self, lane: EuclidLane) -> Self {
        self.lanes.push(lane);
        self
    }
}

/// Drum player that renders Euclidean rhythm lanes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EuclideanPlayer {
    /// Configuration driving the render.
    pub config: EuclidConfig,
    trace_lane_id: LaneId,
}

impl EuclideanPlayer {
    /// Creates a player from its config.
    pub fn new(config: EuclidConfig) -> Self {
        Self {
            config,
            trace_lane_id: LaneId::new("euclid-trace"),
        }
    }

    /// Renders the Euclidean pattern across all active steps and lanes.
    pub fn render(&self) -> DrumPatternRender {
        let mut render = DrumPatternRender::default();
        for step in 0..self.config.step_count {
            let step = u64::from(step);
            if !self.config.automation.is_active(step) {
                continue;
            }
            for lane in &self.config.lanes {
                self.render_lane(&mut render, lane, step);
            }
        }
        render.stable()
    }

    /// Renders the pattern; alias for [`EuclideanPlayer::render`].
    pub fn freeze(&self) -> DrumPatternRender {
        self.render()
    }

    fn render_lane(&self, render: &mut DrumPatternRender, lane: &EuclidLane, step: u64) {
        let local_step = (step % u64::from(lane.steps.max(1))) as u16;
        if !euclid_hit(local_step, lane.pulses, lane.steps, lane.rotation) {
            return;
        }
        let key = self.config.kit.remap(&lane.sound, 36);
        let time = self.step_time(step);
        push_drum_hit(
            render,
            DrumHit {
                player: "euclid",
                lane_id: &lane.lane_id,
                trace_lane_id: &self.trace_lane_id,
                time,
                duration: self.config.gate,
                step,
                sound: &lane.sound,
                key,
                velocity: self.velocity(lane, local_step),
                channel: lane.channel,
            },
        );
    }

    fn velocity(&self, lane: &EuclidLane, local_step: u16) -> u8 {
        if lane.accent_every == 0 {
            return lane.velocity;
        }
        let hit_index = hit_index(local_step, lane.pulses, lane.steps, lane.rotation);
        if hit_index.is_some_and(|index| index % lane.accent_every == 0) {
            lane.accent_velocity
        } else {
            lane.velocity
        }
    }

    fn step_time(&self, step: u64) -> Tick {
        Tick {
            ticks: self.config.rate.ticks * step as i64,
            tpq: self.config.rate.tpq,
        }
    }
}

/// Reports whether a step lands on a pulse of a Euclidean rhythm.
///
/// Distributes `pulses` as evenly as possible across `steps`, applying the
/// given rotation.
pub fn euclid_hit(step: u16, pulses: u16, steps: u16, rotation: u16) -> bool {
    let steps = steps.max(1);
    let pulses = pulses.min(steps);
    if pulses == 0 {
        return false;
    }
    let steps_u32 = u32::from(steps);
    let rotated = (u32::from(step) + u32::from(rotation)) % steps_u32;
    (rotated * u32::from(pulses)) % steps_u32 < u32::from(pulses)
}

fn hit_index(step: u16, pulses: u16, steps: u16, rotation: u16) -> Option<u16> {
    if !euclid_hit(step, pulses, steps, rotation) {
        return None;
    }
    let mut count = 0;
    for candidate in 0..=step {
        if euclid_hit(candidate, pulses, steps, rotation) {
            count += 1;
        }
    }
    Some(count)
}
