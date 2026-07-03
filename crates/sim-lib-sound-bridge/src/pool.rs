use std::time::Duration;

use sim_lib_sound_core::{Frequency, Tone};
use sim_lib_sound_timbre::Timbre;

use crate::{SoundBridgeError, TimbreBank};

/// A tone scheduled to start at a given time, with stereo placement and its
/// originating MIDI channel and key.
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduledTone {
    /// Start time of the tone relative to the bridge clock.
    pub start: Duration,
    /// The rendered tone.
    pub tone: Tone,
    /// Stereo pan in `-1.0..=1.0` (left to right).
    pub pan: f32,
    /// Originating MIDI channel.
    pub channel: u8,
    /// Originating MIDI note key.
    pub key: u8,
}

/// The lifecycle phase of a voice in the pool.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VoicePhase {
    /// Key is held down.
    Held,
    /// Key released but held by the sustain pedal.
    SustainHeld,
    /// Voice is in its release stage.
    Released,
}

/// An allocated voice sounding a single note.
#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    /// MIDI channel that triggered the voice.
    pub channel: u8,
    /// MIDI note key.
    pub key: u8,
    /// Time the voice started.
    pub started_at: Duration,
    /// Time the voice entered its release stage, if any.
    pub release_at: Option<Duration>,
    /// Time the release stage completes, if known.
    pub release_end: Option<Duration>,
    /// Sounding frequency, including any pitch bend or tuning offset.
    pub frequency: Frequency,
    /// Amplitude gain applied to the voice.
    pub amplitude_gain: f64,
    /// Stereo pan in `-1.0..=1.0`.
    pub pan: f32,
    /// Current lifecycle phase.
    pub phase: VoicePhase,
    /// Timbre used to render the voice.
    pub timbre: Timbre,
}

impl Voice {
    fn to_scheduled_tone(&self) -> ScheduledTone {
        let released_at = self.release_at.unwrap_or(self.started_at);
        let total_duration =
            released_at.saturating_sub(self.started_at) + self.timbre.default_envelope.release;
        let tone = self
            .timbre
            .render(self.frequency, total_duration)
            .amplify(self.amplitude_gain);
        ScheduledTone {
            start: self.started_at,
            tone,
            pan: self.pan,
            channel: self.channel,
            key: self.key,
        }
    }
}

/// A polyphony-limited pool of voices that emits scheduled tones as voices
/// finish or are stolen.
#[derive(Clone, Debug, PartialEq)]
pub struct VoicePool {
    polyphony_limit: usize,
    voices: Vec<Voice>,
    emitted: Vec<ScheduledTone>,
    stolen_voice_count: usize,
}

impl VoicePool {
    /// Builds a pool with the given polyphony limit, rejecting a limit of zero.
    pub fn new(polyphony_limit: usize) -> Result<Self, SoundBridgeError> {
        if polyphony_limit == 0 {
            return Err(SoundBridgeError::ZeroPolyphony);
        }
        Ok(Self {
            polyphony_limit,
            voices: Vec::new(),
            emitted: Vec::new(),
            stolen_voice_count: 0,
        })
    }

    /// Returns the maximum number of simultaneous voices.
    pub fn polyphony_limit(&self) -> usize {
        self.polyphony_limit
    }

    /// Returns the number of currently active voices.
    pub fn active_voice_count(&self) -> usize {
        self.voices.len()
    }

    /// Returns the currently active voices.
    pub fn voices(&self) -> &[Voice] {
        &self.voices
    }

    /// Returns the scheduled tones emitted but not yet drained.
    pub fn emitted(&self) -> &[ScheduledTone] {
        &self.emitted
    }

    /// Returns the running count of stolen voices.
    pub fn stolen_voice_count(&self) -> usize {
        self.stolen_voice_count
    }

    /// Drains and returns the emitted scheduled tones, clearing the buffer.
    pub fn drain_emitted(&mut self) -> Vec<ScheduledTone> {
        std::mem::take(&mut self.emitted)
    }

    /// Starts a new voice at `now`, retiring any same-key voice and stealing a
    /// voice if the polyphony limit is reached.
    pub fn note_on(&mut self, mut voice: Voice, now: Duration) {
        self.reap_finished(now);
        self.note_off(voice.channel, voice.key, now, false);
        if self.voices.len() >= self.polyphony_limit {
            self.steal_voice(now);
        }
        voice.phase = VoicePhase::Held;
        voice.release_at = None;
        voice.release_end = None;
        self.voices.push(voice);
    }

    /// Releases the most recent voice matching `channel`/`key`, holding it
    /// under the sustain pedal when `sustain` is set.
    pub fn note_off(&mut self, channel: u8, key: u8, now: Duration, sustain: bool) {
        self.reap_finished(now);
        if let Some(voice) = self
            .voices
            .iter_mut()
            .rev()
            .find(|voice| voice.channel == channel && voice.key == key)
        {
            if sustain {
                voice.phase = VoicePhase::SustainHeld;
            } else {
                voice.phase = VoicePhase::Released;
                voice.release_at = Some(now);
                voice.release_end = Some(now + voice.timbre.default_envelope.release);
            }
        }
    }

    /// Releases all sustain-held voices on `channel` at `now`.
    pub fn release_channel_sustain(&mut self, channel: u8, now: Duration) {
        self.reap_finished(now);
        for voice in &mut self.voices {
            if voice.channel == channel && voice.phase == VoicePhase::SustainHeld {
                voice.phase = VoicePhase::Released;
                voice.release_at = Some(now);
                voice.release_end = Some(now + voice.timbre.default_envelope.release);
            }
        }
    }

    /// Shifts the frequency of all non-released voices on `channel` by `cents`.
    pub fn transpose_channel_cents(&mut self, channel: u8, cents: f64) {
        for voice in &mut self.voices {
            if voice.channel == channel && voice.phase != VoicePhase::Released {
                voice.frequency = voice.frequency.shift_cents(cents);
            }
        }
    }

    /// Emits and removes voices whose release stage has completed by `now`.
    pub fn reap_finished(&mut self, now: Duration) {
        let mut keep = Vec::with_capacity(self.voices.len());
        for voice in self.voices.drain(..) {
            if voice.release_end.is_some_and(|release_end| {
                release_end <= now && voice.phase == VoicePhase::Released
            }) {
                self.emitted.push(voice.to_scheduled_tone());
            } else {
                keep.push(voice);
            }
        }
        self.voices = keep;
    }

    /// Releases every remaining voice and emits all of them.
    pub fn flush_all(&mut self, now: Duration) {
        for voice in &mut self.voices {
            if voice.phase != VoicePhase::Released {
                voice.phase = VoicePhase::Released;
                voice.release_at = Some(now);
                voice.release_end = Some(now + voice.timbre.default_envelope.release);
            }
        }
        let max_end = self
            .voices
            .iter()
            .filter_map(|voice| voice.release_end)
            .max()
            .unwrap_or(now);
        self.reap_finished(max_end);
    }

    fn steal_voice(&mut self, now: Duration) {
        let release_candidate = self
            .voices
            .iter()
            .enumerate()
            .filter(|(_, voice)| voice.phase == VoicePhase::Released)
            .min_by_key(|(_, voice)| voice.release_at.unwrap_or(voice.started_at))
            .map(|(index, _)| index);
        let active_candidate = self
            .voices
            .iter()
            .enumerate()
            .min_by_key(|(_, voice)| voice.started_at)
            .map(|(index, _)| index);
        let index = release_candidate.or(active_candidate).unwrap_or(0);
        let mut stolen = self.voices.swap_remove(index);
        if stolen.release_at.is_none() {
            stolen.release_at = Some(now);
            stolen.release_end = Some(now);
        }
        self.stolen_voice_count += 1;
        self.emitted.push(stolen.to_scheduled_tone());
    }
}

/// Per-channel controller state tracked by the bridge.
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeChannelState {
    /// Bank-select MSB.
    pub bank_msb: u8,
    /// Bank-select LSB.
    pub bank_lsb: u8,
    /// Selected program number.
    pub program: u8,
    /// Channel volume in `0.0..=1.0`.
    pub volume: f64,
    /// Expression level in `0.0..=1.0`.
    pub expression: f64,
    /// Stereo pan in `-1.0..=1.0`.
    pub pan: f32,
    /// Whether the sustain pedal is held.
    pub sustain: bool,
    /// Current pitch-bend offset, in cents.
    pub pitch_bend_cents: f64,
}

impl Default for BridgeChannelState {
    fn default() -> Self {
        Self {
            bank_msb: 0,
            bank_lsb: 0,
            program: 0,
            volume: 1.0,
            expression: 1.0,
            pan: 0.0,
            sustain: false,
            pitch_bend_cents: 0.0,
        }
    }
}

/// Configuration for a [`MidiToSoundBridge`](crate::MidiToSoundBridge).
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeOptions {
    /// Maximum number of simultaneous voices.
    pub polyphony_limit: usize,
    /// Full-scale pitch-bend range, in cents.
    pub bend_range_cents: f64,
}

impl Default for BridgeOptions {
    fn default() -> Self {
        Self {
            polyphony_limit: 32,
            bend_range_cents: 200.0,
        }
    }
}

impl BridgeOptions {
    /// Builds bridge options, rejecting a zero polyphony limit.
    pub fn new(polyphony_limit: usize, bend_range_cents: f64) -> Result<Self, SoundBridgeError> {
        if polyphony_limit == 0 {
            return Err(SoundBridgeError::ZeroPolyphony);
        }
        Ok(Self {
            polyphony_limit,
            bend_range_cents,
        })
    }
}

pub(crate) fn tone_gain(channel: &BridgeChannelState, velocity: u8) -> f64 {
    let velocity_gain = f64::from(velocity) / 127.0;
    velocity_gain * channel.volume * channel.expression
}

pub(crate) fn current_timbre<'a>(bank: &'a TimbreBank, channel: &BridgeChannelState) -> &'a Timbre {
    bank.get(channel.bank_msb, channel.bank_lsb, channel.program)
}
