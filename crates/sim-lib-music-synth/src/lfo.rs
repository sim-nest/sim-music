//! Low-frequency oscillator (LFO) with optional tempo sync.
//!
//! Provides [`LfoSettings`], the [`TempoSync`] descriptor that locks an LFO
//! cycle to a number of beats, and [`Lfo`], a depth-scaled modulation source
//! built on a [`PhaseOscillator`].

use crate::{Oscillator, OscillatorKind, PhaseOscillator};

/// Tempo-sync descriptor: the number of beats spanned by one LFO cycle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TempoSync {
    /// Number of beats per full LFO cycle.
    pub beats_per_cycle: f32,
}

/// Configuration for an [`Lfo`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LfoSettings {
    /// Modulation waveform.
    pub waveform: OscillatorKind,
    /// Free-running rate, in Hz, used when tempo sync is disabled.
    pub rate_hz: f32,
    /// Output modulation depth applied as a multiplier.
    pub depth: f32,
    /// Optional tempo sync; when set, the rate is derived from the host tempo.
    pub tempo_sync: Option<TempoSync>,
}

impl Default for LfoSettings {
    fn default() -> Self {
        Self {
            waveform: OscillatorKind::Sine,
            rate_hz: 5.0,
            depth: 0.0,
            tempo_sync: None,
        }
    }
}

/// A depth-scaled low-frequency modulation source.
#[derive(Clone, Debug, PartialEq)]
pub struct Lfo {
    settings: LfoSettings,
    oscillator: PhaseOscillator,
}

impl Lfo {
    /// Builds an LFO from sanitized `settings`, falling back to a sine waveform
    /// when the requested waveform is [`OscillatorKind::Wavetable`].
    pub fn new(settings: LfoSettings) -> Self {
        let mut oscillator = PhaseOscillator::new(settings.waveform, settings.rate_hz.max(0.0));
        if matches!(settings.waveform, OscillatorKind::Wavetable) {
            oscillator = PhaseOscillator::sine(settings.rate_hz.max(0.0));
        }
        Self {
            settings: sanitize(settings),
            oscillator,
        }
    }

    /// Returns the current (sanitized) LFO settings.
    pub fn settings(&self) -> LfoSettings {
        self.settings
    }

    /// Sets the sample rate, in Hz, of the underlying oscillator.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.oscillator.set_sample_rate(sample_rate_hz);
    }

    /// Resets the LFO phase to its starting position.
    pub fn reset(&mut self) {
        self.oscillator.reset();
    }

    /// Updates the oscillator frequency from the host tempo; when tempo sync is
    /// set the rate is `bpm / 60 / beats_per_cycle`, otherwise the free-running
    /// [`LfoSettings::rate_hz`] is used.
    pub fn set_tempo_bpm(&mut self, tempo_bpm: f64) {
        if let Some(sync) = self.settings.tempo_sync {
            let beats = sync.beats_per_cycle.max(0.001);
            let hz = (tempo_bpm.max(1.0) as f32 / 60.0) / beats;
            self.oscillator.set_frequency(hz);
        } else {
            self.oscillator.set_frequency(self.settings.rate_hz);
        }
    }

    /// Advances the LFO and returns the next sample scaled by the modulation
    /// depth.
    pub fn next_sample(&mut self) -> f32 {
        self.oscillator.next_sample() * self.settings.depth
    }
}

impl Default for Lfo {
    fn default() -> Self {
        Self::new(LfoSettings::default())
    }
}

fn sanitize(settings: LfoSettings) -> LfoSettings {
    LfoSettings {
        waveform: settings.waveform,
        rate_hz: settings.rate_hz.max(0.0),
        depth: settings.depth.max(0.0),
        tempo_sync: settings.tempo_sync.map(|sync| TempoSync {
            beats_per_cycle: sync.beats_per_cycle.max(0.001),
        }),
    }
}
