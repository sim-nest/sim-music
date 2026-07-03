use crate::{Dx7Lfo, GeneratedLut, QLevel, QPhase};

/// One LFO output frame: the pitch modulation and amplitude modulation
/// produced for a single sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dx7LfoFrame {
    /// Pitch modulation in semitones to add to the operator frequency.
    pub pitch_semitones: f32,
    /// Amplitude modulation depth applied to the operator output level.
    pub amp: QLevel,
}

/// Settings for the algorithmic (LUT-driven) DX7 low-frequency oscillator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx7AlgorithmicLfoSettings {
    /// LFO speed (0..=99), mapped nonlinearly to an oscillation rate in Hz.
    pub speed: u8,
    /// Delay (0..=99) before the LFO fades in after note-on.
    pub delay: u8,
    /// Pitch modulation depth (0..=99).
    pub pitch_mod_depth: u8,
    /// Amplitude modulation depth (0..=99).
    pub amp_mod_depth: u8,
    /// Whether the LFO phase resets (syncs) on each note-on.
    pub sync: bool,
    /// Waveform selector (0..=5): sine, triangle, saw-up, saw-down, and
    /// square shapes.
    pub waveform: u8,
    /// Pitch modulation sensitivity (0..=7) scaling the pitch mod depth.
    pub pitch_mod_sens: u8,
}

impl Dx7AlgorithmicLfoSettings {
    /// Builds LFO settings from a patch LFO block, clamping each field to its
    /// valid DX7 range.
    pub fn from_patch_lfo(lfo: &Dx7Lfo) -> Self {
        Self {
            speed: lfo.speed.min(99),
            delay: lfo.delay.min(99),
            pitch_mod_depth: lfo.pitch_mod_depth.min(99),
            amp_mod_depth: lfo.amp_mod_depth.min(99),
            sync: lfo.sync,
            waveform: lfo.waveform.min(5),
            pitch_mod_sens: lfo.pitch_mod_sens.min(7),
        }
    }
}

impl Default for Dx7AlgorithmicLfoSettings {
    /// Returns a moderate-speed sine LFO with no delay and zero modulation
    /// depth, so it is inaudible until depths are set.
    fn default() -> Self {
        Self {
            speed: 35,
            delay: 0,
            pitch_mod_depth: 0,
            amp_mod_depth: 0,
            sync: false,
            waveform: 0,
            pitch_mod_sens: 0,
        }
    }
}

/// Algorithmic DX7 low-frequency oscillator that generates pitch and
/// amplitude modulation frames from a sine lookup table.
#[derive(Clone, Debug, PartialEq)]
pub struct Dx7AlgorithmicLfo {
    settings: Dx7AlgorithmicLfoSettings,
    sample_rate_hz: f32,
    phase: QPhase,
    clock: u64,
    sine: GeneratedLut,
}

impl Dx7AlgorithmicLfo {
    /// Creates an LFO with the given settings at a default 48 kHz sample rate.
    pub fn new(settings: Dx7AlgorithmicLfoSettings) -> Self {
        Self {
            settings,
            sample_rate_hz: 48_000.0,
            phase: QPhase::ZERO,
            clock: 0,
            sine: GeneratedLut::sine(256),
        }
    }

    /// Sets the playback sample rate in Hz (clamped to at least 1.0), which
    /// determines the per-sample phase increment.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Resets the LFO phase and delay clock to their initial state.
    pub fn reset(&mut self) {
        self.phase = QPhase::ZERO;
        self.clock = 0;
    }

    /// Advances the LFO by one sample and returns its pitch and amplitude
    /// modulation frame, returning zeros while still inside the delay window.
    pub fn next_frame(&mut self) -> Dx7LfoFrame {
        if self.clock < self.delay_samples() {
            self.clock = self.clock.saturating_add(1);
            return Dx7LfoFrame {
                pitch_semitones: 0.0,
                amp: QLevel::ZERO,
            };
        }
        let wave = self.wave_sample();
        let pitch_depth = f32::from(self.settings.pitch_mod_depth)
            * f32::from(self.settings.pitch_mod_sens)
            / (99.0 * 7.0);
        let amp_depth = f32::from(self.settings.amp_mod_depth) / 99.0;
        let frame = Dx7LfoFrame {
            pitch_semitones: wave * pitch_depth,
            amp: QLevel::from_f32((wave.abs() * amp_depth).clamp(0.0, 1.0)),
        };
        self.phase.advance_wrapping(QPhase::from_turns(
            self.rate_hz() as f64 / self.sample_rate_hz as f64,
        ));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn rate_hz(&self) -> f32 {
        let normalized = (f32::from(self.settings.speed) + 1.0) / 100.0;
        0.1 + normalized * normalized * 20.0
    }

    fn delay_samples(&self) -> u64 {
        let seconds = f32::from(self.settings.delay) / 99.0 * 2.0;
        (seconds * self.sample_rate_hz) as u64
    }

    fn wave_sample(&self) -> f32 {
        let turns = self.phase.turns() as f32;
        match self.settings.waveform {
            1 => 1.0 - 4.0 * (turns - 0.5).abs(),
            2 => 2.0 * turns - 1.0,
            3 => 1.0 - 2.0 * turns,
            4 => {
                if turns < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            _ => self.sine.sample_phase(self.phase),
        }
    }
}

impl Default for Dx7AlgorithmicLfo {
    /// Returns an LFO built from the default settings.
    fn default() -> Self {
        Self::new(Dx7AlgorithmicLfoSettings::default())
    }
}
