use sim_lib_audio_graph_core::{
    BlockArena, BlockEvent, NullEventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};

use crate::{AdsrSettings, OscillatorKind, SubtractiveSynth, SynthPreset};

/// A deterministic offline-render fixture: a preset, a fixed event list, and
/// the render shape plus the expected output peak range.
#[derive(Clone, Debug, PartialEq)]
pub struct SynthOfflineFixture {
    /// The preset to render.
    pub preset: SynthPreset,
    /// The block events to feed the synth.
    pub events: Vec<BlockEvent<'static>>,
    /// The render sample rate in hertz.
    pub sample_rate_hz: u32,
    /// The number of frames to render.
    pub frames: usize,
    /// The number of output channels.
    pub channels: usize,
    /// The expected `(min, max)` output peak range used for verification.
    pub expected_peak_range: (f32, f32),
}

/// Returns the R31 single-note fixture: a sine voice triggered and released
/// over 128 frames.
pub fn r31_synth_note_fixture() -> SynthOfflineFixture {
    let preset = SynthPreset {
        name: "r31-note-fixture".to_owned(),
        oscillator: OscillatorKind::Sine,
        amp_envelope: AdsrSettings {
            attack_s: 0.0,
            decay_s: 0.0,
            sustain_level: 1.0,
            release_s: 0.02,
        },
        max_voices: 4,
        amp_gain: 0.5,
        filter_cutoff_hz: 20_000.0,
        ..SynthPreset::default()
    };
    SynthOfflineFixture {
        preset,
        events: vec![
            BlockEvent::NoteOn {
                offset: 0,
                channel: 0,
                key: 69,
                velocity: 1.0,
            },
            BlockEvent::NoteOff {
                offset: 64,
                channel: 0,
                key: 69,
                velocity: 0.0,
            },
        ],
        sample_rate_hz: 48_000,
        frames: 128,
        channels: 2,
        expected_peak_range: (0.35, 0.55),
    }
}

/// Renders the fixture through a fresh [`SubtractiveSynth`], returning one
/// sample vector per channel.
pub fn render_synth_offline(fixture: &SynthOfflineFixture) -> Vec<Vec<f32>> {
    let mut synth = SubtractiveSynth::new(fixture.preset.clone());
    render_processor(
        &mut synth,
        &fixture.events,
        fixture.sample_rate_hz,
        fixture.frames,
        fixture.channels,
    )
}

/// Renders the given events through any processor offline, preparing it and
/// returning one sample vector per channel.
pub fn render_processor<P: Processor + ?Sized>(
    processor: &mut P,
    events: &[BlockEvent<'_>],
    sample_rate_hz: u32,
    frames: usize,
    channels: usize,
) -> Vec<Vec<f32>> {
    processor.prepare(PrepareConfig::new(
        sample_rate_hz,
        frames as u32,
        0,
        channels as u16,
    ));
    let mut output = vec![vec![0.0; frames]; channels];
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(Vec::as_mut_slice).collect();
    let mut sink = NullEventSink;
    let mut scratch = BlockArena::with_f32_capacity(frames * channels.max(1));
    let mut block = ProcessBlock {
        frames: frames as u32,
        in_audio: &[],
        out_audio: &mut output_refs,
        in_events: events,
        out_events: &mut sink,
        transport: Transport::default(),
        scratch: &mut scratch,
    };
    processor.process(&mut block);
    output
}
