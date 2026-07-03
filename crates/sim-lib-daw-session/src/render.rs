use sim_kernel::{Error, Result};
use sim_lib_stream_audio::{PcmBuffer, PcmSpec};

use crate::{ClipSource, DawSession, DawTrackKind};

/// Offline render output plus summary counters.
#[derive(Clone, Debug, PartialEq)]
pub struct DawOfflineRender {
    buffer: PcmBuffer,
    tracks_rendered: usize,
    clips_rendered: usize,
}

impl DawSession {
    /// Renders `frames` of the session offline; see [`render_session_offline`].
    pub fn render_offline(&self, frames: usize) -> Result<DawOfflineRender> {
        render_session_offline(self, frames)
    }
}

/// Renders the session to a deterministic offline PCM buffer.
///
/// Sums every audible audio track's constant-source clips into the master bus
/// channel layout, honoring mute and solo. Non-constant clip sources (silence,
/// patch nodes, arrangers) contribute nothing to the offline buffer. Fails if
/// `frames` is zero or a clip gain is non-finite.
pub fn render_session_offline(session: &DawSession, frames: usize) -> Result<DawOfflineRender> {
    if frames == 0 {
        return Err(Error::Eval(
            "DAW offline render frame count must be greater than zero".to_owned(),
        ));
    }
    let channels = session
        .buses()
        .first()
        .map(|bus| bus.channels() as usize)
        .unwrap_or(2)
        .max(1);
    let mut samples = vec![0.0f32; frames * channels];
    let any_solo = session.tracks().iter().any(|track| track.is_solo());
    let mut tracks_rendered = 0;
    let mut clips_rendered = 0;
    let start = session.transport().sample_pos();

    for track in session.tracks() {
        if track.kind() != DawTrackKind::Audio || track.is_muted() {
            continue;
        }
        if any_solo && !track.is_solo() {
            continue;
        }
        tracks_rendered += 1;
        let mut target = RenderTarget {
            output: &mut samples,
            channels,
            frames,
            render_start: start,
        };
        for clip in track.clips() {
            if render_clip(
                &mut target,
                clip.start_frame(),
                clip.frames(),
                clip.source(),
                clip.gain(),
            )? {
                clips_rendered += 1;
            }
        }
    }

    let spec = PcmSpec::f32(channels, session.sample_rate_hz())?;
    Ok(DawOfflineRender {
        buffer: PcmBuffer::f32(spec, frames, samples)?,
        tracks_rendered,
        clips_rendered,
    })
}

impl DawOfflineRender {
    /// Returns the rendered PCM buffer.
    pub fn buffer(&self) -> &PcmBuffer {
        &self.buffer
    }

    /// Returns the number of audible audio tracks that contributed.
    pub fn tracks_rendered(&self) -> usize {
        self.tracks_rendered
    }

    /// Returns the number of clips that contributed samples.
    pub fn clips_rendered(&self) -> usize {
        self.clips_rendered
    }
}

struct RenderTarget<'a> {
    output: &'a mut [f32],
    channels: usize,
    frames: usize,
    render_start: u64,
}

fn render_clip(
    target: &mut RenderTarget<'_>,
    clip_start: u64,
    clip_frames: u64,
    source: &ClipSource,
    gain: f32,
) -> Result<bool> {
    if !gain.is_finite() {
        return Err(Error::Eval("DAW clip gain must be finite".to_owned()));
    }
    let Some(value) = source_value(source)? else {
        return Ok(false);
    };
    let render_end = target.render_start.saturating_add(target.frames as u64);
    let clip_end = clip_start.saturating_add(clip_frames);
    let start = target.render_start.max(clip_start);
    let end = render_end.min(clip_end);
    if start >= end {
        return Ok(false);
    }
    for frame in start..end {
        let local = (frame - target.render_start) as usize;
        for channel in 0..target.channels {
            target.output[local * target.channels + channel] += value * gain;
        }
    }
    Ok(true)
}

fn source_value(source: &ClipSource) -> Result<Option<f32>> {
    match source {
        ClipSource::Silence | ClipSource::PatchNode(_) | ClipSource::Arranger(_) => Ok(None),
        ClipSource::Constant(value) if value.is_finite() => Ok(Some(*value)),
        ClipSource::Constant(_) => Err(Error::Eval(
            "DAW clip constant source must be finite".to_owned(),
        )),
    }
}
