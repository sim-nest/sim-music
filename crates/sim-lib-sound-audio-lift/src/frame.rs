//! Framed Fourier analysis with explicit overlap-add evidence.

use sim_lib_numbers_signal::{
    Direction, Normalization, PaddingPolicy, SignConvention, SignalBuffer, SignalError, SignalView,
    SpectrumPacking, TransformKind, TransformPlan, WindowFunction, WindowSampling, WindowSpec,
    transform,
};
use sim_lib_sound_spectrum::{Spectrum, SpectrumError};
use thiserror::Error;

/// Whether an STFT may only be analyzed or must be overlap-add reconstructable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StftOverlapPolicy {
    /// Admit analysis without promising an inverse.
    AnalysisOnly,
    /// Require the analysis/synthesis window product to be constant by hop residue.
    RequireCola {
        /// Relative and absolute tolerance used by the COLA decision.
        tolerance: f64,
    },
}

/// Complete framed-transform policy and deterministic resource ceilings.
#[derive(Clone, Debug, PartialEq)]
pub struct StftPlan {
    /// Number of samples in every frame.
    pub frame: usize,
    /// Number of samples advanced between frame starts.
    pub hop: usize,
    /// Window multiplied into samples before the forward transform.
    pub analysis_window: WindowSpec,
    /// Window multiplied into samples after the inverse transform.
    pub synthesis_window: WindowSpec,
    /// Whether zero-padding places each frame timestamp at its center.
    pub center: bool,
    /// Admission policy for samples beyond the finite input.
    pub padding: PaddingPolicy,
    /// Forward complex-exponential sign convention; the inverse uses its pair.
    pub phase: SignConvention,
    /// Scaling convention shared by the forward/inverse transform pair.
    pub normalization: Normalization,
    /// Reconstruction promise and COLA tolerance.
    pub overlap: StftOverlapPolicy,
    /// Maximum number of admitted frames.
    pub max_frames: usize,
    /// Maximum number of retained complex cells across all frames.
    pub max_cells: usize,
}

impl Default for StftPlan {
    fn default() -> Self {
        let mut window = WindowSpec::new(WindowFunction::Hann);
        window.sampling = WindowSampling::Periodic;
        Self {
            frame: 2_048,
            hop: 512,
            analysis_window: window.clone(),
            synthesis_window: window,
            center: true,
            padding: PaddingPolicy::Zero,
            phase: SignConvention::NegativeForward,
            normalization: Normalization::Forward,
            overlap: StftOverlapPolicy::RequireCola { tolerance: 1e-10 },
            max_frames: 16_384,
            max_cells: 16_777_216,
        }
    }
}

/// Constant-overlap-add evidence for one analysis/synthesis window pair.
#[derive(Clone, Debug, PartialEq)]
pub struct ColaReport {
    /// Whether every hop residue has the same positive finite gain.
    pub reconstructable: bool,
    /// Smallest overlap gain across hop residues.
    pub gain_min: f64,
    /// Largest overlap gain across hop residues.
    pub gain_max: f64,
    /// Tolerance used for the decision.
    pub tolerance: f64,
    /// Number of residue classes checked.
    pub residues: usize,
}

/// One phase-preserving STFT frame.
#[derive(Clone, Debug, PartialEq)]
pub struct StftFrame {
    /// Zero-based frame index.
    pub index: usize,
    /// Signed sample offset of the frame start relative to the unpadded input.
    pub onset_sample: i64,
    /// Sample coordinate represented by the frame center.
    pub center_sample: i64,
    /// Hermitian-half complex bins, including DC and Nyquist when present.
    pub bins: Vec<(f64, f64)>,
}

impl StftFrame {
    /// Projects this frame through the stable sound-domain spectrum adapter.
    pub fn spectrum(
        &self,
        sample_rate: u32,
        frame_size: usize,
    ) -> Result<Spectrum, AudioTransformError> {
        Ok(Spectrum::from_stft_bins(
            &self.bins,
            sample_rate,
            frame_size,
            self.onset_sample,
        )?)
    }
}

/// Phase-preserving STFT output with sufficient policy to invert it.
#[derive(Clone, Debug, PartialEq)]
pub struct Stft {
    /// Exact plan used to produce the frames.
    pub plan: StftPlan,
    /// Source sample rate in hertz.
    pub sample_rate: u32,
    /// Number of unpadded source samples.
    pub original_len: usize,
    /// Leading zero padding inserted before the source.
    pub left_padding: usize,
    /// Transform frames in chronological order.
    pub frames: Vec<StftFrame>,
    /// COLA evidence for the declared window pair.
    pub cola: ColaReport,
}

/// Failure from framed, constant-Q, or chroma analysis.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AudioTransformError {
    /// A named plan field violated a definition-level invariant.
    #[error("invalid {field}: {reason}")]
    InvalidPlan {
        /// Rejected field or policy family.
        field: &'static str,
        /// Stable explanation of the violated invariant.
        reason: &'static str,
    },
    /// A request exceeded its declared deterministic bound.
    #[error("{resource} needs {required} units, exceeding limit {maximum}")]
    WorkLimit {
        /// Bounded resource that was exceeded.
        resource: &'static str,
        /// Required units.
        required: u64,
        /// Caller-declared ceiling.
        maximum: u64,
    },
    /// The analysis/synthesis window pair failed the declared COLA check.
    #[error("analysis/synthesis windows fail COLA: {report:?}")]
    NonCola {
        /// Failing, inspectable COLA evidence.
        report: ColaReport,
    },
    /// Synthesis was requested from an analysis-only plan.
    #[error("the STFT overlap policy is analysis-only")]
    AnalysisOnly,
    /// A frame collection no longer matches its retained plan and source length.
    #[error("STFT frame layout is inconsistent with its retained plan")]
    FrameLayout,
    /// An original sample has zero synthesis gain and cannot be reconstructed.
    #[error("sample {sample} has no finite nonzero overlap-add gain")]
    UncoveredSample {
        /// Zero-based sample within the original signal.
        sample: usize,
    },
    /// A delegated generic signal operation failed.
    #[error(transparent)]
    Signal(#[from] SignalError),
    /// Projection into the stable sound spectrum descriptor failed.
    #[error(transparent)]
    Spectrum(#[from] SpectrumError),
}

/// Checks constant overlap-add for the plan's analysis/synthesis window pair.
pub fn cola_report(plan: &StftPlan) -> Result<ColaReport, AudioTransformError> {
    validate_stft_plan(plan)?;
    let analysis = plan.analysis_window.generate(plan.frame)?;
    let synthesis = plan.synthesis_window.generate(plan.frame)?;
    let tolerance = match plan.overlap {
        StftOverlapPolicy::AnalysisOnly => 1e-10,
        StftOverlapPolicy::RequireCola { tolerance } => tolerance,
    };
    let gains = (0..plan.hop)
        .map(|residue| {
            (residue..plan.frame)
                .step_by(plan.hop)
                .map(|index| analysis.samples[index] * synthesis.samples[index])
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let gain_min = gains.iter().copied().fold(f64::INFINITY, f64::min);
    let gain_max = gains.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let scale = gain_min.abs().max(gain_max.abs()).max(1.0);
    let reconstructable = gain_min.is_finite()
        && gain_max.is_finite()
        && gain_min > tolerance
        && gain_max - gain_min <= tolerance * scale;
    Ok(ColaReport {
        reconstructable,
        gain_min,
        gain_max,
        tolerance,
        residues: plan.hop,
    })
}

/// Computes a bounded, phase-preserving real STFT through numbers-signal.
pub fn stft(
    samples: &[f32],
    sample_rate: u32,
    plan: &StftPlan,
) -> Result<Stft, AudioTransformError> {
    validate_stft_plan(plan)?;
    if sample_rate == 0 {
        return Err(invalid("sample rate", "sample rate must be positive"));
    }
    let cola = cola_report(plan)?;
    if matches!(plan.overlap, StftOverlapPolicy::RequireCola { .. }) && !cola.reconstructable {
        return Err(AudioTransformError::NonCola { report: cola });
    }
    for (index, sample) in samples.iter().copied().enumerate() {
        if !sample.is_finite() {
            return Err(SignalError::NonFinite {
                index,
                component: "value",
            }
            .into());
        }
    }
    let (left_padding, starts) = frame_layout(samples.len(), plan)?;
    admit_stft(starts.len(), plan)?;
    let window = plan.analysis_window.generate(plan.frame)?;
    let mut frames = Vec::with_capacity(starts.len());
    for (index, start) in starts.into_iter().enumerate() {
        let mut input = vec![0.0; plan.frame];
        for (offset, value) in input.iter_mut().enumerate() {
            let padded = start.checked_add(offset).ok_or_else(|| {
                invalid("frame layout", "sample coordinate arithmetic overflowed")
            })?;
            if let Some(source) = padded
                .checked_sub(left_padding)
                .filter(|at| *at < samples.len())
            {
                *value = f64::from(samples[source]);
            }
            *value *= window.samples[offset];
        }
        let mut transform_plan = TransformPlan::new(TransformKind::RealFft, plan.frame);
        transform_plan.normalization = plan.normalization;
        transform_plan.sign = plan.phase;
        transform_plan.packing = SpectrumPacking::HermitianHalf;
        let SignalBuffer::Complex(bins) = transform(&transform_plan, SignalView::Real(&input))?
        else {
            unreachable!("a real FFT returns complex bins")
        };
        let onset = signed_offset(start, left_padding)?;
        let half = i64::try_from(plan.frame / 2)
            .map_err(|_| invalid("frame", "frame size exceeds signed sample coordinates"))?;
        frames.push(StftFrame {
            index,
            onset_sample: onset,
            center_sample: onset + half,
            bins: bins.as_slice().to_vec(),
        });
    }
    Ok(Stft {
        plan: plan.clone(),
        sample_rate,
        original_len: samples.len(),
        left_padding,
        frames,
        cola,
    })
}

/// Reconstructs PCM only when the retained window pair passed COLA.
pub fn istft(analysis: &Stft) -> Result<Vec<f32>, AudioTransformError> {
    match analysis.plan.overlap {
        StftOverlapPolicy::AnalysisOnly => return Err(AudioTransformError::AnalysisOnly),
        StftOverlapPolicy::RequireCola { .. } if !analysis.cola.reconstructable => {
            return Err(AudioTransformError::NonCola {
                report: analysis.cola.clone(),
            });
        }
        StftOverlapPolicy::RequireCola { .. } => {}
    }
    let (_, expected_starts) = frame_layout(analysis.original_len, &analysis.plan)?;
    if expected_starts.len() != analysis.frames.len() {
        return Err(AudioTransformError::FrameLayout);
    }
    let synthesis = analysis
        .plan
        .synthesis_window
        .generate(analysis.plan.frame)?;
    let analysis_window = analysis
        .plan
        .analysis_window
        .generate(analysis.plan.frame)?;
    let output_len = expected_starts
        .last()
        .copied()
        .unwrap_or(0)
        .checked_add(analysis.plan.frame)
        .ok_or_else(|| invalid("frame layout", "output length overflowed"))?;
    let mut output = vec![0.0; output_len];
    let mut gain = vec![0.0; output_len];
    for ((frame, start), index) in analysis.frames.iter().zip(expected_starts).zip(0usize..) {
        if frame.index != index || frame.bins.len() != analysis.plan.frame / 2 + 1 {
            return Err(AudioTransformError::FrameLayout);
        }
        let mut transform_plan = TransformPlan::new(TransformKind::RealFft, analysis.plan.frame);
        transform_plan.direction = Direction::Inverse;
        transform_plan.normalization = analysis.plan.normalization;
        transform_plan.sign = analysis.plan.phase;
        transform_plan.packing = SpectrumPacking::HermitianHalf;
        let SignalBuffer::Real(samples) =
            transform(&transform_plan, SignalView::Complex(&frame.bins))?
        else {
            unreachable!("an inverse real FFT returns real samples")
        };
        for offset in 0..analysis.plan.frame {
            output[start + offset] += samples.as_slice()[offset] * synthesis.samples[offset];
            gain[start + offset] += analysis_window.samples[offset] * synthesis.samples[offset];
        }
    }
    let mut reconstructed = Vec::with_capacity(analysis.original_len);
    for source in 0..analysis.original_len {
        let padded = analysis.left_padding + source;
        let divisor = gain.get(padded).copied().unwrap_or(0.0);
        if !divisor.is_finite() || divisor.abs() <= analysis.cola.tolerance {
            return Err(AudioTransformError::UncoveredSample { sample: source });
        }
        let value = output[padded] / divisor;
        if !value.is_finite() {
            return Err(AudioTransformError::UncoveredSample { sample: source });
        }
        reconstructed.push(value as f32);
    }
    Ok(reconstructed)
}

fn validate_stft_plan(plan: &StftPlan) -> Result<(), AudioTransformError> {
    if plan.frame < 2 {
        return Err(invalid("frame", "frame size must be at least two"));
    }
    if plan.hop == 0 {
        return Err(invalid("hop", "hop size must be positive"));
    }
    if plan.hop > plan.frame {
        return Err(invalid("hop", "hop size must not exceed frame size"));
    }
    if plan.max_frames == 0 || plan.max_cells == 0 {
        return Err(invalid(
            "STFT limits",
            "frame and cell limits must be positive",
        ));
    }
    if plan.center && plan.padding != PaddingPolicy::Zero {
        return Err(invalid("center", "centered frames require zero padding"));
    }
    if let StftOverlapPolicy::RequireCola { tolerance } = plan.overlap
        && (!tolerance.is_finite() || tolerance <= 0.0)
    {
        return Err(invalid(
            "COLA tolerance",
            "tolerance must be positive and finite",
        ));
    }
    Ok(())
}

fn frame_layout(len: usize, plan: &StftPlan) -> Result<(usize, Vec<usize>), AudioTransformError> {
    if len == 0 {
        return Ok((if plan.center { plan.frame } else { 0 }, Vec::new()));
    }
    if plan.padding == PaddingPolicy::Reject {
        if len < plan.frame || !(len - plan.frame).is_multiple_of(plan.hop) {
            return Err(invalid(
                "padding",
                "reject padding requires complete, exactly aligned frames",
            ));
        }
        return Ok((0, (0..=len - plan.frame).step_by(plan.hop).collect()));
    }
    let left_padding = if plan.center { plan.frame } else { 0 };
    let last_source = left_padding
        .checked_add(len - 1)
        .ok_or_else(|| invalid("frame layout", "sample coordinate arithmetic overflowed"))?;
    let last_start = last_source / plan.hop * plan.hop;
    Ok((left_padding, (0..=last_start).step_by(plan.hop).collect()))
}

fn admit_stft(frames: usize, plan: &StftPlan) -> Result<(), AudioTransformError> {
    if frames > plan.max_frames {
        return Err(limit("STFT frames", frames, plan.max_frames));
    }
    let bins = plan.frame / 2 + 1;
    let cells = frames
        .checked_mul(bins)
        .ok_or_else(|| limit("STFT complex cells", usize::MAX, plan.max_cells))?;
    if cells > plan.max_cells {
        return Err(limit("STFT complex cells", cells, plan.max_cells));
    }
    Ok(())
}

fn signed_offset(start: usize, padding: usize) -> Result<i64, AudioTransformError> {
    let start = i64::try_from(start)
        .map_err(|_| invalid("frame layout", "frame offset exceeds signed coordinates"))?;
    let padding = i64::try_from(padding)
        .map_err(|_| invalid("frame layout", "padding exceeds signed coordinates"))?;
    Ok(start - padding)
}

pub(crate) fn invalid(field: &'static str, reason: &'static str) -> AudioTransformError {
    AudioTransformError::InvalidPlan { field, reason }
}

pub(crate) fn limit(
    resource: &'static str,
    required: usize,
    maximum: usize,
) -> AudioTransformError {
    AudioTransformError::WorkLimit {
        resource,
        required: u64::try_from(required).unwrap_or(u64::MAX),
        maximum: u64::try_from(maximum).unwrap_or(u64::MAX),
    }
}
