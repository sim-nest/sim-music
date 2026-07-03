//! Phase-accumulating oscillators and their waveforms.
//!
//! Provides the [`Oscillator`] trait, the set of supported waveforms
//! ([`OscillatorKind`]), and [`PhaseOscillator`], a single phase-accumulator
//! voice that renders naive and band-limited (PolyBLEP) shapes as well as
//! linearly interpolated wavetables.

use std::f32::consts::TAU;

/// The waveform a [`PhaseOscillator`] produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OscillatorKind {
    /// Pure sine wave.
    Sine,
    /// Naive (non-band-limited) sawtooth.
    Saw,
    /// Naive pulse/square wave shaped by the pulse width.
    Square,
    /// Triangle wave.
    Triangle,
    /// Band-limited sawtooth using PolyBLEP anti-aliasing.
    PolyBlepSaw,
    /// Band-limited pulse/square using PolyBLEP anti-aliasing.
    PolyBlepSquare,
    /// Linearly interpolated lookup over a sample wavetable.
    Wavetable,
}

impl OscillatorKind {
    /// Returns the stable lowercase name of the waveform.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sine => "sine",
            Self::Saw => "saw",
            Self::Square => "square",
            Self::Triangle => "triangle",
            Self::PolyBlepSaw => "polyblep-saw",
            Self::PolyBlepSquare => "polyblep-square",
            Self::Wavetable => "wavetable",
        }
    }

    /// Parses a waveform from its lowercase name, returning `None` if
    /// unrecognized.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "sine" => Some(Self::Sine),
            "saw" => Some(Self::Saw),
            "square" => Some(Self::Square),
            "triangle" => Some(Self::Triangle),
            "polyblep-saw" => Some(Self::PolyBlepSaw),
            "polyblep-square" => Some(Self::PolyBlepSquare),
            "wavetable" => Some(Self::Wavetable),
            _ => None,
        }
    }
}

/// A frequency- and sample-rate-aware audio oscillator that yields one sample
/// per call.
pub trait Oscillator: Clone + Send {
    /// Resets the oscillator's phase to its starting position.
    fn reset(&mut self);
    /// Sets the sample rate, in Hz, used to convert frequency to phase steps.
    fn set_sample_rate(&mut self, sample_rate_hz: f32);
    /// Sets the oscillator frequency, in Hz.
    fn set_frequency(&mut self, frequency_hz: f32);
    /// Advances the phase and returns the next output sample.
    fn next_sample(&mut self) -> f32;
}

/// A single phase-accumulating oscillator that renders any [`OscillatorKind`].
#[derive(Clone, Debug, PartialEq)]
pub struct PhaseOscillator {
    kind: OscillatorKind,
    frequency_hz: f32,
    sample_rate_hz: f32,
    phase: f32,
    pulse_width: f32,
    wavetable: Vec<f32>,
}

impl PhaseOscillator {
    /// Builds an oscillator of the given waveform and frequency at the default
    /// 48 kHz sample rate.
    pub fn new(kind: OscillatorKind, frequency_hz: f32) -> Self {
        Self {
            kind,
            frequency_hz,
            sample_rate_hz: 48_000.0,
            phase: 0.0,
            pulse_width: 0.5,
            wavetable: Vec::new(),
        }
    }

    /// Builds a sine oscillator at the given frequency.
    pub fn sine(frequency_hz: f32) -> Self {
        Self::new(OscillatorKind::Sine, frequency_hz)
    }

    /// Builds a naive sawtooth oscillator at the given frequency.
    pub fn saw(frequency_hz: f32) -> Self {
        Self::new(OscillatorKind::Saw, frequency_hz)
    }

    /// Builds a naive square oscillator at the given frequency.
    pub fn square(frequency_hz: f32) -> Self {
        Self::new(OscillatorKind::Square, frequency_hz)
    }

    /// Builds a triangle oscillator at the given frequency.
    pub fn triangle(frequency_hz: f32) -> Self {
        Self::new(OscillatorKind::Triangle, frequency_hz)
    }

    /// Builds a band-limited (PolyBLEP) sawtooth oscillator at the given
    /// frequency.
    pub fn polyblep_saw(frequency_hz: f32) -> Self {
        Self::new(OscillatorKind::PolyBlepSaw, frequency_hz)
    }

    /// Builds a band-limited (PolyBLEP) square oscillator at the given
    /// frequency.
    pub fn polyblep_square(frequency_hz: f32) -> Self {
        Self::new(OscillatorKind::PolyBlepSquare, frequency_hz)
    }

    /// Builds a wavetable oscillator at the given frequency over `table`.
    pub fn wavetable(frequency_hz: f32, table: Vec<f32>) -> Self {
        Self {
            kind: OscillatorKind::Wavetable,
            frequency_hz,
            sample_rate_hz: 48_000.0,
            phase: 0.0,
            pulse_width: 0.5,
            wavetable: table,
        }
    }

    /// Returns the current waveform kind.
    pub fn kind(&self) -> OscillatorKind {
        self.kind
    }

    /// Returns the current frequency, in Hz.
    pub fn frequency_hz(&self) -> f32 {
        self.frequency_hz
    }

    /// Returns the current sample rate, in Hz.
    pub fn sample_rate_hz(&self) -> f32 {
        self.sample_rate_hz
    }

    /// Returns the current normalized phase in `[0, 1)`.
    pub fn phase(&self) -> f32 {
        self.phase
    }

    /// Returns the current pulse width used by the square waveforms.
    pub fn pulse_width(&self) -> f32 {
        self.pulse_width
    }

    /// Sets the pulse width for the square waveforms, clamped to `[0.01, 0.99]`.
    pub fn set_pulse_width(&mut self, pulse_width: f32) {
        self.pulse_width = pulse_width.clamp(0.01, 0.99);
    }

    /// Returns the backing wavetable samples.
    pub fn wavetable_samples(&self) -> &[f32] {
        &self.wavetable
    }

    /// Replaces the wavetable, switching the oscillator to
    /// [`OscillatorKind::Wavetable`] if it was not already.
    pub fn set_wavetable(&mut self, table: Vec<f32>) {
        self.wavetable = table;
        if self.kind != OscillatorKind::Wavetable {
            self.kind = OscillatorKind::Wavetable;
        }
    }

    fn advance(&mut self) -> f32 {
        let dt = self.phase_increment();
        self.phase = (self.phase + dt).fract();
        dt
    }

    fn phase_increment(&self) -> f32 {
        (self.frequency_hz / self.sample_rate_hz.max(1.0)).clamp(0.0, 0.5)
    }

    fn current_sample(&self, dt: f32) -> f32 {
        match self.kind {
            OscillatorKind::Sine => (TAU * self.phase).sin(),
            OscillatorKind::Saw => 2.0 * self.phase - 1.0,
            OscillatorKind::Square => {
                if self.phase < self.pulse_width {
                    1.0
                } else {
                    -1.0
                }
            }
            OscillatorKind::Triangle => 1.0 - 4.0 * (self.phase - 0.5).abs(),
            OscillatorKind::PolyBlepSaw => (2.0 * self.phase - 1.0) - poly_blep(self.phase, dt),
            OscillatorKind::PolyBlepSquare => {
                let mut sample = if self.phase < self.pulse_width {
                    1.0
                } else {
                    -1.0
                };
                sample += poly_blep(self.phase, dt);
                let falling = (self.phase - self.pulse_width).rem_euclid(1.0);
                sample -= poly_blep(falling, dt);
                sample.clamp(-1.0, 1.0)
            }
            OscillatorKind::Wavetable => self.wavetable_sample(),
        }
    }

    fn wavetable_sample(&self) -> f32 {
        if self.wavetable.is_empty() {
            return 0.0;
        }
        if self.wavetable.len() == 1 {
            return self.wavetable[0];
        }
        let len = self.wavetable.len();
        let position = self.phase * len as f32;
        let left = position.floor() as usize % len;
        let right = (left + 1) % len;
        let frac = position - position.floor();
        self.wavetable[left] * (1.0 - frac) + self.wavetable[right] * frac
    }
}

impl Oscillator for PhaseOscillator {
    fn reset(&mut self) {
        self.phase = 0.0;
    }

    fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    fn set_frequency(&mut self, frequency_hz: f32) {
        self.frequency_hz = frequency_hz.max(0.0);
    }

    fn next_sample(&mut self) -> f32 {
        let dt = self.phase_increment();
        let sample = self.current_sample(dt);
        self.advance();
        sample
    }
}

fn poly_blep(phase: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    if phase < dt {
        let t = phase / dt;
        t + t - t * t - 1.0
    } else if phase > 1.0 - dt {
        let t = (phase - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}
