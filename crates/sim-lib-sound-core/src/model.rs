use std::ops::Add;
use std::time::Duration;

use sim_lib_pitch_core::Pitch;
use thiserror::Error;

/// Error raised when sound primitives are constructed with invalid values.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum SoundCoreError {
    /// A frequency was zero, negative, or non-finite.
    #[error("frequency must be positive")]
    InvalidFrequency,
    /// An amplitude was negative or non-finite.
    #[error("amplitude must be non-negative")]
    InvalidAmplitude,
    /// An envelope sustain level fell outside the `0.0..=1.0` range.
    #[error("envelope sustain must be between 0.0 and 1.0")]
    InvalidSustain,
    /// A tone duration was zero.
    #[error("tone duration must be positive")]
    InvalidDuration,
    /// A tone was built without any partials.
    #[error("tone must contain at least one partial")]
    EmptyPartials,
    /// A time-stretch factor was zero, negative, or non-finite.
    #[error("time-stretch factor must be positive")]
    InvalidStretch,
}

/// A positive frequency in hertz.
///
/// # Examples
///
/// ```
/// use sim_lib_sound_core::Frequency;
///
/// let a4 = Frequency::new(440.0).unwrap();
/// let a5 = Frequency::new(880.0).unwrap();
/// assert!((a5.cents_above(a4) - 1200.0).abs() < 1e-9);
/// assert!(Frequency::new(0.0).is_err());
/// ```
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Frequency(pub f64);

impl Frequency {
    /// Builds a frequency, returning [`SoundCoreError::InvalidFrequency`] when
    /// `hz` is not a positive, finite value.
    pub fn new(hz: f64) -> Result<Self, SoundCoreError> {
        if hz.is_finite() && hz > 0.0 {
            Ok(Self(hz))
        } else {
            Err(SoundCoreError::InvalidFrequency)
        }
    }

    /// Returns the linear ratio of this frequency to `other`.
    pub fn ratio(self, other: Frequency) -> f64 {
        self.0 / other.0
    }

    /// Returns the interval from `other` to this frequency, measured in cents.
    pub fn cents_above(self, other: Frequency) -> f64 {
        1200.0 * self.ratio(other).log2()
    }

    /// Returns this frequency shifted by `cents` (positive raises, negative
    /// lowers).
    pub fn shift_cents(self, cents: f64) -> Frequency {
        Frequency(self.0 * 2.0_f64.powf(cents / 1200.0))
    }
}

/// A non-negative linear amplitude.
///
/// # Examples
///
/// ```
/// use sim_lib_sound_core::Amplitude;
///
/// let unity = Amplitude::from_db(0.0);
/// assert!((unity.0 - 1.0).abs() < 1e-9);
/// assert!(Amplitude::new(-1.0).is_err());
/// ```
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Amplitude(pub f64);

impl Amplitude {
    /// Builds an amplitude, returning [`SoundCoreError::InvalidAmplitude`] when
    /// `linear` is negative or non-finite.
    pub fn new(linear: f64) -> Result<Self, SoundCoreError> {
        if linear.is_finite() && linear >= 0.0 {
            Ok(Self(linear))
        } else {
            Err(SoundCoreError::InvalidAmplitude)
        }
    }

    /// Builds an amplitude from a decibel value relative to unity gain.
    pub fn from_db(db: f64) -> Self {
        Self(10f64.powf(db / 20.0))
    }

    /// Returns this amplitude expressed in decibels relative to unity gain.
    pub fn to_db(self) -> f64 {
        20.0 * self.0.log10()
    }
}

/// A phase angle in radians.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Phase(pub f64);

impl Phase {
    /// Returns this phase wrapped into the `0.0..TAU` range.
    pub fn normalized(self) -> Self {
        let tau = std::f64::consts::TAU;
        Self(self.0.rem_euclid(tau))
    }
}

/// A single sinusoidal component of a tone.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Partial {
    /// Frequency of the component.
    pub frequency: Frequency,
    /// Linear amplitude of the component.
    pub amplitude: Amplitude,
    /// Starting phase of the component.
    pub phase: Phase,
}

impl Partial {
    /// Builds a validated partial, normalizing the phase and rejecting invalid
    /// frequency or amplitude values.
    pub fn new(
        frequency: Frequency,
        amplitude: Amplitude,
        phase: Phase,
    ) -> Result<Self, SoundCoreError> {
        let _ = Frequency::new(frequency.0)?;
        let _ = Amplitude::new(amplitude.0)?;
        Ok(Self {
            frequency,
            amplitude,
            phase: phase.normalized(),
        })
    }
}

/// The interpolation curve applied across an [`Envelope`].
#[derive(Clone, Debug, PartialEq)]
pub enum EnvelopeShape {
    /// Straight-line segments between envelope stages.
    Linear,
    /// Linear segments raised to the given exponent for a curved response.
    Exponential(f64),
    /// A named custom shape, treated as linear by the built-in sampler.
    Custom(String),
}

/// An attack/decay/sustain/release amplitude envelope.
#[derive(Clone, Debug, PartialEq)]
pub struct Envelope {
    /// Time taken to rise from silence to full level.
    pub attack: Duration,
    /// Time taken to fall from full level to the sustain level.
    pub decay: Duration,
    /// Held level during the sustain phase, in `0.0..=1.0`.
    pub sustain: f64,
    /// Time taken to fall from the sustain level back to silence.
    pub release: Duration,
    /// Interpolation curve applied across the stages.
    pub shape: EnvelopeShape,
}

impl Envelope {
    /// Builds an envelope, returning [`SoundCoreError::InvalidSustain`] when
    /// `sustain` falls outside `0.0..=1.0`.
    pub fn new(
        attack: Duration,
        decay: Duration,
        sustain: f64,
        release: Duration,
        shape: EnvelopeShape,
    ) -> Result<Self, SoundCoreError> {
        if !sustain.is_finite() || !(0.0..=1.0).contains(&sustain) {
            return Err(SoundCoreError::InvalidSustain);
        }
        Ok(Self {
            attack,
            decay,
            sustain,
            release,
            shape,
        })
    }

    /// Returns the envelope level in `0.0..=1.0` at elapsed time `t` for a tone
    /// of length `total`.
    pub fn sample_level(&self, t: Duration, total: Duration) -> f64 {
        let elapsed = t.as_secs_f64();
        let attack = self.attack.as_secs_f64();
        let decay = self.decay.as_secs_f64();
        let release = self.release.as_secs_f64();
        let total_secs = total.as_secs_f64();
        let release_start = (total_secs - release).max(0.0);
        match &self.shape {
            EnvelopeShape::Linear | EnvelopeShape::Custom(_) => {
                if attack > 0.0 && elapsed < attack {
                    elapsed / attack
                } else if decay > 0.0 && elapsed < attack + decay {
                    let progress = (elapsed - attack) / decay;
                    1.0 + (self.sustain - 1.0) * progress
                } else if elapsed < release_start {
                    self.sustain
                } else if release > 0.0 && elapsed <= total_secs {
                    let progress = ((elapsed - release_start) / release).clamp(0.0, 1.0);
                    self.sustain * (1.0 - progress)
                } else {
                    0.0
                }
            }
            EnvelopeShape::Exponential(curve) => {
                let base = self.clone().with_shape(EnvelopeShape::Linear);
                base.sample_level(t, total).powf((*curve).max(0.01))
            }
        }
    }

    fn with_shape(mut self, shape: EnvelopeShape) -> Self {
        self.shape = shape;
        self
    }
}

/// A complete tone: a set of [`Partial`]s shaped by an [`Envelope`] over a
/// fixed duration.
#[derive(Clone, Debug, PartialEq)]
pub struct Tone {
    /// Sinusoidal components that sum to form the tone.
    pub partials: Vec<Partial>,
    /// Amplitude envelope applied across the tone.
    pub envelope: Envelope,
    /// Total sounding length of the tone.
    pub duration: Duration,
}

impl Tone {
    /// Builds a pure sine tone at `frequency` with the default envelope.
    pub fn sine(frequency: Frequency, duration: Duration) -> Self {
        Self::from_partials(
            vec![Partial {
                frequency,
                amplitude: Amplitude(1.0),
                phase: Phase(0.0),
            }],
            default_envelope(),
            duration,
        )
        .expect("sine tone is valid")
    }

    /// Builds a sawtooth tone from `partials` harmonics with `1/n` amplitudes.
    pub fn sawtooth(frequency: Frequency, duration: Duration, partials: usize) -> Self {
        Self::harmonic_series(frequency, duration, partials, |n| 1.0 / n as f64, |_| true)
    }

    /// Builds a square tone from the odd harmonics within `partials`, with
    /// `1/n` amplitudes.
    pub fn square(frequency: Frequency, duration: Duration, partials: usize) -> Self {
        Self::harmonic_series(
            frequency,
            duration,
            partials,
            |n| 1.0 / n as f64,
            |n| n % 2 == 1,
        )
    }

    /// Builds a triangle tone from the odd harmonics within `partials`, with
    /// `1/n^2` amplitudes.
    pub fn triangle(frequency: Frequency, duration: Duration, partials: usize) -> Self {
        Self::harmonic_series(
            frequency,
            duration,
            partials,
            |n| 1.0 / ((n * n) as f64),
            |n| n % 2 == 1,
        )
    }

    /// Builds a tone from explicit partials, rejecting empty partial lists,
    /// zero durations, and invalid component values.
    pub fn from_partials(
        partials: Vec<Partial>,
        envelope: Envelope,
        duration: Duration,
    ) -> Result<Self, SoundCoreError> {
        if duration.is_zero() {
            return Err(SoundCoreError::InvalidDuration);
        }
        if partials.is_empty() {
            return Err(SoundCoreError::EmptyPartials);
        }
        for partial in &partials {
            let _ = Frequency::new(partial.frequency.0)?;
            let _ = Amplitude::new(partial.amplitude.0)?;
        }
        Ok(Self {
            partials,
            envelope,
            duration,
        })
    }

    /// Returns the tone with every partial shifted by `cents`.
    pub fn transpose_cents(mut self, cents: f64) -> Self {
        for partial in &mut self.partials {
            partial.frequency = partial.frequency.shift_cents(cents);
        }
        self
    }

    /// Returns the tone with every partial amplitude scaled by `gain`.
    pub fn amplify(mut self, gain: f64) -> Self {
        for partial in &mut self.partials {
            partial.amplitude = Amplitude(partial.amplitude.0 * gain);
        }
        self
    }

    /// Returns the tone with its duration and envelope stages scaled by
    /// `factor`, rejecting non-positive factors.
    pub fn time_stretch(mut self, factor: f64) -> Result<Self, SoundCoreError> {
        if !factor.is_finite() || factor <= 0.0 {
            return Err(SoundCoreError::InvalidStretch);
        }
        self.duration = Duration::from_secs_f64(self.duration.as_secs_f64() * factor);
        self.envelope.attack = Duration::from_secs_f64(self.envelope.attack.as_secs_f64() * factor);
        self.envelope.decay = Duration::from_secs_f64(self.envelope.decay.as_secs_f64() * factor);
        self.envelope.release =
            Duration::from_secs_f64(self.envelope.release.as_secs_f64() * factor);
        Ok(self)
    }

    fn harmonic_series(
        frequency: Frequency,
        duration: Duration,
        partial_count: usize,
        amp: impl Fn(usize) -> f64,
        include: impl Fn(usize) -> bool,
    ) -> Self {
        let partials = (1..=partial_count.max(1))
            .filter(|n| include(*n))
            .map(|n| Partial {
                frequency: Frequency(frequency.0 * n as f64),
                amplitude: Amplitude(amp(n)),
                phase: Phase(0.0),
            })
            .collect();
        Self::from_partials(partials, default_envelope(), duration)
            .expect("harmonic-series tone is valid")
    }
}

impl Add for Tone {
    type Output = Self;

    fn add(mut self, other: Self) -> Self::Output {
        self.partials.extend(other.partials);
        self.duration = self.duration.max(other.duration);
        self
    }
}

/// Returns a general-purpose default envelope (short attack and decay, high
/// sustain, moderate release, linear shape).
pub fn default_envelope() -> Envelope {
    Envelope::new(
        Duration::from_millis(10),
        Duration::from_millis(50),
        0.8,
        Duration::from_millis(100),
        EnvelopeShape::Linear,
    )
    .expect("default envelope is valid")
}

/// Returns the 12-tone equal-temperament frequency of `pitch`, with A4 (MIDI
/// 69) anchored at 440 Hz.
pub fn equal_temperament_frequency(pitch: Pitch) -> Frequency {
    let semitones = pitch.semitone() - 69;
    Frequency(440.0 * 2.0_f64.powf(semitones as f64 / 12.0))
}
