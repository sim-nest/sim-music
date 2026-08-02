use std::{error::Error, fmt};

const MAX_CHANNELS: usize = 32;

/// Finite row-major channel mapping applied before sample quantization.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelMatrix {
    input_channels: usize,
    output_channels: usize,
    gains: Vec<f32>,
}

impl ChannelMatrix {
    /// Builds a bounded row-major matrix with one row per output channel.
    pub fn new(
        input_channels: usize,
        output_channels: usize,
        gains: Vec<f32>,
    ) -> Result<Self, PcmConversionError> {
        if input_channels == 0
            || output_channels == 0
            || input_channels > MAX_CHANNELS
            || output_channels > MAX_CHANNELS
        {
            return Err(PcmConversionError::InvalidChannels);
        }
        let expected = input_channels
            .checked_mul(output_channels)
            .ok_or(PcmConversionError::InvalidChannels)?;
        if gains.len() != expected {
            return Err(PcmConversionError::MatrixShape {
                expected,
                actual: gains.len(),
            });
        }
        if gains.iter().any(|gain| !gain.is_finite()) {
            return Err(PcmConversionError::NonFiniteMatrix);
        }
        Ok(Self {
            input_channels,
            output_channels,
            gains,
        })
    }

    /// Builds an identity mapping for `channels` interleaved lanes.
    pub fn identity(channels: usize) -> Result<Self, PcmConversionError> {
        let cells = channels
            .checked_mul(channels)
            .ok_or(PcmConversionError::InvalidChannels)?;
        let mut gains = vec![0.0; cells];
        for channel in 0..channels {
            gains[channel * channels + channel] = 1.0;
        }
        Self::new(channels, channels, gains)
    }

    /// Builds the canonical mono-to-stereo duplication mapping.
    pub fn mono_to_stereo() -> Self {
        Self {
            input_channels: 1,
            output_channels: 2,
            gains: vec![1.0, 1.0],
        }
    }

    /// Builds an equal-amplitude stereo-to-mono downmix.
    ///
    /// Each input is scaled by one half, so equal correlated channels retain
    /// their level without clipping solely because of the mapping.
    pub fn stereo_to_mono() -> Self {
        Self {
            input_channels: 2,
            output_channels: 1,
            gains: vec![0.5, 0.5],
        }
    }

    /// Returns the number of interleaved source channels.
    pub fn input_channels(&self) -> usize {
        self.input_channels
    }

    /// Returns the number of interleaved destination channels.
    pub fn output_channels(&self) -> usize {
        self.output_channels
    }

    /// Borrows row-major output-by-input gains.
    pub fn gains(&self) -> &[f32] {
        &self.gains
    }

    fn map(&self, frame: &[f32], output_channel: usize) -> f64 {
        let row = output_channel * self.input_channels;
        frame
            .iter()
            .enumerate()
            .map(|(input_channel, sample)| {
                f64::from(*sample) * f64::from(self.gains[row + input_channel])
            })
            .sum()
    }
}

/// Dither and error-feedback policy applied in the PCM16 quantizer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DitherPolicy {
    /// Round directly to the closest PCM16 code.
    None,
    /// Add deterministic triangular probability-density dither with `seed`.
    Tpdf {
        /// Reproducible nonzero or zero seed for the local generator.
        seed: u64,
    },
    /// Apply seeded TPDF plus first-order quantization-error feedback.
    NoiseShapedTpdf {
        /// Reproducible nonzero or zero seed for the local generator.
        seed: u64,
        /// Previous-error coefficient in the supported range `0..=0.95`.
        feedback: f32,
    },
}

/// Hard input bound and explicit quantization policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuantizationPolicy {
    /// Maximum whole input frames admitted by one conversion.
    pub max_frames: usize,
    /// Dither and optional first-order error-feedback behavior.
    pub dither: DitherPolicy,
}

impl Default for QuantizationPolicy {
    fn default() -> Self {
        Self {
            max_frames: 1_048_576,
            dither: DitherPolicy::None,
        }
    }
}

/// Invalid bounded channel mapping or PCM quantization request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PcmConversionError {
    /// Channel counts were zero or exceeded the fixed safety ceiling.
    InvalidChannels,
    /// Matrix storage did not match its declared channel shape.
    MatrixShape {
        /// Required row-major coefficient count.
        expected: usize,
        /// Supplied coefficient count.
        actual: usize,
    },
    /// A channel gain was NaN or infinite.
    NonFiniteMatrix,
    /// Interleaved source samples ended before a complete frame.
    MisalignedInput,
    /// A source sample was NaN or infinite.
    NonFiniteSample {
        /// Zero-based sample offset in the interleaved source.
        index: usize,
    },
    /// Input exceeded the caller-declared work bound.
    FrameLimit {
        /// Whole frames supplied.
        supplied: usize,
        /// Maximum admitted frames.
        maximum: usize,
    },
    /// A noise-shaping coefficient was outside its stable supported range.
    InvalidDither,
    /// Output-size or report arithmetic overflowed.
    SizeOverflow,
}

impl fmt::Display for PcmConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChannels => write!(f, "PCM channel count must be in 1..={MAX_CHANNELS}"),
            Self::MatrixShape { expected, actual } => {
                write!(f, "channel matrix needs {expected} gains, got {actual}")
            }
            Self::NonFiniteMatrix => write!(f, "channel matrix gains must be finite"),
            Self::MisalignedInput => write!(f, "interleaved PCM input ends mid-frame"),
            Self::NonFiniteSample { index } => write!(f, "PCM sample {index} is not finite"),
            Self::FrameLimit { supplied, maximum } => {
                write!(f, "PCM input has {supplied} frames, exceeding {maximum}")
            }
            Self::InvalidDither => write!(f, "noise-shaping feedback must be in 0..=0.95"),
            Self::SizeOverflow => write!(f, "PCM conversion size arithmetic overflowed"),
        }
    }
}

impl Error for PcmConversionError {}

/// Audit report for one channel-map and PCM16 quantization pass.
#[derive(Clone, Debug, PartialEq)]
pub struct PcmConversionReport {
    /// Whole source frames converted.
    pub frames: usize,
    /// Interleaved source channels.
    pub input_channels: usize,
    /// Interleaved destination channels.
    pub output_channels: usize,
    /// Peak absolute mapped float sample before clipping or dither.
    pub peak_before_quantization: f64,
    /// Count of mapped samples outside the representable `[-1, 1]` interval.
    pub clipped_samples: usize,
    /// Root-mean-square difference between mapped floats and PCM16 reconstruction.
    pub quantization_error_rms: f64,
    /// Exact dither policy used by the quantizer.
    pub dither: DitherPolicy,
}

/// PCM16 samples plus visible mapping, clipping, and quantization evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct Pcm16Conversion {
    /// Interleaved signed 16-bit PCM output.
    pub samples: Vec<i16>,
    /// Conversion evidence; clipping is reported rather than hidden.
    pub report: PcmConversionReport,
}

/// Maps bounded interleaved `f32` PCM and quantizes it to signed PCM16.
pub fn convert_f32_to_pcm16(
    input: &[f32],
    matrix: &ChannelMatrix,
    policy: QuantizationPolicy,
) -> Result<Pcm16Conversion, PcmConversionError> {
    validate_policy(policy)?;
    if !input.len().is_multiple_of(matrix.input_channels) {
        return Err(PcmConversionError::MisalignedInput);
    }
    let frames = input.len() / matrix.input_channels;
    if frames > policy.max_frames {
        return Err(PcmConversionError::FrameLimit {
            supplied: frames,
            maximum: policy.max_frames,
        });
    }
    let output_len = frames
        .checked_mul(matrix.output_channels)
        .ok_or(PcmConversionError::SizeOverflow)?;
    let mut samples = Vec::with_capacity(output_len);
    let mut errors = vec![0.0f64; matrix.output_channels];
    let mut random = Random64::new(dither_seed(policy.dither));
    let mut peak = 0.0f64;
    let mut clipped = 0usize;
    let mut error_energy = 0.0f64;

    for (frame_index, frame) in input.chunks(matrix.input_channels).enumerate() {
        for (channel, sample) in frame.iter().copied().enumerate() {
            if !sample.is_finite() {
                return Err(PcmConversionError::NonFiniteSample {
                    index: frame_index * matrix.input_channels + channel,
                });
            }
        }
        for (output_channel, shaped_error) in errors.iter_mut().enumerate() {
            let mapped = matrix.map(frame, output_channel);
            peak = peak.max(mapped.abs());
            clipped += usize::from(!(-1.0..=1.0).contains(&mapped));
            let feedback = dither_feedback(policy.dither);
            let shaped = mapped + *shaped_error * feedback;
            let dither = dither_lsb(policy.dither, &mut random);
            let code = (shaped * 32_768.0 + dither)
                .round()
                .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16;
            let reconstructed = f64::from(code) / 32_768.0;
            *shaped_error = shaped - reconstructed;
            let error = reconstructed - mapped;
            error_energy += error * error;
            samples.push(code);
        }
    }
    let quantization_error_rms = if output_len == 0 {
        0.0
    } else {
        (error_energy / output_len as f64).sqrt()
    };
    Ok(Pcm16Conversion {
        samples,
        report: PcmConversionReport {
            frames,
            input_channels: matrix.input_channels,
            output_channels: matrix.output_channels,
            peak_before_quantization: peak,
            clipped_samples: clipped,
            quantization_error_rms,
            dither: policy.dither,
        },
    })
}

fn validate_policy(policy: QuantizationPolicy) -> Result<(), PcmConversionError> {
    if policy.max_frames == 0 {
        return Err(PcmConversionError::FrameLimit {
            supplied: 0,
            maximum: 0,
        });
    }
    if let DitherPolicy::NoiseShapedTpdf { feedback, .. } = policy.dither
        && (!feedback.is_finite() || !(0.0..=0.95).contains(&feedback))
    {
        return Err(PcmConversionError::InvalidDither);
    }
    Ok(())
}

fn dither_seed(policy: DitherPolicy) -> u64 {
    match policy {
        DitherPolicy::None => 0,
        DitherPolicy::Tpdf { seed } | DitherPolicy::NoiseShapedTpdf { seed, .. } => seed,
    }
}

fn dither_feedback(policy: DitherPolicy) -> f64 {
    match policy {
        DitherPolicy::NoiseShapedTpdf { feedback, .. } => f64::from(feedback),
        DitherPolicy::None | DitherPolicy::Tpdf { .. } => 0.0,
    }
}

fn dither_lsb(policy: DitherPolicy, random: &mut Random64) -> f64 {
    match policy {
        DitherPolicy::None => 0.0,
        DitherPolicy::Tpdf { .. } | DitherPolicy::NoiseShapedTpdf { .. } => {
            random.unit() - random.unit()
        }
    }
}

struct Random64 {
    state: u64,
}

impl Random64 {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9e37_79b9_7f4a_7c15
            } else {
                seed
            },
        }
    }

    fn unit(&mut self) -> f64 {
        let mut value = self.state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.state = value;
        (value >> 11) as f64 / (1u64 << 53) as f64
    }
}
