use sim_kernel::Symbol;
use sim_lib_audio_graph_core::{BlockEvent, PrepareConfig, ProcessBlock, Processor};
use sim_lib_midi_core::{Channel, ChannelMessage, U7};

use crate::{
    ComponentBackend, ComponentBackendSurface, ComponentInspection, ComponentParamDescriptor,
    ComponentParamRange, ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection,
    ComponentPortMedia, ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue,
    DiscreteComponent, Lfo, OscillatorKind, SynthPreset, SynthVoice, VoiceAllocator, VoiceState,
};

/// A polyphonic subtractive synthesizer voiced from a [`SynthPreset`], usable
/// both as an audio-graph [`Processor`] and a [`DiscreteComponent`].
#[derive(Clone, Debug, PartialEq)]
pub struct SubtractiveSynth {
    preset: SynthPreset,
    sample_rate_hz: f32,
    out_channels: usize,
    voices: Vec<SynthVoice>,
    allocator: VoiceAllocator,
    lfo: Lfo,
}

impl SubtractiveSynth {
    /// Creates a synth voiced by the given preset, with at least one voice.
    pub fn new(preset: SynthPreset) -> Self {
        let max_voices = preset.max_voices.max(1);
        Self {
            lfo: Lfo::new(preset.lfo),
            preset,
            sample_rate_hz: 48_000.0,
            out_channels: 2,
            voices: vec![SynthVoice::idle(); max_voices],
            allocator: VoiceAllocator::new(),
        }
    }

    /// Returns the preset voicing this synth.
    pub fn preset(&self) -> &SynthPreset {
        &self.preset
    }

    /// Returns the voice pool.
    pub fn voices(&self) -> &[SynthVoice] {
        &self.voices
    }

    /// Returns the number of voices not currently idle.
    pub fn active_voice_count(&self) -> usize {
        self.voices
            .iter()
            .filter(|voice| voice.state() != VoiceState::Idle)
            .count()
    }

    /// Returns the configured sample rate in hertz.
    pub fn sample_rate_hz(&self) -> f32 {
        self.sample_rate_hz
    }

    /// Returns the number of output channels.
    pub fn out_channels(&self) -> usize {
        self.out_channels
    }

    /// Triggers a note on the given channel, key, and velocity by allocating a
    /// voice.
    pub fn note_on(&mut self, channel: u8, key: u8, velocity: f32) {
        let _ = self.allocator.note_on(
            &mut self.voices,
            channel,
            key,
            velocity,
            &self.preset,
            self.sample_rate_hz,
        );
    }

    /// Releases the voice playing the given channel and key.
    pub fn note_off(&mut self, channel: u8, key: u8) {
        let _ = self.allocator.note_off(&mut self.voices, channel, key);
    }

    fn handle_event(&mut self, event: BlockEvent<'_>) {
        match event {
            BlockEvent::NoteOn {
                channel,
                key,
                velocity,
                ..
            } if velocity > 0.0 => self.note_on(channel, key, velocity),
            BlockEvent::NoteOn { channel, key, .. } | BlockEvent::NoteOff { channel, key, .. } => {
                self.note_off(channel, key);
            }
            BlockEvent::Midi { bytes, len, .. } => {
                if let Some(message) = channel_message_from_short(bytes, len) {
                    self.handle_channel_message(message);
                }
            }
            BlockEvent::MidiLong { .. } | BlockEvent::ParamSet { .. } => {}
        }
    }

    fn handle_channel_message(&mut self, message: ChannelMessage) {
        match message {
            ChannelMessage::NoteOn { ch, key, vel } if vel.0 > 0 => {
                self.note_on(ch.0, key.0, f32::from(vel.0) / 127.0);
            }
            ChannelMessage::NoteOn { ch, key, .. } | ChannelMessage::NoteOff { ch, key, .. } => {
                self.note_off(ch.0, key.0);
            }
            _ => {}
        }
    }

    fn next_mono(&mut self, tempo_bpm: f64) -> f32 {
        self.lfo.set_tempo_bpm(tempo_bpm);
        let lfo = self.lfo.next_sample();
        let mut sum = 0.0;
        for voice in &mut self.voices {
            sum += voice.next_sample(
                &self.preset,
                &self.preset.modulation,
                lfo,
                self.sample_rate_hz,
            );
        }
        sum.clamp(-1.0, 1.0)
    }
}

impl Default for SubtractiveSynth {
    fn default() -> Self {
        Self::new(SynthPreset::default())
    }
}

impl Processor for SubtractiveSynth {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.sample_rate_hz = cfg.sample_rate_hz.max(1) as f32;
        self.out_channels = cfg.out_channels.max(1) as usize;
        self.voices
            .resize(self.preset.max_voices.max(1), SynthVoice::idle());
        for voice in &mut self.voices {
            if voice.state() != VoiceState::Idle {
                voice.reset();
            }
        }
        self.lfo = Lfo::new(self.preset.lfo);
        self.lfo.set_sample_rate(self.sample_rate_hz);
    }

    fn reset(&mut self) {
        for voice in &mut self.voices {
            voice.reset();
        }
        self.allocator.reset();
        self.lfo.reset();
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        let channels = block.out_audio.len().min(self.out_channels);
        for channel in 0..channels {
            block.out_audio[channel][..frames].fill(0.0);
        }
        for frame in 0..frames {
            for event in block.in_events {
                if event_offset(*event) == frame as u32 {
                    self.handle_event(*event);
                }
            }
            let sample = self.next_mono(block.transport.tempo_bpm);
            for channel in 0..channels {
                block.out_audio[channel][frame] = sample;
            }
        }
    }

    fn tail_frames(&self) -> u64 {
        (self.preset.amp_envelope.release_s * self.sample_rate_hz) as u64
    }
}

impl DiscreteComponent for SubtractiveSynth {
    fn component_id(&self) -> Symbol {
        subtractive_synth_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        subtractive_synth_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        subtractive_synth_params()
    }

    fn reset(&mut self) {
        <Self as Processor>::reset(self);
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        <Self as Processor>::prepare(self, config.into());
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        <Self as Processor>::process(self, block);
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            subtractive_synth_component_id(),
            ComponentBackend::Algorithmic,
            self.active_voice_count() > 0,
        )
        .with_field(
            Symbol::qualified("audio-synth/inspect", "active-voices"),
            self.active_voice_count().to_string(),
        )
        .with_field(
            Symbol::qualified("audio-synth/inspect", "sample-rate-hz"),
            self.sample_rate_hz.to_string(),
        )
        .with_field(
            Symbol::qualified("audio-synth/inspect", "out-channels"),
            self.out_channels.to_string(),
        )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        Some(
            ComponentTraceFrame::new(
                subtractive_synth_component_id(),
                ComponentBackend::Algorithmic,
                0,
            )
            .with_integer(
                Symbol::qualified("audio-synth/trace", "active-voices"),
                self.active_voice_count() as i64,
            )
            .with_state(
                Symbol::qualified("audio-synth/trace", "sample-rate-hz"),
                ComponentTraceValue::Float(self.sample_rate_hz as f64),
            )
            .with_clock_position(Symbol::qualified("audio-synth/trace", "clock-position"), 0),
        )
    }
}

/// Returns the component id of the subtractive synth.
pub fn subtractive_synth_component_id() -> Symbol {
    Symbol::qualified("audio-synth", "SubtractiveSynth")
}

/// Returns the port descriptors for the subtractive synth: event, gate, and
/// param inputs, and metadata, audio, and trace outputs.
pub fn subtractive_synth_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "events-in"),
            ComponentPortMedia::Event,
            ComponentPortDirection::Input,
            1,
        ),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "gate-in"),
            ComponentPortMedia::Gate,
            ComponentPortDirection::Input,
            1,
        ),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "params-in"),
            ComponentPortMedia::ControlRate,
            ComponentPortDirection::Input,
            1,
        ),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "metadata-out"),
            ComponentPortMedia::Metadata,
            ComponentPortDirection::Output,
            1,
        )
        .optional(),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "audio-out"),
            ComponentPortMedia::AudioRate,
            ComponentPortDirection::Output,
            2,
        ),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "trace-out"),
            ComponentPortMedia::Trace,
            ComponentPortDirection::Output,
            1,
        )
        .optional(),
    ]
}

/// Returns the parameter descriptors for the subtractive synth: amp gain,
/// filter cutoff, oscillator, max voices, and pulse width.
pub fn subtractive_synth_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "amp-gain"),
            "Amplitude gain",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 1.0, 0.25)),
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "filter-cutoff-hz"),
            "Filter cutoff",
            ComponentParamUnit::Hertz,
        )
        .with_range(ComponentParamRange::new(20.0, 20_000.0, 8_000.0)),
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "oscillator"),
            "Oscillator",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(
            [
                OscillatorKind::Sine,
                OscillatorKind::Saw,
                OscillatorKind::Square,
                OscillatorKind::Triangle,
                OscillatorKind::PolyBlepSaw,
                OscillatorKind::PolyBlepSquare,
                OscillatorKind::Wavetable,
            ]
            .into_iter()
            .map(|kind| Symbol::qualified("audio-synth", kind.as_str()))
            .collect(),
            4,
        ),
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "max-voices"),
            "Max voices",
            ComponentParamUnit::RawInteger,
        )
        .with_range(ComponentParamRange::new(1.0, 64.0, 8.0))
        .with_raw_default(8),
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "pulse-width"),
            "Pulse width",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.01, 0.99, 0.5)),
    ]
}

/// Returns the algorithmic and modeled backend surfaces for the subtractive
/// synth, which must expose identical ports and params.
pub fn subtractive_synth_backend_surfaces() -> [ComponentBackendSurface; 2] {
    [
        subtractive_synth_backend_surface(ComponentBackend::Algorithmic),
        subtractive_synth_backend_surface(ComponentBackend::Modeled),
    ]
}

fn subtractive_synth_backend_surface(backend: ComponentBackend) -> ComponentBackendSurface {
    ComponentBackendSurface::new(
        backend,
        subtractive_synth_ports(),
        subtractive_synth_params(),
    )
}

fn event_offset(event: BlockEvent<'_>) -> u32 {
    match event {
        BlockEvent::Midi { offset, .. }
        | BlockEvent::MidiLong { offset, .. }
        | BlockEvent::ParamSet { offset, .. }
        | BlockEvent::NoteOn { offset, .. }
        | BlockEvent::NoteOff { offset, .. } => offset,
    }
}

fn channel_message_from_short(bytes: [u8; 3], len: u8) -> Option<ChannelMessage> {
    if len < 3 {
        return None;
    }
    let status = bytes[0];
    let channel = Channel::new(status & 0x0f).ok()?;
    let first = U7::try_from(u16::from(bytes[1])).ok()?;
    let second = U7::try_from(u16::from(bytes[2])).ok()?;
    match status & 0xf0 {
        0x80 => Some(ChannelMessage::NoteOff {
            ch: channel,
            key: first,
            vel: second,
        }),
        0x90 => Some(ChannelMessage::NoteOn {
            ch: channel,
            key: first,
            vel: second,
        }),
        _ => None,
    }
}
