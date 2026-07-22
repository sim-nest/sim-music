use std::io::Write;
use std::time::Duration;

use sim_lib_sound_bridge::ScheduledTone;
use sim_lib_sound_core::Tone;

use crate::SoundRenderError;

/// Configuration for a [`PcmRenderer`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RendererOptions {
    /// Output sample rate, in hertz.
    pub sample_rate: u32,
    /// Output channel count (1 for mono, 2 for stereo).
    pub channels: u8,
}

impl RendererOptions {
    /// Builds options, rejecting a zero sample rate or a channel count outside
    /// `1..=2`.
    pub fn new(sample_rate: u32, channels: u8) -> Result<Self, SoundRenderError> {
        if sample_rate == 0 {
            return Err(SoundRenderError::InvalidSampleRate);
        }
        if !(1..=2).contains(&channels) {
            return Err(SoundRenderError::InvalidChannelCount);
        }
        Ok(Self {
            sample_rate,
            channels,
        })
    }
}

impl Default for RendererOptions {
    fn default() -> Self {
        Self {
            sample_rate: 44_100,
            channels: 2,
        }
    }
}

/// A renderer that synthesizes tones into interleaved PCM `f32` samples and
/// encodes them as WAV.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PcmRenderer {
    /// Output sample rate, in hertz.
    sample_rate: u32,
    /// Output channel count (1 for mono, 2 for stereo).
    channels: u8,
}

impl PcmRenderer {
    /// Builds a renderer from validated [`RendererOptions`].
    pub fn new(options: RendererOptions) -> Result<Self, SoundRenderError> {
        let _ = RendererOptions::new(options.sample_rate, options.channels)?;
        Ok(Self {
            sample_rate: options.sample_rate,
            channels: options.channels,
        })
    }

    /// Returns the output sample rate, in hertz.
    pub fn sample_rate(self) -> u32 {
        self.sample_rate
    }

    /// Returns the output channel count.
    pub fn channels(self) -> u8 {
        self.channels
    }

    /// Renders a single tone to interleaved PCM samples, centered in the
    /// stereo field.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use sim_lib_sound_core::{Frequency, Tone};
    /// use sim_lib_sound_render::{PcmRenderer, RendererOptions};
    ///
    /// let renderer = PcmRenderer::new(RendererOptions::new(8_000, 1).unwrap()).unwrap();
    /// let tone = Tone::sine(Frequency::new(440.0).unwrap(), Duration::from_millis(10));
    /// assert_eq!(renderer.render_tone(&tone).len(), 80);
    /// ```
    pub fn render_tone(&self, tone: &Tone) -> Vec<f32> {
        self.render_tone_with_pan(tone, 0.0)
    }

    /// Renders and sums a set of scheduled tones into a single mixed PCM
    /// buffer, honoring each tone's start time and pan.
    pub fn render_mix(&self, tones: &[ScheduledTone]) -> Vec<f32> {
        let frames = tones
            .iter()
            .map(|scheduled| {
                start_frame(self.sample_rate, scheduled.start)
                    + tone_frames(self.sample_rate, &scheduled.tone)
            })
            .max()
            .unwrap_or(0);
        let mut mix = vec![0.0_f32; frames * usize::from(self.channels)];
        for scheduled in tones {
            let rendered = self.render_tone_with_pan(&scheduled.tone, scheduled.pan);
            let offset =
                start_frame(self.sample_rate, scheduled.start) * usize::from(self.channels);
            for (index, sample) in rendered.iter().enumerate() {
                if let Some(slot) = mix.get_mut(offset + index) {
                    *slot += *sample;
                }
            }
        }
        mix
    }

    /// Encodes `samples` as a 16-bit PCM WAV stream to `writer`, returning the
    /// writer on success.
    pub fn write_wav<W: Write>(
        &self,
        samples: &[f32],
        mut writer: W,
    ) -> Result<W, SoundRenderError> {
        let channels = usize::from(self.channels);
        if channels == 0 || !samples.len().is_multiple_of(channels) {
            return Err(SoundRenderError::ChannelMisalignedSamples);
        }
        let sample_count =
            u32::try_from(samples.len()).map_err(|_| SoundRenderError::BufferTooLarge)?;
        let bytes_per_sample = 2_u16;
        let block_align = u16::from(self.channels)
            .checked_mul(bytes_per_sample)
            .ok_or(SoundRenderError::BufferTooLarge)?;
        let byte_rate = self
            .sample_rate
            .checked_mul(u32::from(block_align))
            .ok_or(SoundRenderError::BufferTooLarge)?;
        let data_size = sample_count
            .checked_mul(u32::from(bytes_per_sample))
            .ok_or(SoundRenderError::BufferTooLarge)?;
        let riff_size = 36_u32
            .checked_add(data_size)
            .ok_or(SoundRenderError::BufferTooLarge)?;
        writer
            .write_all(b"RIFF")
            .map_err(|_| SoundRenderError::BufferTooLarge)?;
        writer
            .write_all(&riff_size.to_le_bytes())
            .map_err(|_| SoundRenderError::BufferTooLarge)?;
        writer
            .write_all(b"WAVE")
            .map_err(|_| SoundRenderError::BufferTooLarge)?;
        writer
            .write_all(b"fmt ")
            .map_err(|_| SoundRenderError::BufferTooLarge)?;
        writer
            .write_all(&16_u32.to_le_bytes())
            .map_err(|_| SoundRenderError::BufferTooLarge)?;
        writer
            .write_all(&1_u16.to_le_bytes())
            .map_err(|_| SoundRenderError::BufferTooLarge)?;
        writer
            .write_all(&u16::from(self.channels).to_le_bytes())
            .map_err(|_| SoundRenderError::BufferTooLarge)?;
        writer
            .write_all(&self.sample_rate.to_le_bytes())
            .map_err(|_| SoundRenderError::BufferTooLarge)?;
        writer
            .write_all(&byte_rate.to_le_bytes())
            .map_err(|_| SoundRenderError::BufferTooLarge)?;
        writer
            .write_all(&block_align.to_le_bytes())
            .map_err(|_| SoundRenderError::BufferTooLarge)?;
        writer
            .write_all(&(bytes_per_sample * 8).to_le_bytes())
            .map_err(|_| SoundRenderError::BufferTooLarge)?;
        writer
            .write_all(b"data")
            .map_err(|_| SoundRenderError::BufferTooLarge)?;
        writer
            .write_all(&data_size.to_le_bytes())
            .map_err(|_| SoundRenderError::BufferTooLarge)?;
        for sample in samples {
            let pcm = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
            writer
                .write_all(&pcm.to_le_bytes())
                .map_err(|_| SoundRenderError::BufferTooLarge)?;
        }
        Ok(writer)
    }

    fn render_tone_with_pan(&self, tone: &Tone, pan: f32) -> Vec<f32> {
        let frames = tone_frames(self.sample_rate, tone);
        let mut out = vec![0.0_f32; frames * usize::from(self.channels)];
        let (left_gain, right_gain) = pan_gains(pan);
        for frame in 0..frames {
            let time = Duration::from_secs_f64(frame as f64 / f64::from(self.sample_rate));
            let env = tone.envelope.sample_level(time, tone.duration) as f32;
            let mut mono = 0.0_f32;
            for partial in &tone.partials {
                let angle = std::f64::consts::TAU * partial.frequency.0 * time.as_secs_f64()
                    + partial.phase.0;
                mono += (angle.sin() * partial.amplitude.0) as f32;
            }
            let sample = mono * env;
            match self.channels {
                1 => out[frame] = sample,
                2 => {
                    let base = frame * 2;
                    out[base] = sample * left_gain;
                    out[base + 1] = sample * right_gain;
                }
                _ => unreachable!(),
            }
        }
        out
    }
}

fn tone_frames(sample_rate: u32, tone: &Tone) -> usize {
    (tone.duration.as_secs_f64() * f64::from(sample_rate)).ceil() as usize
}

fn start_frame(sample_rate: u32, start: Duration) -> usize {
    (start.as_secs_f64() * f64::from(sample_rate)).round() as usize
}

fn pan_gains(pan: f32) -> (f32, f32) {
    let normalized = ((pan.clamp(-1.0, 1.0) + 1.0) * 0.5) * std::f32::consts::FRAC_PI_2;
    (normalized.cos(), normalized.sin())
}
