//! Tuning-anchored constant-Q analysis and explicit chroma folding.

use sim_lib_numbers_signal::{
    Normalization, PaddingPolicy, SignConvention, SignalBuffer, SignalView, SpectrumPacking,
    TransformKind, TransformPlan, WindowFunction, WindowSampling, WindowSpec, transform,
};
use sim_lib_pitch_core::Pitch;
use sim_lib_sound_core::Frequency;
use sim_lib_sound_tuning::Tuning;

use crate::{AudioTransformError, invalid};

/// Value derived from each phase-preserving constant-Q coefficient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CqtWeighting {
    /// Coherent-gain-corrected coefficient magnitude.
    Magnitude,
    /// Squared coherent-gain-corrected magnitude.
    Power,
    /// Natural logarithm of power after applying a positive floor.
    LogPower {
        /// Strictly positive power floor applied before the logarithm.
        floor: f64,
    },
}

/// Complete constant-Q grid, framing, convention, and work policy.
#[derive(Clone, Debug, PartialEq)]
pub struct CqtPlan {
    /// Distance between adjacent frame timestamps, in samples.
    pub hop: usize,
    /// Lowest requested geometric center frequency, in hertz.
    pub min_frequency_hz: f64,
    /// Highest requested geometric center frequency, in hertz.
    pub max_frequency_hz: f64,
    /// Number of geometric bins per octave.
    pub bins_per_octave: u32,
    /// Window applied to every variable-length kernel.
    pub window: WindowSpec,
    /// Whether the frame timestamp is the center rather than the start.
    pub center: bool,
    /// Admission policy at finite signal boundaries.
    pub padding: PaddingPolicy,
    /// Complex-exponential sign used by delegated FFT plans.
    pub phase: SignConvention,
    /// Magnitude, power, or log-power output policy.
    pub weighting: CqtWeighting,
    /// Longest admitted variable window.
    pub max_window: usize,
    /// Maximum admitted frame count.
    pub max_frames: usize,
    /// Maximum admitted geometric bin count.
    pub max_bins: usize,
    /// Maximum sum of variable-window samples transformed across the request.
    pub max_work: u64,
}

impl Default for CqtPlan {
    fn default() -> Self {
        let mut window = WindowSpec::new(WindowFunction::Hann);
        window.sampling = WindowSampling::Periodic;
        Self {
            hop: 512,
            min_frequency_hz: 55.0,
            max_frequency_hz: 3_520.0,
            bins_per_octave: 12,
            window,
            center: true,
            padding: PaddingPolicy::Zero,
            phase: SignConvention::NegativeForward,
            weighting: CqtWeighting::Power,
            max_window: 32_768,
            max_frames: 16_384,
            max_bins: 256,
            max_work: 1_000_000_000,
        }
    }
}

/// Tuning facts retained with constant-Q and chroma outputs.
#[derive(Clone, Debug, PartialEq)]
pub struct CqtReference {
    /// Stable tuning-system name.
    pub tuning: String,
    /// Anchor pitch supplied by the tuning.
    pub pitch: Pitch,
    /// Anchor frequency supplied by the tuning.
    pub frequency: Frequency,
    /// Equal divisions reported by the tuning for one octave.
    pub divisions: u32,
    /// Tuning degree occupied by the anchor pitch.
    pub degree: u32,
}

/// One constant-Q bin with requested and realized kernel evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct CqtBin {
    /// Geometric bin offset from the tuning reference.
    pub reference_offset: i32,
    /// Requested tuning-anchored center frequency.
    pub center_frequency: Frequency,
    /// Frequency of the delegated FFT bin actually evaluated.
    pub evaluated_frequency: Frequency,
    /// Variable FFT/window length.
    pub window_len: usize,
    /// Coefficient value after the declared weighting policy.
    pub value: f64,
}

/// Constant-Q values at one frame timestamp.
#[derive(Clone, Debug, PartialEq)]
pub struct CqtFrame {
    /// Zero-based frame index.
    pub index: usize,
    /// Signed frame timestamp in source-sample coordinates.
    pub onset_sample: i64,
    /// Tuning-ordered constant-Q bins.
    pub bins: Vec<CqtBin>,
}

/// Bounded-work evidence for a complete constant-Q request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CqtReport {
    /// Number of frame timestamps evaluated.
    pub frames: usize,
    /// Number of geometric bins evaluated per frame.
    pub bins: usize,
    /// Sum of variable-window lengths transformed across all frames.
    pub work_units: u64,
    /// Caller-declared work ceiling.
    pub work_limit: u64,
}

/// Tuning-anchored, bounded constant-Q output.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstantQ {
    /// Exact plan used for the request.
    pub plan: CqtPlan,
    /// Source sample rate in hertz.
    pub sample_rate: u32,
    /// Number of source samples.
    pub original_len: usize,
    /// Tuning and anchor facts used to construct the grid.
    pub reference: CqtReference,
    /// Constant-Q frames.
    pub frames: Vec<CqtFrame>,
    /// Bounded-work evidence.
    pub report: CqtReport,
}

#[derive(Clone, Debug)]
struct Kernel {
    reference_offset: i32,
    center_frequency: f64,
    evaluated_frequency: f64,
    fft_bin: usize,
    window: Vec<f64>,
    coherent_gain: f64,
}

/// Computes variable-window constant-Q bins through numbers-signal real FFTs.
pub fn constant_q(
    samples: &[f32],
    sample_rate: u32,
    tuning: &dyn Tuning,
    plan: &CqtPlan,
) -> Result<ConstantQ, AudioTransformError> {
    validate_cqt_plan(sample_rate, plan)?;
    for (index, sample) in samples.iter().copied().enumerate() {
        if !sample.is_finite() {
            return Err(sim_lib_numbers_signal::SignalError::NonFinite {
                index,
                component: "value",
            }
            .into());
        }
    }
    let reference = cqt_reference(tuning)?;
    let kernels = build_kernels(sample_rate, reference.frequency.0, plan)?;
    let frame_times = cqt_frame_times(samples.len(), kernels_max_len(&kernels), plan)?;
    let work_per_frame = kernels.iter().try_fold(0u64, |sum, kernel| {
        sum.checked_add(u64::try_from(kernel.window.len()).unwrap_or(u64::MAX))
            .ok_or_else(|| work_limit(u64::MAX, plan.max_work))
    })?;
    let work_units = work_per_frame
        .checked_mul(u64::try_from(frame_times.len()).unwrap_or(u64::MAX))
        .ok_or_else(|| work_limit(u64::MAX, plan.max_work))?;
    if work_units > plan.max_work {
        return Err(work_limit(work_units, plan.max_work));
    }

    let mut frames = Vec::with_capacity(frame_times.len());
    for (index, timestamp) in frame_times.into_iter().enumerate() {
        let mut bins = Vec::with_capacity(kernels.len());
        for kernel in &kernels {
            let start = if plan.center {
                timestamp
                    - i64::try_from(kernel.window.len() / 2).map_err(|_| {
                        invalid("CQT window", "window exceeds signed sample coordinates")
                    })?
            } else {
                timestamp
            };
            let input = windowed_samples(samples, start, &kernel.window, plan.padding)?;
            let mut transform_plan =
                TransformPlan::new(TransformKind::RealFft, kernel.window.len());
            transform_plan.normalization = Normalization::Forward;
            transform_plan.sign = plan.phase;
            transform_plan.packing = SpectrumPacking::HermitianHalf;
            let SignalBuffer::Complex(spectrum) =
                transform(&transform_plan, SignalView::Real(&input))?
            else {
                unreachable!("a real FFT returns complex bins")
            };
            let (real, imaginary) = spectrum.as_slice()[kernel.fft_bin];
            let one_sided_scale = if 2 * kernel.fft_bin == kernel.window.len() {
                1.0
            } else {
                2.0
            };
            let magnitude = one_sided_scale * real.hypot(imaginary) / kernel.coherent_gain;
            let value = apply_weighting(magnitude, plan.weighting);
            bins.push(CqtBin {
                reference_offset: kernel.reference_offset,
                center_frequency: Frequency(kernel.center_frequency),
                evaluated_frequency: Frequency(kernel.evaluated_frequency),
                window_len: kernel.window.len(),
                value,
            });
        }
        frames.push(CqtFrame {
            index,
            onset_sample: timestamp,
            bins,
        });
    }
    Ok(ConstantQ {
        plan: plan.clone(),
        sample_rate,
        original_len: samples.len(),
        reference,
        report: CqtReport {
            frames: frames.len(),
            bins: kernels.len(),
            work_units,
            work_limit: plan.max_work,
        },
        frames,
    })
}

fn validate_cqt_plan(sample_rate: u32, plan: &CqtPlan) -> Result<(), AudioTransformError> {
    if sample_rate == 0 {
        return Err(invalid("sample rate", "sample rate must be positive"));
    }
    if plan.hop == 0 || plan.bins_per_octave == 0 {
        return Err(invalid(
            "CQT grid",
            "hop and bins per octave must be positive",
        ));
    }
    if !plan.min_frequency_hz.is_finite()
        || !plan.max_frequency_hz.is_finite()
        || plan.min_frequency_hz <= 0.0
        || plan.min_frequency_hz > plan.max_frequency_hz
        || plan.max_frequency_hz > f64::from(sample_rate) / 2.0
    {
        return Err(invalid(
            "CQT frequency range",
            "frequencies must be finite, positive, ordered, and at or below Nyquist",
        ));
    }
    if plan.max_window < 2 || plan.max_frames == 0 || plan.max_bins == 0 || plan.max_work == 0 {
        return Err(invalid(
            "CQT limits",
            "all deterministic limits must be positive",
        ));
    }
    if plan.center && plan.padding != PaddingPolicy::Zero {
        return Err(invalid(
            "CQT center",
            "centered kernels require zero padding",
        ));
    }
    if let CqtWeighting::LogPower { floor } = plan.weighting
        && (!floor.is_finite() || floor <= 0.0)
    {
        return Err(invalid(
            "CQT log floor",
            "log-power floor must be positive and finite",
        ));
    }
    Ok(())
}

fn cqt_reference(tuning: &dyn Tuning) -> Result<CqtReference, AudioTransformError> {
    let (pitch, frequency) = tuning.reference();
    if !frequency.0.is_finite() || frequency.0 <= 0.0 || tuning.divisions() == 0 {
        return Err(invalid(
            "tuning reference",
            "tuning must expose a positive finite reference and divisions",
        ));
    }
    let degree = tuning
        .degree_of_pitch(pitch)
        .map_err(|_| invalid("tuning reference", "anchor pitch has no tuning degree"))?;
    Ok(CqtReference {
        tuning: tuning.name().to_owned(),
        pitch,
        frequency,
        divisions: tuning.divisions(),
        degree: degree.index,
    })
}

fn build_kernels(
    sample_rate: u32,
    reference_hz: f64,
    plan: &CqtPlan,
) -> Result<Vec<Kernel>, AudioTransformError> {
    let bins_per_octave = f64::from(plan.bins_per_octave);
    let minimum = (bins_per_octave * (plan.min_frequency_hz / reference_hz).log2()).ceil();
    let maximum = (bins_per_octave * (plan.max_frequency_hz / reference_hz).log2()).floor();
    if minimum < f64::from(i32::MIN) || maximum > f64::from(i32::MAX) || minimum > maximum {
        return Err(invalid(
            "CQT grid",
            "frequency range produces no bounded integer grid",
        ));
    }
    let first = minimum as i32;
    let last = maximum as i32;
    let count = usize::try_from(i64::from(last) - i64::from(first) + 1)
        .map_err(|_| invalid("CQT grid", "bin count exceeds platform limits"))?;
    if count > plan.max_bins {
        return Err(AudioTransformError::WorkLimit {
            resource: "CQT bins",
            required: count as u64,
            maximum: plan.max_bins as u64,
        });
    }
    let quality = (2.0_f64.powf(1.0 / bins_per_octave) - 1.0).recip();
    let fft_bin = quality.ceil() as usize;
    let mut kernels = Vec::with_capacity(count);
    for offset in first..=last {
        let center = reference_hz * 2.0_f64.powf(f64::from(offset) / bins_per_octave);
        let desired = (fft_bin as f64 * f64::from(sample_rate) / center)
            .round()
            .max((2 * fft_bin) as f64) as usize;
        if desired > plan.max_window {
            return Err(AudioTransformError::WorkLimit {
                resource: "CQT window samples",
                required: desired as u64,
                maximum: plan.max_window as u64,
            });
        }
        let generated = plan.window.generate(desired)?;
        let coherent_gain = generated.metrics.coherent_gain.abs();
        if !coherent_gain.is_finite() || coherent_gain <= f64::EPSILON {
            return Err(invalid(
                "CQT window",
                "window coherent gain must be finite and nonzero",
            ));
        }
        kernels.push(Kernel {
            reference_offset: offset,
            center_frequency: center,
            evaluated_frequency: fft_bin as f64 * f64::from(sample_rate) / desired as f64,
            fft_bin,
            window: generated.samples,
            coherent_gain,
        });
    }
    Ok(kernels)
}

fn cqt_frame_times(
    len: usize,
    longest_window: usize,
    plan: &CqtPlan,
) -> Result<Vec<i64>, AudioTransformError> {
    if len == 0 {
        return Ok(Vec::new());
    }
    let count = if plan.padding == PaddingPolicy::Reject {
        if len < longest_window {
            return Err(invalid(
                "CQT padding",
                "input is shorter than the longest kernel",
            ));
        }
        1 + (len - longest_window) / plan.hop
    } else {
        1 + (len - 1) / plan.hop
    };
    if count > plan.max_frames {
        return Err(AudioTransformError::WorkLimit {
            resource: "CQT frames",
            required: count as u64,
            maximum: plan.max_frames as u64,
        });
    }
    (0..count)
        .map(|index| {
            index
                .checked_mul(plan.hop)
                .and_then(|sample| i64::try_from(sample).ok())
                .ok_or_else(|| invalid("CQT frame layout", "sample coordinate overflowed"))
        })
        .collect()
}

fn kernels_max_len(kernels: &[Kernel]) -> usize {
    kernels
        .iter()
        .map(|kernel| kernel.window.len())
        .max()
        .unwrap_or(0)
}

fn windowed_samples(
    samples: &[f32],
    start: i64,
    window: &[f64],
    padding: PaddingPolicy,
) -> Result<Vec<f64>, AudioTransformError> {
    let mut input = Vec::with_capacity(window.len());
    for (offset, coefficient) in window.iter().copied().enumerate() {
        let coordinate = start
            .checked_add(i64::try_from(offset).map_err(|_| {
                invalid("CQT frame layout", "window coordinate exceeds signed range")
            })?)
            .ok_or_else(|| invalid("CQT frame layout", "window coordinate overflowed"))?;
        let sample = usize::try_from(coordinate)
            .ok()
            .and_then(|index| samples.get(index))
            .copied();
        match (sample, padding) {
            (Some(value), _) => input.push(f64::from(value) * coefficient),
            (None, PaddingPolicy::Zero) => input.push(0.0),
            (None, PaddingPolicy::Reject) => {
                return Err(invalid(
                    "CQT padding",
                    "a kernel crosses the finite input boundary",
                ));
            }
        }
    }
    Ok(input)
}

fn apply_weighting(magnitude: f64, weighting: CqtWeighting) -> f64 {
    match weighting {
        CqtWeighting::Magnitude => magnitude,
        CqtWeighting::Power => magnitude * magnitude,
        CqtWeighting::LogPower { floor } => (magnitude * magnitude).max(floor).ln(),
    }
}

fn work_limit(required: u64, maximum: u64) -> AudioTransformError {
    AudioTransformError::WorkLimit {
        resource: "CQT window samples",
        required,
        maximum,
    }
}
