//! Polyphonic synth voices and voice allocation.
//!
//! Defines [`VoiceState`], a single [`SynthVoice`] (oscillator plus ADSR
//! envelope plus one-pole low-pass filter driven by a [`ModulationMatrix`]),
//! the [`VoiceAllocator`] that assigns notes to voices, and the
//! [`midi_key_to_hz`] equal-temperament pitch conversion.

use crate::{
    AdsrEnvelope, AdsrSettings, ModulationInput, ModulationMatrix, Oscillator, OscillatorKind,
    PhaseOscillator, SynthPreset,
};

/// Lifecycle state of a [`SynthVoice`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceState {
    /// Unallocated and silent.
    Idle,
    /// Sounding with the gate held (note-on).
    Active,
    /// Note-off received; tailing off through the envelope release.
    Released,
}

/// A single synthesizer voice: one oscillator, ADSR envelope, and low-pass
/// filter for one held note.
#[derive(Clone, Debug, PartialEq)]
pub struct SynthVoice {
    state: VoiceState,
    channel: u8,
    key: u8,
    velocity: f32,
    age: u64,
    base_frequency_hz: f32,
    oscillator: PhaseOscillator,
    envelope: AdsrEnvelope,
    filter_z: f32,
}

impl SynthVoice {
    /// Builds an idle, silent voice with default oscillator and envelope.
    pub fn idle() -> Self {
        Self {
            state: VoiceState::Idle,
            channel: 0,
            key: 0,
            velocity: 0.0,
            age: 0,
            base_frequency_hz: 0.0,
            oscillator: PhaseOscillator::sine(0.0),
            envelope: AdsrEnvelope::default(),
            filter_z: 0.0,
        }
    }

    /// Returns the voice's current lifecycle state.
    pub fn state(&self) -> VoiceState {
        self.state
    }

    /// Returns the MIDI channel the voice is sounding on.
    pub fn channel(&self) -> u8 {
        self.channel
    }

    /// Returns the MIDI key number the voice is playing.
    pub fn key(&self) -> u8 {
        self.key
    }

    /// Returns the note velocity in `[0, 1]`.
    pub fn velocity(&self) -> f32 {
        self.velocity
    }

    /// Returns the allocation age stamp (used for voice stealing).
    pub fn age(&self) -> u64 {
        self.age
    }

    /// Starts the voice on a note: sets channel, key, velocity, and age,
    /// converts the key to its base frequency, and builds the oscillator and
    /// envelope from `preset` at the given sample rate.
    pub fn start(
        &mut self,
        channel: u8,
        key: u8,
        velocity: f32,
        age: u64,
        preset: &SynthPreset,
        sample_rate_hz: f32,
    ) {
        self.state = VoiceState::Active;
        self.channel = channel;
        self.key = key;
        self.velocity = velocity.clamp(0.0, 1.0);
        self.age = age;
        self.base_frequency_hz = midi_key_to_hz(key);
        self.oscillator = oscillator_from_preset(preset, self.base_frequency_hz);
        self.oscillator.set_sample_rate(sample_rate_hz);
        self.oscillator.set_pulse_width(preset.pulse_width);
        self.envelope = AdsrEnvelope::new(preset.amp_envelope);
        self.envelope.set_sample_rate(sample_rate_hz);
        self.envelope.note_on();
        self.filter_z = 0.0;
    }

    /// Transitions an active voice to [`VoiceState::Released`] and starts the
    /// envelope release.
    pub fn release(&mut self) {
        if self.state == VoiceState::Active {
            self.state = VoiceState::Released;
            self.envelope.note_off();
        }
    }

    /// Returns the voice to the idle state, clearing all per-note state.
    pub fn reset(&mut self) {
        *self = Self::idle();
    }

    /// Renders the next sample: advances the envelope, applies the
    /// `modulation` matrix (driven by `lfo1`, the envelope, and velocity) to
    /// pitch, pulse width, filter cutoff, and gain, and returns the filtered,
    /// enveloped, velocity- and gain-scaled output. Returns 0 and resets when
    /// the envelope has run out.
    pub fn next_sample(
        &mut self,
        preset: &SynthPreset,
        modulation: &ModulationMatrix,
        lfo1: f32,
        sample_rate_hz: f32,
    ) -> f32 {
        if self.state == VoiceState::Idle {
            return 0.0;
        }
        let envelope_value = self.envelope.next_sample();
        if self.envelope.is_idle() {
            self.reset();
            return 0.0;
        }
        let mods = modulation.apply(ModulationInput {
            lfo1,
            envelope1: envelope_value,
            velocity: self.velocity,
            constant: 1.0,
        });
        let frequency = self.base_frequency_hz * 2.0_f32.powf(mods.osc_pitch_semitones / 12.0);
        self.oscillator.set_frequency(frequency);
        self.oscillator
            .set_pulse_width((preset.pulse_width + mods.pulse_width).clamp(0.01, 0.99));
        let raw = self.oscillator.next_sample();
        let cutoff = (preset.filter_cutoff_hz + mods.filter_cutoff_hz).clamp(20.0, 20_000.0);
        let filtered = self.low_pass(raw, cutoff, sample_rate_hz);
        let gain = (preset.amp_gain + mods.amp_gain).max(0.0);
        filtered * envelope_value * self.velocity * gain
    }

    fn low_pass(&mut self, input: f32, cutoff_hz: f32, sample_rate_hz: f32) -> f32 {
        let alpha = 1.0 - (-std::f32::consts::TAU * cutoff_hz / sample_rate_hz.max(1.0)).exp();
        self.filter_z += alpha.clamp(0.0, 1.0) * (input - self.filter_z);
        self.filter_z
    }
}

/// Assigns incoming notes to a pool of [`SynthVoice`]s, tracking an increasing
/// age stamp for voice stealing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceAllocator {
    next_age: u64,
}

impl VoiceAllocator {
    /// Builds an allocator with a fresh age counter.
    pub fn new() -> Self {
        Self { next_age: 1 }
    }

    /// Allocates a voice for a note-on, preferring idle then released then
    /// oldest voices; starts the chosen voice and returns its index, or `None`
    /// if the pool is empty.
    pub fn note_on(
        &mut self,
        voices: &mut [SynthVoice],
        channel: u8,
        key: u8,
        velocity: f32,
        preset: &SynthPreset,
        sample_rate_hz: f32,
    ) -> Option<usize> {
        let index = pick_voice(voices)?;
        let age = self.next_age;
        self.next_age = self.next_age.wrapping_add(1).max(1);
        voices[index].start(channel, key, velocity, age, preset, sample_rate_hz);
        Some(index)
    }

    /// Releases the active voice matching `channel` and `key`, returning its
    /// index, or `None` if no such voice is sounding.
    pub fn note_off(&mut self, voices: &mut [SynthVoice], channel: u8, key: u8) -> Option<usize> {
        let index = voices.iter().position(|voice| {
            voice.state == VoiceState::Active && voice.channel == channel && voice.key == key
        })?;
        voices[index].release();
        Some(index)
    }

    /// Resets the allocator's age counter.
    pub fn reset(&mut self) {
        self.next_age = 1;
    }
}

impl Default for VoiceAllocator {
    fn default() -> Self {
        Self::new()
    }
}

/// Converts a MIDI key number to its frequency in Hz using A4 = 440 Hz
/// equal temperament.
pub fn midi_key_to_hz(key: u8) -> f32 {
    let semitone = sim_lib_pitch_core::Pitch::from_midi(key).semitone() as f32;
    440.0 * 2.0_f32.powf((semitone - 69.0) / 12.0)
}

fn oscillator_from_preset(preset: &SynthPreset, frequency_hz: f32) -> PhaseOscillator {
    match preset.oscillator {
        OscillatorKind::Sine => PhaseOscillator::sine(frequency_hz),
        OscillatorKind::Saw => PhaseOscillator::saw(frequency_hz),
        OscillatorKind::Square => PhaseOscillator::square(frequency_hz),
        OscillatorKind::Triangle => PhaseOscillator::triangle(frequency_hz),
        OscillatorKind::PolyBlepSaw => PhaseOscillator::polyblep_saw(frequency_hz),
        OscillatorKind::PolyBlepSquare => PhaseOscillator::polyblep_square(frequency_hz),
        OscillatorKind::Wavetable => {
            PhaseOscillator::wavetable(frequency_hz, preset.wavetable.clone())
        }
    }
}

fn pick_voice(voices: &[SynthVoice]) -> Option<usize> {
    if let Some(index) = voices
        .iter()
        .position(|voice| voice.state == VoiceState::Idle)
    {
        return Some(index);
    }
    voices
        .iter()
        .enumerate()
        .filter(|(_, voice)| voice.state == VoiceState::Released)
        .min_by_key(|(_, voice)| voice.age)
        .map(|(index, _)| index)
        .or_else(|| {
            voices
                .iter()
                .enumerate()
                .min_by_key(|(_, voice)| voice.age)
                .map(|(index, _)| index)
        })
}

impl Default for SynthVoice {
    fn default() -> Self {
        Self::idle()
    }
}

#[allow(dead_code)]
fn _assert_settings_copy(_: AdsrSettings) {}
