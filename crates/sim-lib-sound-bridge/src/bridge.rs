use std::time::Duration;

use sim_lib_midi_core::{
    CC_ALL_NOTES_OFF, CC_BANK_SELECT_MSB, CC_EXPRESSION_MSB, CC_PAN_MSB, CC_SUSTAIN_PEDAL,
    CC_VOLUME_MSB, ChannelMessage, MetaEvent, MidiEvent, MidiPayload, MidiSink, TickTime, U14,
};
use sim_lib_pitch_core::Pitch;
use sim_lib_sound_tuning::{Tuning, render_pitch_with_tuning};

use crate::{
    BridgeChannelState, BridgeOptions, ScheduledTone, SoundBridgeError, TimbreBank, Voice,
    VoicePhase, VoicePool, current_timbre, tone_gain,
};

/// A [`MidiSink`] that interprets MIDI events into scheduled tones, managing
/// per-channel state, tuning, voice allocation, and timing.
pub struct MidiToSoundBridge {
    tpq: u32,
    options: BridgeOptions,
    bank: TimbreBank,
    tuning: Box<dyn Tuning>,
    clock: Duration,
    last_tick: TickTime,
    us_per_quarter: u32,
    channels: [BridgeChannelState; 16],
    voices: VoicePool,
    emitted: Vec<ScheduledTone>,
}

impl MidiToSoundBridge {
    /// Builds a bridge with the given ticks-per-quarter, timbre bank, tuning,
    /// and options.
    pub fn new(
        tpq: u32,
        bank: TimbreBank,
        tuning: Box<dyn Tuning>,
        options: BridgeOptions,
    ) -> Result<Self, SoundBridgeError> {
        Ok(Self {
            tpq,
            voices: VoicePool::new(options.polyphony_limit)?,
            options,
            bank,
            tuning,
            clock: Duration::ZERO,
            last_tick: TickTime::new(0, tpq).expect("validated tpq"),
            us_per_quarter: 500_000,
            channels: std::array::from_fn(|_| BridgeChannelState::default()),
            emitted: Vec::new(),
        })
    }

    /// Returns the bridge options.
    pub fn options(&self) -> &BridgeOptions {
        &self.options
    }

    /// Returns the timbre bank.
    pub fn bank(&self) -> &TimbreBank {
        &self.bank
    }

    /// Returns the underlying voice pool.
    pub fn voice_pool(&self) -> &VoicePool {
        &self.voices
    }

    /// Drains all tones emitted so far, returning them and clearing the buffer.
    pub fn drain_tones(&mut self) -> Vec<ScheduledTone> {
        self.drain_pending();
        std::mem::take(&mut self.emitted)
    }

    /// Returns the number of voices stolen due to polyphony limits.
    pub fn stolen_voice_count(&self) -> usize {
        self.voices.stolen_voice_count()
    }

    fn advance_to(&mut self, time: TickTime) -> Result<(), SoundBridgeError> {
        let rebased = if time.tpq == self.tpq {
            time
        } else {
            time.quantize(self.tpq)
        };
        if rebased.ticks < self.last_tick.ticks {
            return Err(SoundBridgeError::NonMonotonicTime);
        }
        let delta_ticks = rebased.ticks - self.last_tick.ticks;
        let delta_quarters = delta_ticks as f64 / f64::from(self.tpq);
        let delta_secs = delta_quarters * f64::from(self.us_per_quarter) / 1_000_000.0;
        self.clock += Duration::from_secs_f64(delta_secs.max(0.0));
        self.last_tick = rebased;
        self.voices.reap_finished(self.clock);
        self.drain_pending();
        Ok(())
    }

    fn handle_channel_message(&mut self, message: &ChannelMessage) {
        match *message {
            ChannelMessage::NoteOn { ch, key, vel } if vel.0 == 0 => {
                let sustain = self.channels[ch.0 as usize].sustain;
                self.voices.note_off(ch.0, key.0, self.clock, sustain);
            }
            ChannelMessage::NoteOn { ch, key, vel } => {
                let state = self.channels[ch.0 as usize].clone();
                let pitch = Pitch::from_midi(key.0);
                let frequency = render_pitch_with_tuning(pitch, self.tuning.as_ref());
                let frequency = frequency.shift_cents(state.pitch_bend_cents);
                let voice = Voice {
                    channel: ch.0,
                    key: key.0,
                    started_at: self.clock,
                    release_at: None,
                    release_end: None,
                    frequency,
                    amplitude_gain: tone_gain(&state, vel.0),
                    pan: state.pan,
                    phase: VoicePhase::Held,
                    timbre: current_timbre(&self.bank, &state).clone(),
                };
                self.voices.note_on(voice, self.clock);
            }
            ChannelMessage::NoteOff { ch, key, .. } => {
                let sustain = self.channels[ch.0 as usize].sustain;
                self.voices.note_off(ch.0, key.0, self.clock, sustain);
            }
            ChannelMessage::ControlChange { ch, cc, value } => {
                let state = &mut self.channels[ch.0 as usize];
                match cc {
                    CC_BANK_SELECT_MSB => state.bank_msb = value.0,
                    CC_VOLUME_MSB => state.volume = f64::from(value.0) / 127.0,
                    CC_PAN_MSB => state.pan = (f32::from(value.0) - 64.0) / 63.0,
                    CC_EXPRESSION_MSB => state.expression = f64::from(value.0) / 127.0,
                    CC_SUSTAIN_PEDAL => {
                        let sustain = value.0 >= 64;
                        if state.sustain && !sustain {
                            self.voices.release_channel_sustain(ch.0, self.clock);
                        }
                        state.sustain = sustain;
                    }
                    CC_ALL_NOTES_OFF => {
                        let sustain = state.sustain;
                        for key in 0..=127 {
                            self.voices.note_off(ch.0, key, self.clock, sustain);
                        }
                    }
                    _ => {}
                }
            }
            ChannelMessage::ProgramChange { ch, program } => {
                self.channels[ch.0 as usize].program = program.0;
            }
            ChannelMessage::PitchBend { ch, value } => {
                let cents = pitch_bend_to_cents(value, self.options.bend_range_cents);
                let state = &mut self.channels[ch.0 as usize];
                let delta = cents - state.pitch_bend_cents;
                state.pitch_bend_cents = cents;
                self.voices.transpose_channel_cents(ch.0, delta);
            }
            ChannelMessage::PolyAftertouch { .. } | ChannelMessage::ChanAftertouch { .. } => {}
        }
    }

    fn drain_pending(&mut self) {
        self.emitted.extend(self.voices.drain_emitted());
    }
}

impl MidiSink for MidiToSoundBridge {
    type Err = SoundBridgeError;

    fn tpq(&self) -> u32 {
        self.tpq
    }

    fn write(&mut self, event: &MidiEvent) -> Result<(), Self::Err> {
        self.advance_to(event.time)?;
        match &event.payload {
            MidiPayload::Channel(message) => self.handle_channel_message(message),
            MidiPayload::Meta(MetaEvent::Tempo { us_per_quarter }) => {
                self.us_per_quarter = *us_per_quarter;
            }
            MidiPayload::Meta(_) | MidiPayload::SysEx(_) | MidiPayload::Raw(_) => {}
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Err> {
        self.voices.flush_all(self.clock);
        self.drain_pending();
        Ok(())
    }
}

fn pitch_bend_to_cents(value: U14, bend_range_cents: f64) -> f64 {
    let normalized = (f64::from(value.0) - 8192.0) / 8192.0;
    normalized * bend_range_cents
}
