use thiserror::Error;

use sim_lib_midi_core::{MemoryMidiSource, pump};
use sim_lib_music_core::{Pitch, Score};
use sim_lib_music_lower::{LowerError, LowerOpts, lower_score};
use sim_lib_music_shapes::{MusicShapeError, decode_music_file};
use sim_lib_sound_bridge::{BridgeOptions, MidiToSoundBridge, SoundBridgeError, TimbreBank};
use sim_lib_sound_core::Frequency;
use sim_lib_sound_dissonance::{
    DissonanceRegistry, DissonanceScore, analyze_chord as analyze_tone_chord,
};
use sim_lib_sound_gm::general_midi_bank;
use sim_lib_sound_render::{PcmRenderer, RendererOptions, SoundRenderError};
use sim_lib_sound_tuning::{EqualTemperament, PitchClassN, SoundTuningError, Tuning};
use sim_lib_stream_core::{PcmSampleFormat, StreamEnvelope, StreamPacket};

/// Error raised by the sound wasm facade.
#[derive(Debug, Error)]
pub enum SoundWasmError {
    /// A music-shape file failed to decode.
    #[error(transparent)]
    Shape(#[from] MusicShapeError),
    /// Lowering a score to MIDI failed.
    #[error(transparent)]
    Lower(#[from] LowerError),
    /// The MIDI-to-sound bridge failed.
    #[error(transparent)]
    Bridge(#[from] SoundBridgeError),
    /// Pumping MIDI events through the bridge failed.
    #[error("midi pump failed")]
    Pump,
    /// PCM rendering or WAV encoding failed.
    #[error(transparent)]
    Render(#[from] SoundRenderError),
    /// A tuning operation failed.
    #[error(transparent)]
    Tuning(#[from] SoundTuningError),
    /// A web-audio preview could not be produced.
    #[error("web audio preview failed: {0}")]
    Preview(String),
}

/// A rendered audio buffer with its WAV encoding and frame metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderedAudioView {
    /// WAV-encoded audio bytes.
    pub wav: Vec<u8>,
    /// Sample rate, in hertz.
    pub sample_rate: u32,
    /// Channel count.
    pub channels: u8,
    /// Number of audio frames.
    pub frame_count: usize,
}

/// A single dissonance model's score for the rendered chord.
#[derive(Clone, Debug, PartialEq)]
pub struct DissonanceModelView {
    /// Name of the dissonance model.
    pub model: String,
    /// Computed dissonance score.
    pub score: f64,
}

/// A single degree of a tuning, with its frequency and interval from the root.
#[derive(Clone, Debug, PartialEq)]
pub struct TuningIntervalView {
    /// Degree index within the tuning.
    pub degree: u32,
    /// Frequency of the degree, in hertz.
    pub frequency_hz: f64,
    /// Interval from the root, in cents.
    pub cents_from_root: f64,
}

/// The full result of rendering a score demo: audio, dissonance, intervals,
/// and diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundDemoReport {
    /// Rendered audio.
    pub audio: RenderedAudioView,
    /// Per-model dissonance scores for the rendered chord.
    pub dissonance: Vec<DissonanceModelView>,
    /// Interval table for the tuning used.
    pub intervals: Vec<TuningIntervalView>,
    /// Human-readable diagnostics.
    pub diagnostics: Vec<String>,
}

/// A decoded buffered-PCM stream ready for Web Audio playback.
#[derive(Clone, Debug, PartialEq)]
pub struct WebAudioPreview {
    /// Stream identifier.
    pub stream_id: String,
    /// Stream profile name.
    pub profile: String,
    /// Sample rate, in hertz.
    pub sample_rate: u32,
    /// Channel count.
    pub channels: usize,
    /// Number of audio frames.
    pub frame_count: usize,
    /// Interleaved PCM samples in `-1.0..=1.0`.
    pub samples: Vec<f32>,
}

/// Stable wasm engine entrypoint names exposed to browser and ABI adapters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundWasmEngineEntryPoints {
    /// Entrypoint that renders a music file into a demo report.
    pub render_demo: &'static str,
    /// Entrypoint that decodes buffered PCM into a preview.
    pub preview_pcm: &'static str,
    /// Entrypoint for the audio-worklet preview.
    pub audio_worklet: &'static str,
}

/// Returns the stable wasm engine entrypoint names.
///
/// # Examples
///
/// ```
/// use sim_lib_sound_wasm_frame::sound_wasm_engine_entry_points;
///
/// let entries = sound_wasm_engine_entry_points();
/// assert_eq!(entries.render_demo, "sound-wasm-render-demo");
/// ```
pub fn sound_wasm_engine_entry_points() -> SoundWasmEngineEntryPoints {
    SoundWasmEngineEntryPoints {
        render_demo: "sound-wasm-render-demo",
        preview_pcm: "sound-wasm-preview-pcm",
        audio_worklet: "sound-audio-worklet-preview",
    }
}

/// Builds a [`WebAudioPreview`] from a buffered-PCM stream envelope, converting
/// any 16-bit samples to floats and rejecting non-preview profiles.
pub fn web_audio_preview_from_buffered_pcm(
    envelope: &StreamEnvelope,
    sample_rate: u32,
) -> Result<WebAudioPreview, SoundWasmError> {
    if sample_rate == 0 {
        return Err(SoundWasmError::Preview(
            "sample rate must be greater than zero".to_owned(),
        ));
    }
    let profile = envelope.profile().name().as_qualified_str();
    if profile != "stream/profile/buffered-pcm-preview"
        && profile != "stream/profile/lan-buffered-audio-preview"
    {
        return Err(SoundWasmError::Preview(format!(
            "profile {profile} is not a buffered PCM preview"
        )));
    }
    let StreamPacket::Pcm(packet) = envelope.packet() else {
        return Err(SoundWasmError::Preview(
            "buffered preview envelope must carry PCM".to_owned(),
        ));
    };
    let samples = match packet.sample_format() {
        PcmSampleFormat::F32 => packet.samples_f32().to_vec(),
        PcmSampleFormat::I16 => packet
            .samples_i16()
            .iter()
            .map(|sample| {
                if *sample == i16::MIN {
                    -1.0
                } else {
                    f32::from(*sample) / f32::from(i16::MAX)
                }
            })
            .collect(),
    };
    Ok(WebAudioPreview {
        stream_id: envelope.stream_id().as_qualified_str(),
        profile,
        sample_rate,
        channels: packet.channels(),
        frame_count: packet.frames(),
        samples,
    })
}

/// Decodes a music file and renders it into a [`SoundDemoReport`] using the
/// General MIDI bank, equal temperament, and default rendering options.
pub fn render_music_file(input: &str) -> Result<SoundDemoReport, SoundWasmError> {
    let score = decode_music_file(input)?;
    let tuning = EqualTemperament::default();
    let bank = general_midi_bank();
    let renderer = PcmRenderer::new(RendererOptions::default())?;
    render_score_demo(&score, &bank, &tuning, &renderer)
}

/// Renders a [`Score`] into a [`SoundDemoReport`] with the given bank, tuning,
/// and renderer: lowers it to MIDI, bridges it to tones, mixes and encodes the
/// audio, and computes dissonance and tuning tables.
pub fn render_score_demo(
    score: &Score,
    bank: &TimbreBank,
    tuning: &dyn Tuning,
    renderer: &PcmRenderer,
) -> Result<SoundDemoReport, SoundWasmError> {
    let smf = lower_score(score, &LowerOpts::default())?;
    let events = smf
        .merged_events()
        .into_iter()
        .map(|tracked| tracked.event)
        .collect();
    let mut source = MemoryMidiSource::new(smf.tpq, events);
    let mut bridge = MidiToSoundBridge::new(
        smf.tpq,
        bank.clone(),
        Box::new(FrozenTuning::from_tuning(tuning)),
        BridgeOptions::default(),
    )?;
    let _ = pump(&mut source, &mut bridge).map_err(|_| SoundWasmError::Pump)?;
    let tones = bridge.drain_tones();
    let samples = renderer.render_mix(&tones);
    let wav = renderer.write_wav(&samples, Vec::new())?;
    Ok(SoundDemoReport {
        audio: RenderedAudioView {
            wav,
            sample_rate: renderer.sample_rate,
            channels: renderer.channels,
            frame_count: samples.len() / usize::from(renderer.channels),
        },
        dissonance: tone_dissonance(&tones),
        intervals: tuning_table(tuning)?,
        diagnostics: diagnostics(&bridge, &samples),
    })
}

fn tone_dissonance(tones: &[sim_lib_sound_bridge::ScheduledTone]) -> Vec<DissonanceModelView> {
    let chord = tones
        .iter()
        .take(4)
        .map(|scheduled| scheduled.tone.clone())
        .collect::<Vec<_>>();
    analyze_tone_chord(&chord, &DissonanceRegistry::new_with_builtins())
        .into_iter()
        .map(score_view)
        .collect()
}

fn tuning_table(tuning: &dyn Tuning) -> Result<Vec<TuningIntervalView>, SoundTuningError> {
    let (reference_pitch, reference_frequency) = tuning.reference();
    (0..tuning.divisions())
        .map(|degree| {
            let pcn = PitchClassN::new(tuning.divisions(), degree)?;
            let frequency = tuning.frequency_of_degree(pcn, reference_pitch.octave)?;
            Ok(TuningIntervalView {
                degree,
                frequency_hz: frequency.0,
                cents_from_root: frequency.cents_above(reference_frequency),
            })
        })
        .collect()
}

fn diagnostics(bridge: &MidiToSoundBridge, samples: &[f32]) -> Vec<String> {
    let mut out = Vec::new();
    if bridge.stolen_voice_count() > 0 {
        out.push(format!(
            "voice stealing: {} voice(s)",
            bridge.stolen_voice_count()
        ));
    }
    if let Some(peak) = samples.iter().map(|sample| sample.abs()).reduce(f32::max)
        && peak > 1.0
    {
        out.push(format!("audio clipping peak {:.3}", peak));
    }
    out
}

fn score_view(score: DissonanceScore) -> DissonanceModelView {
    DissonanceModelView {
        model: score.model,
        score: score.score,
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FrozenTuning {
    reference: (Pitch, Frequency),
    divisions: u32,
}

impl FrozenTuning {
    fn from_tuning(tuning: &dyn Tuning) -> Self {
        Self {
            reference: tuning.reference(),
            divisions: tuning.divisions(),
        }
    }
}

impl Tuning for FrozenTuning {
    fn name(&self) -> &'static str {
        "frozen-equal-temperament"
    }

    fn reference(&self) -> (Pitch, Frequency) {
        self.reference
    }

    fn frequency_of(&self, pitch: Pitch) -> Frequency {
        EqualTemperament::new(self.divisions, self.reference)
            .unwrap_or_default()
            .frequency_of(pitch)
    }

    fn pitch_of(&self, frequency: Frequency) -> Pitch {
        EqualTemperament::new(self.divisions, self.reference)
            .unwrap_or_default()
            .pitch_of(frequency)
    }

    fn divisions(&self) -> u32 {
        self.divisions
    }

    fn frequency_of_degree(
        &self,
        degree: PitchClassN,
        octave: i16,
    ) -> Result<Frequency, SoundTuningError> {
        EqualTemperament::new(self.divisions, self.reference)
            .unwrap_or_default()
            .frequency_of_degree(degree, octave)
    }
}
