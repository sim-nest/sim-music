use crate::{
    Dx7Envelope, Dx7EnvelopeGenerator, Dx7EnvelopeSettings, Dx7PatchOperator, midi_key_to_hz,
};

/// How a DX7 operator derives its frequency from coarse/fine tuning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dx7FrequencyMode {
    /// Frequency tracks the played key, multiplied by a coarse/fine ratio.
    Ratio,
    /// Frequency is a fixed absolute value, independent of the played key.
    Fixed,
}

impl Dx7FrequencyMode {
    /// Returns the lowercase string name of this frequency mode (`"ratio"` or
    /// `"fixed"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ratio => "ratio",
            Self::Fixed => "fixed",
        }
    }
}

/// Per-operator pitch tuning: mode plus coarse, fine, and detune offsets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dx7PitchSettings {
    /// Whether the operator runs in ratio (key-tracking) or fixed mode.
    pub mode: Dx7FrequencyMode,
    /// Coarse tuning (0..=31): the integer ratio multiplier or fixed decade.
    pub coarse: u8,
    /// Fine tuning (0..=99): the fractional part added to the coarse value.
    pub fine: u8,
    /// Detune offset in DX7 units (-7..=7), applied as a fraction of a
    /// semitone.
    pub detune: i8,
}

impl Dx7PitchSettings {
    /// Builds pitch settings from a patch operator, selecting the mode from
    /// its oscillator-mode flag and clamping the tuning fields.
    pub fn from_patch_operator(operator: &Dx7PatchOperator) -> Self {
        Self {
            mode: if operator.oscillator_mode == 0 {
                Dx7FrequencyMode::Ratio
            } else {
                Dx7FrequencyMode::Fixed
            },
            coarse: operator.frequency_coarse.min(31),
            fine: operator.frequency_fine.min(99),
            detune: operator.detune.min(14) as i8 - 7,
        }
    }

    /// Computes the operator frequency in Hz for a played `key`, applying
    /// detune and `pitch_offset_semitones` (pitch bend, envelope, and LFO) on
    /// top of the coarse/fine tuning. The result is never negative.
    pub fn frequency_hz(self, key: u8, pitch_offset_semitones: f32) -> f32 {
        let detune = f32::from(self.detune) * 0.1;
        match self.mode {
            Dx7FrequencyMode::Ratio => {
                let ratio = self.coarse.max(1) as f32 + f32::from(self.fine.min(99)) / 100.0;
                midi_key_to_hz(key) * ratio * 2.0_f32.powf((pitch_offset_semitones + detune) / 12.0)
            }
            Dx7FrequencyMode::Fixed => {
                let coarse_hz = 1.0 + f32::from(self.coarse);
                let fine_hz = f32::from(self.fine.min(99)) / 100.0;
                (coarse_hz + fine_hz) * 2.0_f32.powf((pitch_offset_semitones + detune) / 12.0)
            }
        }
        .max(0.0)
    }
}

impl Default for Dx7PitchSettings {
    /// Returns unity tuning: ratio mode at the played pitch (coarse 1) with no
    /// fine tuning or detune.
    fn default() -> Self {
        Self {
            mode: Dx7FrequencyMode::Ratio,
            coarse: 1,
            fine: 0,
            detune: 0,
        }
    }
}

/// Settings for the per-voice pitch envelope: an envelope shape plus a
/// modulation depth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dx7PitchEnvelopeSettings {
    /// The four-rate/four-level envelope driving the pitch sweep.
    pub envelope: Dx7EnvelopeSettings,
    /// Peak pitch deviation in semitones at full envelope swing.
    pub depth_semitones: f32,
}

impl Dx7PitchEnvelopeSettings {
    /// Builds pitch envelope settings from a patch envelope and a non-negative
    /// modulation depth in semitones.
    pub fn from_patch_envelope(envelope: &Dx7Envelope, depth_semitones: f32) -> Self {
        Self {
            envelope: Dx7EnvelopeSettings::new(envelope.rates, envelope.levels),
            depth_semitones: depth_semitones.max(0.0),
        }
    }
}

impl Default for Dx7PitchEnvelopeSettings {
    /// Returns a centered, inactive pitch envelope: a constant mid-level shape
    /// with zero depth, so pitch is unaffected.
    fn default() -> Self {
        Self {
            envelope: Dx7EnvelopeSettings::constant(50),
            depth_semitones: 0.0,
        }
    }
}

/// Stateful pitch envelope that converts an envelope level into a bipolar
/// semitone offset.
#[derive(Clone, Debug, PartialEq)]
pub struct Dx7PitchEnvelope {
    settings: Dx7PitchEnvelopeSettings,
    envelope: Dx7EnvelopeGenerator,
}

impl Dx7PitchEnvelope {
    /// Creates a pitch envelope from the given settings.
    pub fn new(settings: Dx7PitchEnvelopeSettings) -> Self {
        Self {
            envelope: Dx7EnvelopeGenerator::new(settings.envelope),
            settings,
        }
    }

    /// Sets the playback sample rate in Hz for the underlying envelope.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.envelope.set_sample_rate(sample_rate_hz);
    }

    /// Resets the underlying envelope to its idle state.
    pub fn reset(&mut self) {
        self.envelope.reset();
    }

    /// Advances the envelope one sample and returns the pitch offset in
    /// semitones, centered around zero and scaled by the configured depth.
    pub fn next_semitones(&mut self, gate: bool) -> f32 {
        let centered = self.envelope.next_level(gate, 0).to_f32() * 2.0 - 1.0;
        centered * self.settings.depth_semitones
    }
}

impl Default for Dx7PitchEnvelope {
    /// Returns a pitch envelope built from the default settings.
    fn default() -> Self {
        Self::new(Dx7PitchEnvelopeSettings::default())
    }
}
