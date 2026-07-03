//! Attack/decay/sustain/release amplitude envelope.
//!
//! Defines the [`AdsrSettings`] parameter block, the [`AdsrStage`] state
//! machine, and [`AdsrEnvelope`], a sample-by-sample linear ADSR generator
//! gated by note-on and note-off events.

/// Tunable ADSR envelope parameters, in seconds and normalized level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdsrSettings {
    /// Attack time, in seconds, to ramp from 0 to full level.
    pub attack_s: f32,
    /// Decay time, in seconds, to fall from full level to the sustain level.
    pub decay_s: f32,
    /// Held level after decay, in `[0, 1]`.
    pub sustain_level: f32,
    /// Release time, in seconds, to fall from the current level to 0 after
    /// note-off.
    pub release_s: f32,
}

impl Default for AdsrSettings {
    fn default() -> Self {
        Self {
            attack_s: 0.005,
            decay_s: 0.08,
            sustain_level: 0.75,
            release_s: 0.12,
        }
    }
}

/// Current phase of an [`AdsrEnvelope`] state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdsrStage {
    /// Silent and waiting for a note-on.
    Idle,
    /// Ramping up toward full level.
    Attack,
    /// Falling from full level toward the sustain level.
    Decay,
    /// Holding at the sustain level until note-off.
    Sustain,
    /// Falling from the current level toward zero after note-off.
    Release,
}

/// A linear ADSR amplitude envelope generator driven one sample at a time.
#[derive(Clone, Debug, PartialEq)]
pub struct AdsrEnvelope {
    settings: AdsrSettings,
    sample_rate_hz: f32,
    stage: AdsrStage,
    value: f32,
    release_step: f32,
}

impl AdsrEnvelope {
    /// Builds an envelope from sanitized `settings` at the default 48 kHz sample
    /// rate, starting [`AdsrStage::Idle`].
    pub fn new(settings: AdsrSettings) -> Self {
        Self {
            settings: sanitize(settings),
            sample_rate_hz: 48_000.0,
            stage: AdsrStage::Idle,
            value: 0.0,
            release_step: 0.0,
        }
    }

    /// Returns the current (sanitized) envelope settings.
    pub fn settings(&self) -> AdsrSettings {
        self.settings
    }

    /// Returns the current stage of the envelope.
    pub fn stage(&self) -> AdsrStage {
        self.stage
    }

    /// Returns the current envelope output level.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Sets the sample rate, in Hz, used to convert times into per-sample steps
    /// (clamped to at least 1).
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Replaces and sanitizes the envelope settings.
    pub fn set_settings(&mut self, settings: AdsrSettings) {
        self.settings = sanitize(settings);
    }

    /// Returns the envelope to [`AdsrStage::Idle`] with zero level.
    pub fn reset(&mut self) {
        self.stage = AdsrStage::Idle;
        self.value = 0.0;
        self.release_step = 0.0;
    }

    /// Triggers the envelope; jumps straight to decay at full level when attack
    /// is zero, otherwise enters [`AdsrStage::Attack`].
    pub fn note_on(&mut self) {
        if self.settings.attack_s <= 0.0 {
            self.value = 1.0;
            self.stage = AdsrStage::Decay;
        } else {
            self.stage = AdsrStage::Attack;
        }
    }

    /// Releases the note; resets immediately when already silent or release is
    /// zero, otherwise enters [`AdsrStage::Release`] with a computed decay step.
    pub fn note_off(&mut self) {
        if self.value <= 0.0 || self.settings.release_s <= 0.0 {
            self.reset();
        } else {
            self.release_step = self.value / (self.settings.release_s * self.sample_rate_hz);
            self.stage = AdsrStage::Release;
        }
    }

    /// Returns whether the envelope is idle (fully released or never triggered).
    pub fn is_idle(&self) -> bool {
        self.stage == AdsrStage::Idle
    }

    /// Advances the state machine by one sample and returns the level, clamped
    /// to `[0, 1]`.
    pub fn next_sample(&mut self) -> f32 {
        match self.stage {
            AdsrStage::Idle => {
                self.value = 0.0;
            }
            AdsrStage::Attack => {
                self.value += 1.0 / (self.settings.attack_s * self.sample_rate_hz);
                if self.value >= 1.0 {
                    self.value = 1.0;
                    self.stage = AdsrStage::Decay;
                }
            }
            AdsrStage::Decay => {
                if self.settings.decay_s <= 0.0 {
                    self.value = self.settings.sustain_level;
                    self.stage = AdsrStage::Sustain;
                } else {
                    let step = (1.0 - self.settings.sustain_level)
                        / (self.settings.decay_s * self.sample_rate_hz);
                    self.value -= step;
                    if self.value <= self.settings.sustain_level {
                        self.value = self.settings.sustain_level;
                        self.stage = AdsrStage::Sustain;
                    }
                }
            }
            AdsrStage::Sustain => {
                self.value = self.settings.sustain_level;
            }
            AdsrStage::Release => {
                self.value -= self.release_step;
                if self.value <= 0.0 {
                    self.reset();
                }
            }
        }
        self.value.clamp(0.0, 1.0)
    }
}

impl Default for AdsrEnvelope {
    fn default() -> Self {
        Self::new(AdsrSettings::default())
    }
}

fn sanitize(settings: AdsrSettings) -> AdsrSettings {
    AdsrSettings {
        attack_s: settings.attack_s.max(0.0),
        decay_s: settings.decay_s.max(0.0),
        sustain_level: settings.sustain_level.clamp(0.0, 1.0),
        release_s: settings.release_s.max(0.0),
    }
}
