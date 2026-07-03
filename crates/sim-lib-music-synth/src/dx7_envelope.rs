use crate::QLevel;

/// Current stage of the DX7 four-rate/four-level envelope generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dx7EnvelopeStage {
    /// Inactive: the envelope holds at zero until a gate opens.
    Idle,
    /// First attack segment, ramping toward level 1 after note-on.
    Rate1,
    /// Second segment, ramping toward level 2.
    Rate2,
    /// Third segment, ramping toward level 3.
    Rate3,
    /// Fourth (release) segment, ramping toward level 4 after note-off.
    Rate4,
    /// Sustain hold at level 3 while the gate stays open.
    Sustain,
}

/// The four rates and four levels that define a DX7 operator or pitch
/// envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx7EnvelopeSettings {
    /// Segment rates R1..=R4 in DX7 units (0..=99); higher is faster.
    pub rates: [u8; 4],
    /// Segment target levels L1..=L4 in DX7 units (0..=99).
    pub levels: [u8; 4],
}

impl Dx7EnvelopeSettings {
    /// Creates envelope settings, clamping every rate and level to the DX7
    /// range 0..=99.
    pub fn new(rates: [u8; 4], levels: [u8; 4]) -> Self {
        Self {
            rates: rates.map(clamp_dx7_rate),
            levels: levels.map(clamp_dx7_level),
        }
    }

    /// Creates settings that hold a constant `level` by using the fastest
    /// rates and the same target level for every segment.
    pub fn constant(level: u8) -> Self {
        Self::new([99; 4], [level; 4])
    }
}

impl Default for Dx7EnvelopeSettings {
    /// Returns a fast organ-style envelope: maximum rates with full sustain
    /// and a release to zero.
    fn default() -> Self {
        Self::new([99; 4], [99, 99, 99, 0])
    }
}

/// Stateful DX7 envelope generator that steps a level toward each segment's
/// target as the gate opens and closes.
#[derive(Clone, Debug, PartialEq)]
pub struct Dx7EnvelopeGenerator {
    settings: Dx7EnvelopeSettings,
    sample_rate_hz: f32,
    stage: Dx7EnvelopeStage,
    value: QLevel,
    gate: bool,
}

impl Dx7EnvelopeGenerator {
    /// Creates an idle envelope generator with the given settings at a default
    /// 48 kHz sample rate.
    pub fn new(settings: Dx7EnvelopeSettings) -> Self {
        Self {
            settings,
            sample_rate_hz: 48_000.0,
            stage: Dx7EnvelopeStage::Idle,
            value: QLevel::ZERO,
            gate: false,
        }
    }

    /// Returns the rates and levels currently driving this envelope.
    pub fn settings(&self) -> Dx7EnvelopeSettings {
        self.settings
    }

    /// Returns the current envelope stage.
    pub fn stage(&self) -> Dx7EnvelopeStage {
        self.stage
    }

    /// Returns the current envelope level.
    pub fn value(&self) -> QLevel {
        self.value
    }

    /// Sets the playback sample rate in Hz (clamped to at least 1.0), which
    /// scales the per-sample step size.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Resets the envelope to idle at zero level with the gate closed.
    pub fn reset(&mut self) {
        self.stage = Dx7EnvelopeStage::Idle;
        self.value = QLevel::ZERO;
        self.gate = false;
    }

    /// Advances the envelope one sample and returns the new level.
    ///
    /// A rising `gate` starts the attack at [`Dx7EnvelopeStage::Rate1`] and a
    /// falling gate jumps to the release at [`Dx7EnvelopeStage::Rate4`];
    /// `rate_scale_boost` (from keyboard rate scaling) adds to the segment
    /// rate to advance faster.
    pub fn next_level(&mut self, gate: bool, rate_scale_boost: u8) -> QLevel {
        if gate && !self.gate {
            self.stage = Dx7EnvelopeStage::Rate1;
        } else if !gate && self.gate {
            self.stage = Dx7EnvelopeStage::Rate4;
        }
        self.gate = gate;

        let Some(target) = self.stage_target() else {
            self.value = QLevel::ZERO;
            return self.value;
        };
        let rate = self.stage_rate().saturating_add(rate_scale_boost).min(99);
        let next = step_toward(
            self.value.raw(),
            level_raw(target),
            rate,
            self.sample_rate_hz,
        );
        self.value = QLevel::from_raw(next);
        if next == level_raw(target) {
            self.stage = self.next_stage();
        }
        self.value
    }

    fn stage_target(&self) -> Option<u8> {
        match self.stage {
            Dx7EnvelopeStage::Idle => None,
            Dx7EnvelopeStage::Rate1 => Some(self.settings.levels[0]),
            Dx7EnvelopeStage::Rate2 => Some(self.settings.levels[1]),
            Dx7EnvelopeStage::Rate3 | Dx7EnvelopeStage::Sustain => Some(self.settings.levels[2]),
            Dx7EnvelopeStage::Rate4 => Some(self.settings.levels[3]),
        }
    }

    fn stage_rate(&self) -> u8 {
        match self.stage {
            Dx7EnvelopeStage::Idle | Dx7EnvelopeStage::Sustain => 0,
            Dx7EnvelopeStage::Rate1 => self.settings.rates[0],
            Dx7EnvelopeStage::Rate2 => self.settings.rates[1],
            Dx7EnvelopeStage::Rate3 => self.settings.rates[2],
            Dx7EnvelopeStage::Rate4 => self.settings.rates[3],
        }
    }

    fn next_stage(&self) -> Dx7EnvelopeStage {
        match self.stage {
            Dx7EnvelopeStage::Idle => Dx7EnvelopeStage::Idle,
            Dx7EnvelopeStage::Rate1 => Dx7EnvelopeStage::Rate2,
            Dx7EnvelopeStage::Rate2 => Dx7EnvelopeStage::Rate3,
            Dx7EnvelopeStage::Rate3 | Dx7EnvelopeStage::Sustain => Dx7EnvelopeStage::Sustain,
            Dx7EnvelopeStage::Rate4 => Dx7EnvelopeStage::Idle,
        }
    }
}

impl Default for Dx7EnvelopeGenerator {
    /// Returns a generator built from the default envelope settings.
    fn default() -> Self {
        Self::new(Dx7EnvelopeSettings::default())
    }
}

fn step_toward(current: i32, target: i32, rate: u8, sample_rate_hz: f32) -> i32 {
    if current == target {
        return current;
    }
    let normalized = (f32::from(rate) + 1.0) / 100.0;
    let scale = 48_000.0 / sample_rate_hz.max(1.0);
    let step = (normalized * normalized * QLevel::ONE.raw() as f32 * scale)
        .round()
        .max(1.0) as i32;
    if current < target {
        current.saturating_add(step).min(target)
    } else {
        current.saturating_sub(step).max(target)
    }
}

fn level_raw(level: u8) -> i32 {
    ((i64::from(clamp_dx7_level(level)) * i64::from(QLevel::ONE.raw())) / 99) as i32
}

fn clamp_dx7_rate(value: u8) -> u8 {
    value.min(99)
}

fn clamp_dx7_level(value: u8) -> u8 {
    value.min(99)
}
