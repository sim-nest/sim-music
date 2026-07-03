use std::path::Path;

use sim_kernel::{Error, Result};
use sim_lib_stream_audio::{MemoryPcmSink, MemoryPcmSource, PcmBuffer, PcmPumpSummary, PcmSpec};
use sim_lib_stream_core::{StreamMetadata, StreamValue};

use crate::effect_io::{read_file_with_effect, write_file_with_effect};

/// A PCM stream decoded from a WAV file together with its sample format.
pub struct WavStream {
    spec: PcmSpec,
    stream: StreamValue,
}

impl WavStream {
    /// Returns the PCM sample format (channels and sample rate) of the WAV.
    pub fn spec(&self) -> PcmSpec {
        self.spec
    }

    /// Borrows the decoded PCM stream.
    pub fn stream(&self) -> &StreamValue {
        &self.stream
    }

    /// Consumes the wrapper and returns the decoded PCM stream.
    pub fn into_stream(self) -> StreamValue {
        self.stream
    }
}

/// Reads a canonical PCM16 WAV file from `path` and opens it as a PCM stream.
///
/// The read is gated by the filesystem read capability and recorded as a
/// KERNEL 6 filesystem effect. Packets carry up to `frames_per_packet` frames.
pub fn read_wav_stream(
    cx: &mut sim_kernel::Cx,
    path: impl AsRef<Path>,
    frames_per_packet: usize,
    metadata: StreamMetadata,
) -> Result<WavStream> {
    let bytes = read_file_with_effect(cx, path)?;
    wav_bytes_to_stream(&bytes, frames_per_packet, metadata)
}

/// Decodes canonical PCM16 WAV bytes and opens them as a PCM stream.
///
/// Only little-endian PCM16 RIFF/WAVE input is supported; other formats fail.
pub fn wav_bytes_to_stream(
    bytes: &[u8],
    frames_per_packet: usize,
    metadata: StreamMetadata,
) -> Result<WavStream> {
    let (spec, buffers) = wav_bytes_to_buffers(bytes, frames_per_packet)?;
    let mut source = MemoryPcmSource::new(spec, buffers)?;
    let stream = sim_lib_stream_audio::pcm_source_to_stream(&mut source, metadata)?;
    Ok(WavStream { spec, stream })
}

/// Drains a PCM stream and writes it to `path` as a canonical PCM16 WAV file.
///
/// The write is gated by the filesystem write capability and recorded as a
/// KERNEL 6 filesystem effect. Returns a summary of the pumped PCM.
pub fn write_wav_stream(
    cx: &mut sim_kernel::Cx,
    path: impl AsRef<Path>,
    stream: &StreamValue,
    spec: PcmSpec,
) -> Result<PcmPumpSummary> {
    let mut sink = MemoryPcmSink::new(spec);
    let summary = sim_lib_stream_audio::stream_to_pcm_sink(stream, &mut sink)?;
    let bytes = pcm_buffers_to_wav_bytes(spec, sink.buffers())?;
    write_file_with_effect(cx, path, bytes)?;
    Ok(summary)
}

/// Drains a PCM stream and encodes it as canonical PCM16 WAV bytes.
///
/// Returns the encoded bytes together with a summary of the pumped PCM.
pub fn stream_to_wav_bytes(
    stream: &StreamValue,
    spec: PcmSpec,
) -> Result<(Vec<u8>, PcmPumpSummary)> {
    let mut sink = MemoryPcmSink::new(spec);
    let summary = sim_lib_stream_audio::stream_to_pcm_sink(stream, &mut sink)?;
    Ok((pcm_buffers_to_wav_bytes(spec, sink.buffers())?, summary))
}

/// Encodes PCM buffers into canonical PCM16 WAV bytes.
///
/// Every buffer must share `spec`; a mismatched buffer spec is an error.
///
/// # Examples
///
/// ```
/// use sim_lib_stream_audio::PcmSpec;
/// use sim_lib_stream_file::pcm_buffers_to_wav_bytes;
///
/// let spec = PcmSpec::i16(2, 48_000).unwrap();
/// let bytes = pcm_buffers_to_wav_bytes(spec, &[]).unwrap();
/// assert_eq!(&bytes[0..4], b"RIFF");
/// assert_eq!(&bytes[8..12], b"WAVE");
/// ```
pub fn pcm_buffers_to_wav_bytes(spec: PcmSpec, buffers: &[PcmBuffer]) -> Result<Vec<u8>> {
    let mut samples = Vec::new();
    for buffer in buffers {
        if buffer.spec() != spec {
            return Err(Error::Eval(
                "WAV writer received a PCM buffer with a mismatched spec".to_owned(),
            ));
        }
        samples.extend_from_slice(buffer.samples_i16());
    }
    encode_wav_i16(spec, &samples)
}

fn wav_bytes_to_buffers(
    bytes: &[u8],
    frames_per_packet: usize,
) -> Result<(PcmSpec, Vec<PcmBuffer>)> {
    if frames_per_packet == 0 {
        return Err(Error::Eval(
            "WAV frames-per-packet must be greater than zero".to_owned(),
        ));
    }
    let parsed = parse_wav_i16(bytes)?;
    let channels = parsed.spec.channels();
    let samples_per_packet = channels
        .checked_mul(frames_per_packet)
        .ok_or_else(|| Error::Eval("WAV packet sample count overflow".to_owned()))?;
    let mut buffers = Vec::new();
    for chunk in parsed.samples.chunks(samples_per_packet) {
        if chunk.is_empty() {
            continue;
        }
        if !chunk.len().is_multiple_of(channels) {
            return Err(Error::Eval(
                "malformed WAV file: PCM data ends mid-frame".to_owned(),
            ));
        }
        buffers.push(PcmBuffer::i16(
            parsed.spec,
            chunk.len() / channels,
            chunk.to_vec(),
        )?);
    }
    Ok((parsed.spec, buffers))
}

struct ParsedWav {
    spec: PcmSpec,
    samples: Vec<i16>,
}

fn parse_wav_i16(bytes: &[u8]) -> Result<ParsedWav> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(Error::Eval(
            "malformed WAV file: missing RIFF/WAVE header".to_owned(),
        ));
    }
    let mut pos = 12usize;
    let mut fmt: Option<(usize, u32, u16)> = None;
    let mut data: Option<&[u8]> = None;
    while pos + 8 <= bytes.len() {
        let chunk_id = &bytes[pos..pos + 4];
        let len = read_u32_le(bytes, pos + 4)? as usize;
        let start = pos + 8;
        let end = start
            .checked_add(len)
            .ok_or_else(|| Error::Eval("malformed WAV file: chunk length overflow".to_owned()))?;
        if end > bytes.len() {
            return Err(Error::Eval(
                "malformed WAV file: chunk extends past end of file".to_owned(),
            ));
        }
        match chunk_id {
            b"fmt " => fmt = Some(parse_fmt_chunk(&bytes[start..end])?),
            b"data" => data = Some(&bytes[start..end]),
            _ => {}
        }
        pos = end + usize::from(!len.is_multiple_of(2));
    }
    let (channels, sample_rate, bits_per_sample) =
        fmt.ok_or_else(|| Error::Eval("malformed WAV file: missing fmt chunk".to_owned()))?;
    if bits_per_sample != 16 {
        return Err(Error::Eval(format!(
            "malformed WAV file: unsupported PCM bit depth {bits_per_sample}"
        )));
    }
    let data =
        data.ok_or_else(|| Error::Eval("malformed WAV file: missing data chunk".to_owned()))?;
    if !data.len().is_multiple_of(2) {
        return Err(Error::Eval(
            "malformed WAV file: PCM16 data has odd byte length".to_owned(),
        ));
    }
    let spec = PcmSpec::i16(channels, sample_rate)?;
    let samples = data
        .chunks_exact(2)
        .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
        .collect::<Vec<_>>();
    if !samples.len().is_multiple_of(channels) {
        return Err(Error::Eval(
            "malformed WAV file: PCM data ends mid-frame".to_owned(),
        ));
    }
    Ok(ParsedWav { spec, samples })
}

fn parse_fmt_chunk(bytes: &[u8]) -> Result<(usize, u32, u16)> {
    if bytes.len() < 16 {
        return Err(Error::Eval(
            "malformed WAV file: fmt chunk is too short".to_owned(),
        ));
    }
    let audio_format = read_u16_le(bytes, 0)?;
    if audio_format != 1 {
        return Err(Error::Eval(format!(
            "malformed WAV file: unsupported audio format {audio_format}"
        )));
    }
    let channels = usize::from(read_u16_le(bytes, 2)?);
    let sample_rate = read_u32_le(bytes, 4)?;
    let block_align = usize::from(read_u16_le(bytes, 12)?);
    let bits_per_sample = read_u16_le(bytes, 14)?;
    if block_align != channels.saturating_mul(2) {
        return Err(Error::Eval(
            "malformed WAV file: PCM16 block alignment mismatch".to_owned(),
        ));
    }
    Ok((channels, sample_rate, bits_per_sample))
}

fn encode_wav_i16(spec: PcmSpec, samples: &[i16]) -> Result<Vec<u8>> {
    if !samples.len().is_multiple_of(spec.channels()) {
        return Err(Error::Eval(
            "WAV writer received samples that end mid-frame".to_owned(),
        ));
    }
    let channels = u16::try_from(spec.channels())
        .map_err(|_| Error::Eval("WAV channel count exceeds u16".to_owned()))?;
    let data_len = samples
        .len()
        .checked_mul(2)
        .ok_or_else(|| Error::Eval("WAV data length overflow".to_owned()))?;
    let data_len_u32 =
        u32::try_from(data_len).map_err(|_| Error::Eval("WAV data exceeds u32".to_owned()))?;
    let riff_len = 36u32
        .checked_add(data_len_u32)
        .ok_or_else(|| Error::Eval("WAV RIFF length overflow".to_owned()))?;
    let block_align = channels
        .checked_mul(2)
        .ok_or_else(|| Error::Eval("WAV block alignment overflow".to_owned()))?;
    let byte_rate = spec
        .sample_rate_hz()
        .checked_mul(u32::from(block_align))
        .ok_or_else(|| Error::Eval("WAV byte rate overflow".to_owned()))?;

    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&spec.sample_rate_hz().to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len_u32.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(out)
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Result<u16> {
    let end = offset + 2;
    let slice = bytes.get(offset..end).ok_or_else(|| {
        Error::Eval(format!(
            "malformed WAV file: unexpected end at byte {offset}"
        ))
    })?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset + 4;
    let slice = bytes.get(offset..end).ok_or_else(|| {
        Error::Eval(format!(
            "malformed WAV file: unexpected end at byte {offset}"
        ))
    })?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}
