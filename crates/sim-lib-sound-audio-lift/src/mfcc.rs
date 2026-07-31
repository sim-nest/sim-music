//! Policy-complete mel, Bark, and ERB cepstral coefficients.

use crate::{AudioTransformError, Filterbank, FilterbankPlan, Stft, invalid};

/// Whether filterbank inputs are magnitude or power spectra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpectralEnergy {
    /// Use complex magnitude.
    Magnitude,
    /// Use squared complex magnitude.
    Power,
}

/// DCT-II scaling applied to filterbank log energies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DctNormalization {
    /// Preserve the unscaled DCT-II sum.
    None,
    /// Use the orthonormal DCT-II convention.
    Orthonormal,
}

/// Normalization applied after DCT and optional liftering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CepstralNormalization {
    /// Preserve coefficients.
    None,
    /// Subtract the across-frame mean of each coefficient.
    Mean,
    /// Subtract mean and divide by standard deviation with a declared floor.
    MeanVariance {
        /// Minimum admitted variance before square root.
        variance_floor: f64,
    },
}

/// Full MFCC policy.
#[derive(Clone, Debug, PartialEq)]
pub struct MfccPlan {
    /// Perceptual filterbank policy, including mel/Bark/ERB selection.
    pub filterbank: FilterbankPlan,
    /// Magnitude or power input to the filterbank.
    pub energy: SpectralEnergy,
    /// Positive floor applied before the natural logarithm.
    pub log_floor: f64,
    /// Number of DCT coefficients retained.
    pub coefficients: usize,
    /// DCT-II scaling convention.
    pub dct_normalization: DctNormalization,
    /// Optional positive sinusoidal lifter parameter.
    pub lifter: Option<f64>,
    /// Across-frame cepstral normalization policy.
    pub normalization: CepstralNormalization,
    /// Maximum band accumulation plus DCT multiply-add work.
    pub max_work: u64,
}

impl Default for MfccPlan {
    fn default() -> Self {
        Self {
            filterbank: FilterbankPlan::default(),
            energy: SpectralEnergy::Power,
            log_floor: 1e-10,
            coefficients: 13,
            dct_normalization: DctNormalization::Orthonormal,
            lifter: Some(22.0),
            normalization: CepstralNormalization::Mean,
            max_work: 100_000_000,
        }
    }
}

/// One MFCC vector tied to its STFT timestamp.
#[derive(Clone, Debug, PartialEq)]
pub struct MfccFrame {
    /// Source STFT frame index.
    pub index: usize,
    /// Signed source-sample coordinate of the frame start.
    pub onset_sample: i64,
    /// Retained cepstral coefficients.
    pub coefficients: Vec<f64>,
}

/// MFCC output retaining sample rate, scale, floors, transforms, and work.
#[derive(Clone, Debug, PartialEq)]
pub struct Mfcc {
    /// Full source sample rate rather than an inferred or default rate.
    pub sample_rate: u32,
    /// Complete MFCC policy.
    pub plan: MfccPlan,
    /// Concrete filterbank used for the analysis.
    pub filterbank: Filterbank,
    /// Timestamped coefficient vectors.
    pub frames: Vec<MfccFrame>,
    /// Charged band and DCT work.
    pub work_used: u64,
}

/// Computes policy-complete MFCCs from an existing STFT.
pub fn mfcc(analysis: &Stft, plan: &MfccPlan) -> Result<Mfcc, AudioTransformError> {
    validate_mfcc(plan)?;
    let filterbank = Filterbank::new(analysis.sample_rate, analysis.plan.frame, &plan.filterbank)?;
    let per_frame = filterbank
        .plan
        .bands
        .checked_mul(filterbank.fft_size / 2 + 1)
        .and_then(|value| value.checked_add(plan.coefficients.checked_mul(filterbank.plan.bands)?))
        .ok_or_else(|| work_limit(u64::MAX, plan.max_work))?;
    let required = per_frame
        .checked_mul(analysis.frames.len())
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| work_limit(u64::MAX, plan.max_work))?;
    if required > plan.max_work {
        return Err(work_limit(required, plan.max_work));
    }
    let mut frames = analysis
        .frames
        .iter()
        .map(|frame| frame_mfcc(frame, &filterbank, plan))
        .collect::<Result<Vec<_>, AudioTransformError>>()?;
    normalize_cepstra(&mut frames, plan.normalization);
    Ok(Mfcc {
        sample_rate: analysis.sample_rate,
        plan: plan.clone(),
        filterbank,
        frames,
        work_used: required,
    })
}

fn frame_mfcc(
    frame: &crate::StftFrame,
    filterbank: &Filterbank,
    plan: &MfccPlan,
) -> Result<MfccFrame, AudioTransformError> {
    let energy = frame
        .bins
        .iter()
        .map(|(real, imaginary)| match plan.energy {
            SpectralEnergy::Magnitude => real.hypot(*imaginary),
            SpectralEnergy::Power => real * real + imaginary * imaginary,
        })
        .collect::<Vec<_>>();
    let log_energy = filterbank
        .apply(&energy)?
        .into_iter()
        .map(|value| value.max(plan.log_floor).ln())
        .collect::<Vec<_>>();
    let mut coefficients = dct(&log_energy, plan);
    apply_lifter(&mut coefficients, plan.lifter);
    Ok(MfccFrame {
        index: frame.index,
        onset_sample: frame.onset_sample,
        coefficients,
    })
}

fn validate_mfcc(plan: &MfccPlan) -> Result<(), AudioTransformError> {
    if !plan.log_floor.is_finite()
        || plan.log_floor <= 0.0
        || plan.coefficients == 0
        || plan.coefficients > plan.filterbank.bands
        || plan.max_work == 0
        || plan
            .lifter
            .is_some_and(|value| !value.is_finite() || value <= 0.0)
        || matches!(
            plan.normalization,
            CepstralNormalization::MeanVariance { variance_floor }
                if !variance_floor.is_finite() || variance_floor <= 0.0
        )
    {
        return Err(invalid(
            "MFCC plan",
            "log floor, coefficient count, lifter, normalization, and work bounds are invalid",
        ));
    }
    Ok(())
}

fn dct(values: &[f64], plan: &MfccPlan) -> Vec<f64> {
    let length = values.len() as f64;
    (0..plan.coefficients)
        .map(|coefficient| {
            let sum = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    value
                        * (std::f64::consts::PI / length
                            * (index as f64 + 0.5)
                            * coefficient as f64)
                            .cos()
                })
                .sum::<f64>();
            match plan.dct_normalization {
                DctNormalization::None => sum,
                DctNormalization::Orthonormal if coefficient == 0 => sum / length.sqrt(),
                DctNormalization::Orthonormal => sum * (2.0 / length).sqrt(),
            }
        })
        .collect()
}

fn apply_lifter(coefficients: &mut [f64], lifter: Option<f64>) {
    let Some(lifter) = lifter else { return };
    for (index, coefficient) in coefficients.iter_mut().enumerate() {
        *coefficient *= 1.0 + lifter / 2.0 * (std::f64::consts::PI * index as f64 / lifter).sin();
    }
}

fn normalize_cepstra(frames: &mut [MfccFrame], policy: CepstralNormalization) {
    if frames.is_empty() || policy == CepstralNormalization::None {
        return;
    }
    let coefficients = frames[0].coefficients.len();
    let means = (0..coefficients)
        .map(|index| {
            frames
                .iter()
                .map(|frame| frame.coefficients[index])
                .sum::<f64>()
                / frames.len() as f64
        })
        .collect::<Vec<_>>();
    let divisor = match policy {
        CepstralNormalization::None => return,
        CepstralNormalization::Mean => vec![1.0; coefficients],
        CepstralNormalization::MeanVariance { variance_floor } => (0..coefficients)
            .map(|index| {
                (frames
                    .iter()
                    .map(|frame| (frame.coefficients[index] - means[index]).powi(2))
                    .sum::<f64>()
                    / frames.len() as f64)
                    .max(variance_floor)
                    .sqrt()
            })
            .collect(),
    };
    for frame in frames {
        for (index, value) in frame.coefficients.iter_mut().enumerate() {
            *value = (*value - means[index]) / divisor[index];
        }
    }
}

fn work_limit(required: u64, maximum: u64) -> AudioTransformError {
    AudioTransformError::WorkLimit {
        resource: "MFCC analysis",
        required,
        maximum,
    }
}
