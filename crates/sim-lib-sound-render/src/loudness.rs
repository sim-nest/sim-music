use std::{error::Error, fmt};

mod dsp;
mod validate;

use dsp::{measure_true_peak, weight_channels};
use validate::{validate_normalization, validate_spec};

const LOUDNESS_OFFSET_DB: f64 = -0.691;
const MOMENTARY_SECONDS: f64 = 0.400;
const MOMENTARY_STEP_SECONDS: f64 = 0.100;

/// Semantic channel position used by ITU-R BS.1770 energy weighting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoudnessChannel {
    /// Mono or front-center program channel.
    Center,
    /// Front-left channel.
    Left,
    /// Front-right channel.
    Right,
    /// Left surround channel, weighted by 1.41 in BS.1770.
    LeftSurround,
    /// Right surround channel, weighted by 1.41 in BS.1770.
    RightSurround,
    /// Low-frequency-effects channel, excluded by BS.1770.
    Lfe,
}

/// Ordered interleaved channel layout for loudness measurement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoudnessLayout {
    channels: Vec<LoudnessChannel>,
}

impl LoudnessLayout {
    /// Builds a bounded nonempty layout.
    pub fn new(channels: Vec<LoudnessChannel>) -> Result<Self, LoudnessError> {
        if channels.is_empty() || channels.len() > 32 {
            return Err(LoudnessError::InvalidPolicy {
                field: "channel layout",
                reason: "must contain between one and 32 channels",
            });
        }
        Ok(Self { channels })
    }

    /// Standard one-channel program layout.
    pub fn mono() -> Self {
        Self {
            channels: vec![LoudnessChannel::Center],
        }
    }

    /// Standard left/right program layout.
    pub fn stereo() -> Self {
        Self {
            channels: vec![LoudnessChannel::Left, LoudnessChannel::Right],
        }
    }

    /// Standard 5.1 order: left, right, center, LFE, left surround, right surround.
    pub fn five_point_one() -> Self {
        Self {
            channels: vec![
                LoudnessChannel::Left,
                LoudnessChannel::Right,
                LoudnessChannel::Center,
                LoudnessChannel::Lfe,
                LoudnessChannel::LeftSurround,
                LoudnessChannel::RightSurround,
            ],
        }
    }

    /// Returns the ordered interleaved channels.
    pub fn channels(&self) -> &[LoudnessChannel] {
        &self.channels
    }
}

/// Frequency weighting applied before gated loudness integration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrequencyWeighting {
    /// Two-stage ITU-R BS.1770 K-weighting (shelf plus RLB high-pass).
    ItuRBs1770K,
    /// No weighting, retained for calibration and controlled comparisons.
    Flat,
}

/// Block-gating policy for integrated loudness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GatingPolicy {
    /// EBU R128 absolute `-70 LUFS` and relative `-10 LU` gates.
    EbuR128,
    /// Explicit absolute and relative gate thresholds.
    AbsoluteRelative {
        /// Absolute block threshold in LUFS.
        absolute_lufs: f64,
        /// Relative threshold below absolute-gated program loudness, in LU.
        relative_lu: f64,
    },
    /// Integrate all complete momentary blocks without gating.
    None,
}

/// Bandlimited true-peak interpolation policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TruePeakPolicy {
    /// Integer oversampling factor; BS.1770 measurement conventionally uses four.
    pub oversample_factor: usize,
    /// Even Blackman-windowed sinc length.
    pub taps: usize,
    /// Hard interpolation multiply-accumulate ceiling.
    pub max_work: u64,
}

impl Default for TruePeakPolicy {
    fn default() -> Self {
        Self {
            oversample_factor: 4,
            taps: 24,
            max_work: 100_000_000,
        }
    }
}

/// Complete bounded policy for EBU/ITU loudness and true-peak measurement.
#[derive(Clone, Debug, PartialEq)]
pub struct LoudnessSpec {
    /// PCM sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Ordered interleaved channel layout.
    pub layout: LoudnessLayout,
    /// Frequency weighting applied independently to every channel.
    pub frequency_weighting: FrequencyWeighting,
    /// Integrated-loudness block gate.
    pub gating: GatingPolicy,
    /// Bandlimited true-peak interpolation policy.
    pub true_peak: TruePeakPolicy,
    /// Hard input-frame ceiling.
    pub max_frames: usize,
}

impl Default for LoudnessSpec {
    fn default() -> Self {
        Self {
            sample_rate_hz: 48_000,
            layout: LoudnessLayout::stereo(),
            frequency_weighting: FrequencyWeighting::ItuRBs1770K,
            gating: GatingPolicy::EbuR128,
            true_peak: TruePeakPolicy::default(),
            max_frames: 16_777_216,
        }
    }
}

/// One 400 ms EBU momentary block.
#[derive(Clone, Debug, PartialEq)]
pub struct MomentaryLoudness {
    /// First whole PCM frame in the block.
    pub start_frame: usize,
    /// BS.1770 channel-weighted mean-square energy.
    pub mean_square: f64,
    /// Loudness in LUFS, or `None` for digital silence.
    pub lufs: Option<f64>,
}

/// Sample-peak and interpolated true-peak evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct TruePeakReport {
    /// Largest absolute input sample.
    pub sample_peak: f64,
    /// Largest absolute bandlimited interpolated sample.
    pub true_peak: f64,
    /// True peak in dBTP, or `None` for digital silence.
    pub true_peak_dbtp: Option<f64>,
    /// Integer interpolation factor actually used.
    pub oversample_factor: usize,
    /// Interpolation multiply-accumulates charged by the report.
    pub work_units: u64,
}

/// Standards-named integrated, momentary, gating, and true-peak evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct LoudnessReport {
    /// Final gated integrated loudness in LUFS, or `None` when no block survives.
    pub integrated_lufs: Option<f64>,
    /// Loudness after the absolute gate and before the relative gate.
    pub absolute_gated_lufs: Option<f64>,
    /// Effective absolute gate in LUFS, when gating is enabled.
    pub absolute_gate_lufs: Option<f64>,
    /// Effective program-relative gate in LUFS, when it can be derived.
    pub relative_gate_lufs: Option<f64>,
    /// Complete 400 ms blocks at 100 ms spacing.
    pub momentary: Vec<MomentaryLoudness>,
    /// Blocks admitted by the final gate.
    pub gated_blocks: usize,
    /// Interpolated peak evidence over the unweighted source PCM.
    pub true_peak: TruePeakReport,
    /// Exact measurement policy.
    pub spec: LoudnessSpec,
}

/// Target and safety ceiling for transparent loudness normalization.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalizationSpec {
    /// Requested integrated output loudness in LUFS.
    pub target_lufs: f64,
    /// Review ceiling in dBTP; it is reported, never enforced by hidden limiting.
    pub max_true_peak_dbtp: f64,
    /// Maximum absolute gain change the caller permits, in decibels.
    pub max_abs_gain_db: f64,
}

/// Normalized PCM plus fully visible gain, ceiling, and clipping evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalizationReport {
    /// Gain-adjusted float PCM; samples are not clipped or limited.
    pub samples: Vec<f32>,
    /// Measurement before gain.
    pub input: LoudnessReport,
    /// Measurement after the exact applied gain.
    pub output: LoudnessReport,
    /// Gain implied by target minus measured integrated loudness.
    pub requested_gain_db: f64,
    /// Gain actually applied after the explicit gain bound.
    pub applied_gain_db: f64,
    /// Whether `max_abs_gain_db` constrained the requested gain.
    pub gain_limited: bool,
    /// Whether measured output true peak exceeds the review ceiling.
    pub true_peak_ceiling_exceeded: bool,
    /// Output float samples outside `[-1, 1]`.
    pub clipped_samples: usize,
}

/// Invalid loudness input, bound, or standards policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoudnessError {
    /// A named policy field violated its finite definition.
    InvalidPolicy {
        /// Rejected field.
        field: &'static str,
        /// Stable reason.
        reason: &'static str,
    },
    /// Interleaved samples ended mid-frame.
    MisalignedInput,
    /// A source sample was NaN or infinite.
    NonFiniteSample {
        /// Zero-based interleaved sample offset.
        index: usize,
    },
    /// Input exceeded the declared frame bound.
    FrameLimit {
        /// Whole frames supplied.
        supplied: usize,
        /// Maximum admitted frames.
        maximum: usize,
    },
    /// True-peak interpolation exceeded its deterministic work ceiling.
    WorkLimit {
        /// Required multiply-accumulates.
        required: u64,
        /// Policy ceiling.
        maximum: u64,
    },
    /// Normalization cannot derive gain from digital silence or a fully gated signal.
    UndefinedIntegratedLoudness,
    /// Size or work arithmetic overflowed.
    SizeOverflow,
}

impl fmt::Display for LoudnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPolicy { field, reason } => {
                write!(f, "invalid loudness {field}: {reason}")
            }
            Self::MisalignedInput => write!(f, "interleaved loudness input ends mid-frame"),
            Self::NonFiniteSample { index } => write!(f, "loudness sample {index} is not finite"),
            Self::FrameLimit { supplied, maximum } => {
                write!(
                    f,
                    "loudness input has {supplied} frames, exceeding {maximum}"
                )
            }
            Self::WorkLimit { required, maximum } => {
                write!(
                    f,
                    "true-peak interpolation needs {required} work, exceeding {maximum}"
                )
            }
            Self::UndefinedIntegratedLoudness => {
                write!(f, "integrated loudness is undefined for this signal")
            }
            Self::SizeOverflow => write!(f, "loudness size arithmetic overflowed"),
        }
    }
}

impl Error for LoudnessError {}

/// Measures complete 400 ms momentary blocks, gated integrated loudness, and
/// bandlimited true peak under one retained policy.
pub fn measure_loudness(
    input: &[f32],
    spec: LoudnessSpec,
) -> Result<LoudnessReport, LoudnessError> {
    validate_spec(&spec)?;
    let channels = spec.layout.channels.len();
    if !input.len().is_multiple_of(channels) {
        return Err(LoudnessError::MisalignedInput);
    }
    for (index, sample) in input.iter().copied().enumerate() {
        if !sample.is_finite() {
            return Err(LoudnessError::NonFiniteSample { index });
        }
    }
    let frames = input.len() / channels;
    if frames > spec.max_frames {
        return Err(LoudnessError::FrameLimit {
            supplied: frames,
            maximum: spec.max_frames,
        });
    }
    let weighted = weight_channels(input, &spec);
    let momentary = momentary_blocks(&weighted, &spec)?;
    let (
        absolute_gate_lufs,
        relative_gate_lufs,
        absolute_gated_lufs,
        integrated_lufs,
        gated_blocks,
    ) = integrate_blocks(&momentary, spec.gating);
    let true_peak = measure_true_peak(input, &spec)?;
    Ok(LoudnessReport {
        integrated_lufs,
        absolute_gated_lufs,
        absolute_gate_lufs,
        relative_gate_lufs,
        momentary,
        gated_blocks,
        true_peak,
        spec,
    })
}

/// Applies one visible scalar gain to reach the requested loudness, then
/// remeasures without clipping, limiting, or concealing a true-peak violation.
pub fn normalize_loudness(
    input: &[f32],
    loudness: LoudnessSpec,
    normalization: NormalizationSpec,
) -> Result<NormalizationReport, LoudnessError> {
    validate_normalization(normalization)?;
    let before = measure_loudness(input, loudness.clone())?;
    let integrated = before
        .integrated_lufs
        .ok_or(LoudnessError::UndefinedIntegratedLoudness)?;
    let requested_gain_db = normalization.target_lufs - integrated;
    let applied_gain_db = requested_gain_db.clamp(
        -normalization.max_abs_gain_db,
        normalization.max_abs_gain_db,
    );
    let gain_limited = (applied_gain_db - requested_gain_db).abs() > 1e-12;
    let gain = 10.0f64.powf(applied_gain_db / 20.0);
    let samples = input
        .iter()
        .map(|sample| (f64::from(*sample) * gain) as f32)
        .collect::<Vec<_>>();
    let clipped_samples = samples.iter().filter(|sample| sample.abs() > 1.0).count();
    let output = measure_loudness(&samples, loudness)?;
    let true_peak_ceiling_exceeded = output
        .true_peak
        .true_peak_dbtp
        .is_some_and(|peak| peak > normalization.max_true_peak_dbtp);
    Ok(NormalizationReport {
        samples,
        input: before,
        output,
        requested_gain_db,
        applied_gain_db,
        gain_limited,
        true_peak_ceiling_exceeded,
        clipped_samples,
    })
}

fn momentary_blocks(
    weighted: &[f64],
    spec: &LoudnessSpec,
) -> Result<Vec<MomentaryLoudness>, LoudnessError> {
    let channels = spec.layout.channels.len();
    let frames = weighted.len() / channels;
    let window = (f64::from(spec.sample_rate_hz) * MOMENTARY_SECONDS).round() as usize;
    let step = (f64::from(spec.sample_rate_hz) * MOMENTARY_STEP_SECONDS).round() as usize;
    if frames < window {
        return Ok(Vec::new());
    }
    let count = (frames - window) / step + 1;
    let mut blocks = Vec::with_capacity(count);
    for block in 0..count {
        let start = block.checked_mul(step).ok_or(LoudnessError::SizeOverflow)?;
        let mut energy = 0.0;
        for (channel, position) in spec.layout.channels.iter().enumerate() {
            let channel_energy = (start..start + window)
                .map(|frame| weighted[frame * channels + channel].powi(2))
                .sum::<f64>()
                / window as f64;
            energy += channel_weight(*position) * channel_energy;
        }
        blocks.push(MomentaryLoudness {
            start_frame: start,
            mean_square: energy,
            lufs: loudness_level(energy),
        });
    }
    Ok(blocks)
}

#[allow(clippy::type_complexity)]
fn integrate_blocks(
    blocks: &[MomentaryLoudness],
    policy: GatingPolicy,
) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>, usize) {
    if policy == GatingPolicy::None {
        let integrated = mean_energy(blocks.iter().map(|block| block.mean_square));
        return (None, None, integrated, integrated, blocks.len());
    }
    let (absolute, relative) = match policy {
        GatingPolicy::EbuR128 => (-70.0, -10.0),
        GatingPolicy::AbsoluteRelative {
            absolute_lufs,
            relative_lu,
        } => (absolute_lufs, relative_lu),
        GatingPolicy::None => unreachable!(),
    };
    let absolute_energies = blocks
        .iter()
        .filter(|block| block.lufs.is_some_and(|level| level > absolute))
        .map(|block| block.mean_square)
        .collect::<Vec<_>>();
    let absolute_gated = mean_energy(absolute_energies.iter().copied());
    let relative_gate = absolute_gated.map(|level| level + relative);
    let final_energies = blocks
        .iter()
        .filter(|block| {
            block.lufs.is_some_and(|level| {
                level > absolute && relative_gate.is_none_or(|relative| level > relative)
            })
        })
        .map(|block| block.mean_square)
        .collect::<Vec<_>>();
    let gated_blocks = final_energies.len();
    (
        Some(absolute),
        relative_gate,
        absolute_gated,
        mean_energy(final_energies.into_iter()),
        gated_blocks,
    )
}

fn channel_weight(channel: LoudnessChannel) -> f64 {
    match channel {
        LoudnessChannel::LeftSurround | LoudnessChannel::RightSurround => 1.41,
        LoudnessChannel::Lfe => 0.0,
        LoudnessChannel::Center | LoudnessChannel::Left | LoudnessChannel::Right => 1.0,
    }
}

fn mean_energy(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, count) = values.fold((0.0, 0usize), |(sum, count), value| {
        (sum + value, count + 1)
    });
    (count > 0)
        .then(|| sum / count as f64)
        .and_then(loudness_level)
}

fn loudness_level(mean_square: f64) -> Option<f64> {
    (mean_square > 0.0).then(|| LOUDNESS_OFFSET_DB + 10.0 * mean_square.log10())
}
