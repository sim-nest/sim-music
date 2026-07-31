//! Inspectable mel, Bark, and ERB triangular filterbanks.

use crate::{AudioTransformError, invalid};

/// Perceptual frequency scale used to place triangular filter bands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrequencyScale {
    /// HTK-style mel scale.
    Mel,
    /// Psychoacoustic Bark scale.
    Bark,
    /// Equivalent-rectangular-bandwidth rate.
    Erb,
}

/// Explicit triangular filterbank policy.
#[derive(Clone, Debug, PartialEq)]
pub struct FilterbankPlan {
    /// Perceptual spacing scale.
    pub scale: FrequencyScale,
    /// Number of triangular bands.
    pub bands: usize,
    /// Lowest band edge in hertz.
    pub minimum_hz: f64,
    /// Highest edge in hertz, or the full Nyquist range when absent.
    pub maximum_hz: Option<f64>,
    /// Maximum retained band-by-bin weights.
    pub max_weights: usize,
}

impl Default for FilterbankPlan {
    fn default() -> Self {
        Self {
            scale: FrequencyScale::Mel,
            bands: 40,
            minimum_hz: 20.0,
            maximum_hz: None,
            max_weights: 4_194_304,
        }
    }
}

/// Hertz edges of one perceptual triangular band.
#[derive(Clone, Debug, PartialEq)]
pub struct FilterBand {
    /// Lower zero crossing.
    pub lower_hz: f64,
    /// Unit-gain center.
    pub center_hz: f64,
    /// Upper zero crossing.
    pub upper_hz: f64,
}

/// Dense, inspectable triangular filterbank over real-FFT bins.
#[derive(Clone, Debug, PartialEq)]
pub struct Filterbank {
    /// Complete scale and range policy.
    pub plan: FilterbankPlan,
    /// Full source sample rate in hertz.
    pub sample_rate: u32,
    /// FFT frame size that fixes the bin frequencies.
    pub fft_size: usize,
    /// Hertz edges for every band.
    pub bands: Vec<FilterBand>,
    /// Band-major triangular weights, including DC and Nyquist bins.
    pub weights: Vec<Vec<f64>>,
}

impl Filterbank {
    /// Builds mel, Bark, or ERB triangular weights over a real-FFT grid.
    pub fn new(
        sample_rate: u32,
        fft_size: usize,
        plan: &FilterbankPlan,
    ) -> Result<Self, AudioTransformError> {
        validate_filterbank(sample_rate, fft_size, plan)?;
        let bins = fft_size / 2 + 1;
        let cells = plan
            .bands
            .checked_mul(bins)
            .ok_or_else(|| filterbank_limit(usize::MAX, plan.max_weights))?;
        if cells > plan.max_weights {
            return Err(filterbank_limit(cells, plan.max_weights));
        }
        let maximum_hz = plan
            .maximum_hz
            .unwrap_or_else(|| f64::from(sample_rate) / 2.0);
        let lower = scale_from_hz(plan.scale, plan.minimum_hz);
        let upper = scale_from_hz(plan.scale, maximum_hz);
        let points = (0..plan.bands + 2)
            .map(|index| {
                let ratio = index as f64 / (plan.bands + 1) as f64;
                hz_from_scale(plan.scale, lower + ratio * (upper - lower))
            })
            .collect::<Vec<_>>();
        let bands = points
            .windows(3)
            .map(|point| FilterBand {
                lower_hz: point[0],
                center_hz: point[1],
                upper_hz: point[2],
            })
            .collect::<Vec<_>>();
        let weights = bands
            .iter()
            .map(|band| {
                (0..bins)
                    .map(|bin| {
                        let hz = bin as f64 * f64::from(sample_rate) / fft_size as f64;
                        triangular_weight(hz, band)
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        Ok(Self {
            plan: plan.clone(),
            sample_rate,
            fft_size,
            bands,
            weights,
        })
    }

    /// Applies the filterbank to one non-negative FFT-bin energy row.
    pub fn apply(&self, energy: &[f64]) -> Result<Vec<f64>, AudioTransformError> {
        if energy.len() != self.fft_size / 2 + 1
            || energy
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(invalid(
                "filterbank input",
                "one finite non-negative value is required for every real-FFT bin",
            ));
        }
        Ok(self
            .weights
            .iter()
            .map(|weights| {
                weights
                    .iter()
                    .zip(energy)
                    .map(|(weight, value)| weight * value)
                    .sum()
            })
            .collect())
    }
}

/// Converts hertz to the selected perceptual scale.
pub fn scale_from_hz(scale: FrequencyScale, hz: f64) -> f64 {
    match scale {
        FrequencyScale::Mel => 2_595.0 * (1.0 + hz / 700.0).log10(),
        FrequencyScale::Bark => {
            13.0 * (0.000_76 * hz).atan() + 3.5 * ((hz / 7_500.0) * (hz / 7_500.0)).atan()
        }
        FrequencyScale::Erb => 21.4 * (1.0 + 0.004_37 * hz).log10(),
    }
}

/// Converts the selected perceptual scale coordinate back to hertz.
pub fn hz_from_scale(scale: FrequencyScale, value: f64) -> f64 {
    match scale {
        FrequencyScale::Mel => 700.0 * (10_f64.powf(value / 2_595.0) - 1.0),
        FrequencyScale::Erb => (10_f64.powf(value / 21.4) - 1.0) / 0.004_37,
        FrequencyScale::Bark => invert_bark(value),
    }
}

fn invert_bark(value: f64) -> f64 {
    let mut low = 0.0;
    let mut high = 384_000.0;
    for _ in 0..64 {
        let middle = (low + high) / 2.0;
        if scale_from_hz(FrequencyScale::Bark, middle) < value {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) / 2.0
}

fn validate_filterbank(
    sample_rate: u32,
    fft_size: usize,
    plan: &FilterbankPlan,
) -> Result<(), AudioTransformError> {
    let nyquist = f64::from(sample_rate) / 2.0;
    let maximum = plan.maximum_hz.unwrap_or(nyquist);
    if sample_rate == 0
        || fft_size < 2
        || plan.bands == 0
        || plan.max_weights == 0
        || !plan.minimum_hz.is_finite()
        || !maximum.is_finite()
        || plan.minimum_hz < 0.0
        || maximum <= plan.minimum_hz
        || maximum > nyquist
    {
        return Err(invalid(
            "filterbank plan",
            "bands, FFT/sample rate, and finite ascending Nyquist-bounded edges are required",
        ));
    }
    Ok(())
}

fn triangular_weight(hz: f64, band: &FilterBand) -> f64 {
    if hz <= band.lower_hz || hz >= band.upper_hz {
        0.0
    } else if hz <= band.center_hz {
        (hz - band.lower_hz) / (band.center_hz - band.lower_hz)
    } else {
        (band.upper_hz - hz) / (band.upper_hz - band.center_hz)
    }
}

fn filterbank_limit(required: usize, maximum: usize) -> AudioTransformError {
    AudioTransformError::WorkLimit {
        resource: "filterbank weights",
        required: u64::try_from(required).unwrap_or(u64::MAX),
        maximum: u64::try_from(maximum).unwrap_or(u64::MAX),
    }
}
