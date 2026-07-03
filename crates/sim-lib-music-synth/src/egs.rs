use crate::{Dx7EnvelopeSettings, Dx7FrequencyMode, Dx7PitchSettings, QPhase, midi_key_to_hz};

/// Bit width of the EGS level word.
pub const DX7_EGS_LEVEL_BITS: u8 = 14;
/// Bit width of an EGS rate value.
pub const DX7_EGS_RATE_BITS: u8 = 7;
/// Bit width of the EGS pitch word.
pub const DX7_EGS_PITCH_BITS: u8 = 16;

const LEVEL_MAX: u16 = (1 << DX7_EGS_LEVEL_BITS) - 1;

/// Declared fixed-point word widths of the EGS envelope/pitch generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx7EgsWordWidths {
    /// Level word bit width.
    pub level_bits: u8,
    /// Rate value bit width.
    pub rate_bits: u8,
    /// Pitch word bit width.
    pub pitch_bits: u8,
}

impl Dx7EgsWordWidths {
    /// Returns the word widths the EGS is compiled against.
    pub const fn declared() -> Self {
        Self {
            level_bits: DX7_EGS_LEVEL_BITS,
            rate_bits: DX7_EGS_RATE_BITS,
            pitch_bits: DX7_EGS_PITCH_BITS,
        }
    }
}

/// Current stage of the four-rate EGS envelope state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dx7EgsStage {
    /// Silent, awaiting a gate.
    Idle,
    /// Attack segment toward level 1.
    Rate1,
    /// Decay segment toward level 2.
    Rate2,
    /// Decay segment toward level 3.
    Rate3,
    /// Release segment toward level 4 after gate-off.
    Rate4,
    /// Holding at the sustain level while gated.
    Sustain,
}

/// A running EGS envelope generator that steps toward staged level targets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dx7EgsEnvelope {
    settings: Dx7EnvelopeSettings,
    value: u16,
    stage: Dx7EgsStage,
    gate: bool,
}

impl Dx7EgsEnvelope {
    /// Creates an idle envelope from the given rate/level settings.
    pub fn new(settings: Dx7EnvelopeSettings) -> Self {
        Self {
            settings,
            value: 0,
            stage: Dx7EgsStage::Idle,
            gate: false,
        }
    }

    /// Returns the current envelope level word.
    pub fn value(&self) -> u16 {
        self.value
    }

    /// Returns the current envelope stage.
    pub fn stage(&self) -> Dx7EgsStage {
        self.stage
    }

    /// Resets the envelope to idle with zero level and gate released.
    pub fn reset(&mut self) {
        self.value = 0;
        self.stage = Dx7EgsStage::Idle;
        self.gate = false;
    }

    /// Advances the envelope one sample given the current gate, applying a
    /// keyboard rate-scale boost, and returns the new level word.
    pub fn next_level(&mut self, gate: bool, rate_scale_boost: u8) -> u16 {
        if gate && !self.gate {
            self.stage = Dx7EgsStage::Rate1;
        } else if !gate && self.gate {
            self.stage = Dx7EgsStage::Rate4;
        }
        self.gate = gate;

        let Some(target) = self.target() else {
            self.value = 0;
            return self.value;
        };
        let rate = self.rate().saturating_add(rate_scale_boost).min(99);
        self.value = step_toward(self.value, level_to_word(target), rate);
        if self.value == level_to_word(target) {
            self.stage = self.next_stage();
        }
        self.value
    }

    fn target(&self) -> Option<u8> {
        match self.stage {
            Dx7EgsStage::Idle => None,
            Dx7EgsStage::Rate1 => Some(self.settings.levels[0]),
            Dx7EgsStage::Rate2 => Some(self.settings.levels[1]),
            Dx7EgsStage::Rate3 | Dx7EgsStage::Sustain => Some(self.settings.levels[2]),
            Dx7EgsStage::Rate4 => Some(self.settings.levels[3]),
        }
    }

    fn rate(&self) -> u8 {
        match self.stage {
            Dx7EgsStage::Idle | Dx7EgsStage::Sustain => 0,
            Dx7EgsStage::Rate1 => self.settings.rates[0],
            Dx7EgsStage::Rate2 => self.settings.rates[1],
            Dx7EgsStage::Rate3 => self.settings.rates[2],
            Dx7EgsStage::Rate4 => self.settings.rates[3],
        }
    }

    fn next_stage(&self) -> Dx7EgsStage {
        match self.stage {
            Dx7EgsStage::Idle => Dx7EgsStage::Idle,
            Dx7EgsStage::Rate1 => Dx7EgsStage::Rate2,
            Dx7EgsStage::Rate2 => Dx7EgsStage::Rate3,
            Dx7EgsStage::Rate3 | Dx7EgsStage::Sustain => Dx7EgsStage::Sustain,
            Dx7EgsStage::Rate4 => Dx7EgsStage::Idle,
        }
    }
}

impl Default for Dx7EgsEnvelope {
    fn default() -> Self {
        Self::new(Dx7EnvelopeSettings::constant(99))
    }
}

/// EGS pitch generator: converts pitch settings into integer pitch words and
/// per-sample phase increments at a given sample rate.
#[derive(Clone, Debug, PartialEq)]
pub struct Dx7EgsPitch {
    sample_rate_hz: u32,
}

impl Dx7EgsPitch {
    /// Creates a pitch generator for the given sample rate (clamped to >= 1).
    pub fn new(sample_rate_hz: u32) -> Self {
        Self {
            sample_rate_hz: sample_rate_hz.max(1),
        }
    }

    /// Updates the sample rate used for phase-increment computation.
    pub fn set_sample_rate(&mut self, sample_rate_hz: u32) {
        self.sample_rate_hz = sample_rate_hz.max(1);
    }

    /// Computes the integer EGS pitch word from settings, key, and a pitch
    /// modulation offset in semitones.
    pub fn pitch_word(&self, settings: Dx7PitchSettings, key: u8, pitch_mod_semitones: i16) -> i32 {
        let mode = match settings.mode {
            Dx7FrequencyMode::Ratio => 0,
            Dx7FrequencyMode::Fixed => 1 << 15,
        };
        mode + (i32::from(key) << 8)
            + (i32::from(settings.coarse.min(31)) << 3)
            + i32::from(settings.fine.min(99))
            + (i32::from(settings.detune) << 4)
            + i32::from(pitch_mod_semitones)
    }

    /// Computes the per-sample phase increment for the given pitch settings,
    /// key, and pitch modulation in semitones.
    pub fn phase_delta(
        &self,
        settings: Dx7PitchSettings,
        key: u8,
        pitch_mod_semitones: f32,
    ) -> QPhase {
        let frequency = settings.frequency_hz(key, pitch_mod_semitones);
        let turns = f64::from(frequency) / f64::from(self.sample_rate_hz);
        QPhase::from_turns(turns)
    }
}

impl Default for Dx7EgsPitch {
    fn default() -> Self {
        Self::new(48_000)
    }
}

/// Returns the modeled operator frequency in hertz for the given pitch
/// settings, key, and pitch modulation, handling ratio and fixed modes.
pub fn modeled_pitch_hz(settings: Dx7PitchSettings, key: u8, pitch_mod_semitones: f32) -> f32 {
    match settings.mode {
        Dx7FrequencyMode::Ratio => settings.frequency_hz(key, pitch_mod_semitones),
        Dx7FrequencyMode::Fixed => {
            let base = settings.frequency_hz(60, pitch_mod_semitones);
            base.max(midi_key_to_hz(0) / 128.0)
        }
    }
}

fn step_toward(current: u16, target: u16, rate: u8) -> u16 {
    if current == target {
        return current;
    }
    let rate = u32::from(rate) + 1;
    let step = ((rate * rate * u32::from(LEVEL_MAX)) / 10_000).max(1) as u16;
    if current < target {
        current.saturating_add(step).min(target)
    } else {
        current.saturating_sub(step).max(target)
    }
}

fn level_to_word(level: u8) -> u16 {
    ((u32::from(level.min(99)) * u32::from(LEVEL_MAX)) / 99) as u16
}
