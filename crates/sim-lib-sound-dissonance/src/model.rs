use std::collections::HashMap;
use std::sync::Arc;

use sim_lib_sound_core::{Amplitude, Frequency, Tone};
use sim_lib_sound_spectrum::Spectrum;
use thiserror::Error;

/// Error raised when dissonance analysis receives invalid acoustic input.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DissonanceInputError {
    /// A frequency, amplitude, curve parameter, or spread was non-finite.
    #[error("dissonance inputs must be finite")]
    NonFiniteInput,
    /// A frequency was zero or negative.
    #[error("partial frequency must be positive")]
    InvalidFrequency,
    /// An amplitude was negative.
    #[error("partial amplitude must be non-negative")]
    InvalidAmplitude,
}

/// Named three-component sonance result produced by a sensory model.
#[derive(Clone, Debug, PartialEq)]
pub struct Sonance {
    /// Sum of partial-pair roughness mass before normalization.
    pub roughness_mass: f64,
    /// Roughness density normalized by audible partial-pair opportunity.
    pub normalized_density: f64,
    /// Harmonic-context contribution, such as harmonic entropy.
    pub harmonic_context: f64,
    /// Model, normalization, aggregation, and partial-policy provenance.
    pub evidence: SonanceEvidence,
}

impl Sonance {
    /// Returns an explicit scalar compatibility projection for callers that
    /// still consume the older one-number API.
    pub fn compatibility_score(&self) -> f64 {
        self.roughness_mass + self.normalized_density + self.harmonic_context
    }
}

/// Provenance for a [`Sonance`] result.
#[derive(Clone, Debug, PartialEq)]
pub struct SonanceEvidence {
    /// The model that produced the result.
    pub model: String,
    /// The named normalization policy.
    pub normalization: &'static str,
    /// The named aggregation policy.
    pub aggregation: &'static str,
    /// The curve family used by partial-pair roughness.
    pub curve_family: &'static str,
    /// Partial filtering and inaudible-pair policy report.
    pub partial_policy: PartialPairPolicyReport,
    /// Human-readable source facts used to produce the result.
    pub provenance: Vec<String>,
}

/// Report describing invalid, silent, and evaluated partial pairs.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PartialPairPolicyReport {
    /// Number of partial bins accepted as finite and audible.
    pub audible_partials: usize,
    /// Number of bins skipped because their amplitude is zero.
    pub inaudible_partials: usize,
    /// Number of evaluated audible partial pairs.
    pub evaluated_pairs: usize,
    /// Number of partial pairs skipped because at least one side was inaudible.
    pub skipped_inaudible_pairs: usize,
}

/// A named dissonance model's score for some input.
#[derive(Clone, Debug, PartialEq)]
pub struct DissonanceScore {
    /// Name of the model that produced the score.
    pub model: String,
    /// Computed dissonance value (higher is more dissonant).
    pub score: f64,
    /// The typed sonance components behind [`DissonanceScore::score`].
    pub sonance: Sonance,
}

/// Psychoacoustic roughness curve family used for partial-pair scoring.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PsychoacousticCurveFamily {
    /// Plomp-Levelt critical-band curve.
    PlompLevelt,
    /// Sethares curve with equal exponential slopes.
    Sethares,
    /// Helmholtz close-beating curve.
    HelmholtzBeating,
    /// Gaussian simple-ratio window used by harmonic entropy.
    HarmonicEntropy {
        /// Standard deviation, in cents.
        spread: f64,
    },
}

impl PsychoacousticCurveFamily {
    fn name(self) -> &'static str {
        match self {
            Self::PlompLevelt => "plomp-levelt",
            Self::Sethares => "sethares",
            Self::HelmholtzBeating => "helmholtz-beating",
            Self::HarmonicEntropy { .. } => "harmonic-entropy",
        }
    }
}

/// A model that estimates the sensory dissonance of tones, pairs, and chords.
pub trait DissonanceModel: Send + Sync {
    /// Returns the stable identifier of this model.
    fn name(&self) -> &'static str;

    /// Returns a short human-readable description of the model.
    fn description(&self) -> &'static str;

    /// Returns the dissonance of a single tone (defaults to a one-tone chord).
    fn dissonance_of_tone(&self, tone: &Tone) -> f64 {
        self.dissonance_of_chord(std::slice::from_ref(tone))
    }

    /// Returns the dissonance of two tones sounded together.
    fn dissonance_of_pair(&self, left: &Tone, right: &Tone) -> f64 {
        self.dissonance_of_chord(&[left.clone(), right.clone()])
    }

    /// Returns the dissonance of a chord of tones.
    fn dissonance_of_chord(&self, tones: &[Tone]) -> f64;

    /// Returns typed sonance components for a chord.
    fn sonance_of_chord(&self, tones: &[Tone]) -> Result<Sonance, DissonanceInputError> {
        let score = self.dissonance_of_chord(tones);
        finite(score)?;
        Ok(Sonance {
            roughness_mass: score,
            normalized_density: 0.0,
            harmonic_context: 0.0,
            evidence: SonanceEvidence {
                model: self.name().to_owned(),
                normalization: "raw",
                aggregation: "model-defined",
                curve_family: "model-defined",
                partial_policy: PartialPairPolicyReport::default(),
                provenance: vec![format!("tone-count={}", tones.len())],
            },
        })
    }

    /// Returns the dissonance computed directly from a spectrum, if the model
    /// supports spectral input.
    fn dissonance_of_spectrum(&self, _spectrum: &Spectrum) -> Option<f64> {
        None
    }
}

/// A registry of dissonance models keyed by name.
#[derive(Default)]
pub struct DissonanceRegistry {
    models: HashMap<String, Arc<dyn DissonanceModel>>,
}

impl DissonanceRegistry {
    /// Returns a registry populated with the built-in dissonance models.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_sound_dissonance::DissonanceRegistry;
    ///
    /// let registry = DissonanceRegistry::new_with_builtins();
    /// assert!(registry.list().contains(&"sethares".to_owned()));
    /// ```
    pub fn new_with_builtins() -> Self {
        let mut registry = Self::default();
        registry.register(Arc::new(PlompLevelt));
        registry.register(Arc::new(Sethares));
        registry.register(Arc::new(HelmholtzBeating));
        registry.register(Arc::new(HarmonicEntropy { spread: 18.0 }));
        registry
    }

    /// Registers `model`, replacing any existing model with the same name.
    pub fn register(&mut self, model: Arc<dyn DissonanceModel>) {
        self.models.insert(model.name().to_owned(), model);
    }

    /// Returns the model registered under `name`, if any.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn DissonanceModel>> {
        self.models.get(name)
    }

    /// Returns the names of all registered models, sorted.
    pub fn list(&self) -> Vec<String> {
        let mut names = self.models.keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }
}

/// Pairwise roughness output for one pair of partials.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PairRoughness {
    /// First partial frequency.
    pub left_frequency: Frequency,
    /// Second partial frequency.
    pub right_frequency: Frequency,
    /// Amplitude-weighted roughness contribution.
    pub roughness: f64,
}

/// Plomp-Levelt critical-band roughness model.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PlompLevelt;

/// Sethares spectral-roughness model.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Sethares;

/// Helmholtz beating model, counting close-frequency partials.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HelmholtzBeating;

/// Harmonic-entropy model over nearby simple-ratio interpretations.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HarmonicEntropy {
    /// Standard deviation, in cents, of the ratio-matching window.
    pub spread: f64,
}

/// A serializable description selecting a dissonance model.
#[derive(Clone, Debug, PartialEq)]
pub enum DissonanceModelDescriptor {
    /// The [`PlompLevelt`] model.
    PlompLevelt,
    /// The [`Sethares`] model.
    Sethares,
    /// The [`HelmholtzBeating`] model.
    HelmholtzBeating,
    /// The [`HarmonicEntropy`] model with the given spread.
    HarmonicEntropy {
        /// Standard deviation, in cents, of the ratio-matching window.
        spread: f64,
    },
}

impl DissonanceModelDescriptor {
    /// Builds the shared model object described by this descriptor.
    pub fn to_model(&self) -> Arc<dyn DissonanceModel> {
        match self {
            Self::PlompLevelt => Arc::new(PlompLevelt),
            Self::Sethares => Arc::new(Sethares),
            Self::HelmholtzBeating => Arc::new(HelmholtzBeating),
            Self::HarmonicEntropy { spread } => Arc::new(HarmonicEntropy { spread: *spread }),
        }
    }
}

impl DissonanceModel for PlompLevelt {
    fn name(&self) -> &'static str {
        "plomp-levelt"
    }

    fn description(&self) -> &'static str {
        "critical-band roughness estimate"
    }

    fn dissonance_of_chord(&self, tones: &[Tone]) -> f64 {
        sensory_sonance(tones, PsychoacousticCurveFamily::PlompLevelt)
            .map(|sonance| sonance.compatibility_score())
            .unwrap_or(f64::NAN)
    }

    fn sonance_of_chord(&self, tones: &[Tone]) -> Result<Sonance, DissonanceInputError> {
        sensory_sonance(tones, PsychoacousticCurveFamily::PlompLevelt)
    }

    fn dissonance_of_spectrum(&self, spectrum: &Spectrum) -> Option<f64> {
        sensory_sonance_from_bins(&spectrum.bins, PsychoacousticCurveFamily::PlompLevelt)
            .ok()
            .map(|sonance| sonance.compatibility_score())
    }
}

impl DissonanceModel for Sethares {
    fn name(&self) -> &'static str {
        "sethares"
    }

    fn description(&self) -> &'static str {
        "spectral roughness with tuning-agnostic ratio weighting"
    }

    fn dissonance_of_chord(&self, tones: &[Tone]) -> f64 {
        sensory_sonance(tones, PsychoacousticCurveFamily::Sethares)
            .map(|sonance| sonance.compatibility_score())
            .unwrap_or(f64::NAN)
    }

    fn sonance_of_chord(&self, tones: &[Tone]) -> Result<Sonance, DissonanceInputError> {
        sensory_sonance(tones, PsychoacousticCurveFamily::Sethares)
    }

    fn dissonance_of_spectrum(&self, spectrum: &Spectrum) -> Option<f64> {
        sensory_sonance_from_bins(&spectrum.bins, PsychoacousticCurveFamily::Sethares)
            .ok()
            .map(|sonance| sonance.compatibility_score())
    }
}

impl DissonanceModel for HelmholtzBeating {
    fn name(&self) -> &'static str {
        "helmholtz-beating"
    }

    fn description(&self) -> &'static str {
        "counts close-frequency beating within about 30 hz"
    }

    fn dissonance_of_chord(&self, tones: &[Tone]) -> f64 {
        sensory_sonance(tones, PsychoacousticCurveFamily::HelmholtzBeating)
            .map(|sonance| sonance.compatibility_score())
            .unwrap_or(f64::NAN)
    }

    fn sonance_of_chord(&self, tones: &[Tone]) -> Result<Sonance, DissonanceInputError> {
        sensory_sonance(tones, PsychoacousticCurveFamily::HelmholtzBeating)
    }

    fn dissonance_of_spectrum(&self, spectrum: &Spectrum) -> Option<f64> {
        sensory_sonance_from_bins(&spectrum.bins, PsychoacousticCurveFamily::HelmholtzBeating)
            .ok()
            .map(|sonance| sonance.compatibility_score())
    }
}

impl DissonanceModel for HarmonicEntropy {
    fn name(&self) -> &'static str {
        "harmonic-entropy"
    }

    fn description(&self) -> &'static str {
        "entropy over nearby simple-ratio interpretations"
    }

    fn dissonance_of_chord(&self, tones: &[Tone]) -> f64 {
        self.sonance_of_chord(tones)
            .map(|sonance| sonance.compatibility_score())
            .unwrap_or(f64::NAN)
    }

    fn sonance_of_chord(&self, tones: &[Tone]) -> Result<Sonance, DissonanceInputError> {
        finite(self.spread)?;
        let mut sonance = sensory_sonance(
            tones,
            PsychoacousticCurveFamily::HarmonicEntropy {
                spread: self.spread,
            },
        )?;
        let entropy = harmonic_entropy_score(&chord_bins_checked(tones)?.0, self.spread)?;
        sonance.roughness_mass = 0.0;
        sonance.normalized_density = 0.0;
        sonance.harmonic_context = entropy;
        sonance.evidence.aggregation = "mean-pair-entropy";
        Ok(sonance)
    }

    fn dissonance_of_spectrum(&self, spectrum: &Spectrum) -> Option<f64> {
        harmonic_entropy_score(&spectrum.bins, self.spread).ok()
    }
}

/// Scores a chord against every model in `registry`, returning one
/// [`DissonanceScore`] per model.
pub fn analyze_chord(tones: &[Tone], registry: &DissonanceRegistry) -> Vec<DissonanceScore> {
    try_analyze_chord(tones, registry).unwrap_or_default()
}

/// Checked variant of [`analyze_chord`] that rejects non-finite acoustic input.
pub fn try_analyze_chord(
    tones: &[Tone],
    registry: &DissonanceRegistry,
) -> Result<Vec<DissonanceScore>, DissonanceInputError> {
    registry
        .list()
        .into_iter()
        .filter_map(|name| registry.get(&name).map(|model| (name, model)))
        .map(|(name, model)| {
            let sonance = model.sonance_of_chord(tones)?;
            Ok(DissonanceScore {
                model: name,
                score: sonance.compatibility_score(),
                sonance,
            })
        })
        .collect()
}

fn chord_bins(tones: &[Tone]) -> Vec<(Frequency, Amplitude)> {
    tones
        .iter()
        .flat_map(|tone| {
            tone.partials
                .iter()
                .map(|partial| (partial.frequency, partial.amplitude))
        })
        .collect()
}

fn chord_bins_checked(
    tones: &[Tone],
) -> Result<(Vec<(Frequency, Amplitude)>, PartialPairPolicyReport), DissonanceInputError> {
    checked_bins(&chord_bins(tones))
}

fn checked_bins(
    bins: &[(Frequency, Amplitude)],
) -> Result<(Vec<(Frequency, Amplitude)>, PartialPairPolicyReport), DissonanceInputError> {
    let mut audible = Vec::new();
    let mut report = PartialPairPolicyReport::default();
    for (frequency, amplitude) in bins {
        finite(frequency.0)?;
        finite(amplitude.0)?;
        if frequency.0 <= 0.0 {
            return Err(DissonanceInputError::InvalidFrequency);
        }
        if amplitude.0 < 0.0 {
            return Err(DissonanceInputError::InvalidAmplitude);
        }
        if amplitude.0 == 0.0 {
            report.inaudible_partials += 1;
        } else {
            report.audible_partials += 1;
            audible.push((*frequency, *amplitude));
        }
    }
    let total_pairs = bins.len().saturating_mul(bins.len().saturating_sub(1)) / 2;
    report.evaluated_pairs = audible
        .len()
        .saturating_mul(audible.len().saturating_sub(1))
        / 2;
    report.skipped_inaudible_pairs = total_pairs.saturating_sub(report.evaluated_pairs);
    Ok((audible, report))
}

fn sensory_sonance(
    tones: &[Tone],
    curve: PsychoacousticCurveFamily,
) -> Result<Sonance, DissonanceInputError> {
    sensory_sonance_from_bins(&chord_bins(tones), curve)
}

fn sensory_sonance_from_bins(
    bins: &[(Frequency, Amplitude)],
    curve: PsychoacousticCurveFamily,
) -> Result<Sonance, DissonanceInputError> {
    let (audible, report) = checked_bins(bins)?;
    let pair_roughness = partial_pair_roughness(&audible, curve)?;
    let roughness_mass = pair_roughness
        .iter()
        .map(|pair| pair.roughness)
        .sum::<f64>()
        .max(0.0);
    let normalized_density = if report.evaluated_pairs == 0 {
        0.0
    } else {
        roughness_mass / report.evaluated_pairs as f64
    };
    Ok(Sonance {
        roughness_mass,
        normalized_density,
        harmonic_context: 0.0,
        evidence: SonanceEvidence {
            model: curve.name().to_owned(),
            normalization: "audible-pair-mean",
            aggregation: "sum-partial-pairs",
            curve_family: curve.name(),
            partial_policy: report,
            provenance: vec![format!("partial-pairs={}", pair_roughness.len())],
        },
    })
}

/// Computes roughness for every audible pair in `bins` using `curve`.
pub fn partial_pair_roughness(
    bins: &[(Frequency, Amplitude)],
    curve: PsychoacousticCurveFamily,
) -> Result<Vec<PairRoughness>, DissonanceInputError> {
    finite_curve(curve)?;
    let mut pairs = Vec::new();
    for i in 0..bins.len() {
        for j in (i + 1)..bins.len() {
            let (left_frequency, left_amplitude) = bins[i];
            let (right_frequency, right_amplitude) = bins[j];
            let roughness = pair_curve(
                left_frequency,
                left_amplitude,
                right_frequency,
                right_amplitude,
                curve,
            )?;
            pairs.push(PairRoughness {
                left_frequency,
                right_frequency,
                roughness,
            });
        }
    }
    Ok(pairs)
}

fn pair_curve(
    left_f: Frequency,
    left_a: Amplitude,
    right_f: Frequency,
    right_a: Amplitude,
    curve: PsychoacousticCurveFamily,
) -> Result<f64, DissonanceInputError> {
    finite(left_f.0)?;
    finite(right_f.0)?;
    finite(left_a.0)?;
    finite(right_a.0)?;
    if left_f.0 <= 0.0 || right_f.0 <= 0.0 {
        return Err(DissonanceInputError::InvalidFrequency);
    }
    if left_a.0 < 0.0 || right_a.0 < 0.0 {
        return Err(DissonanceInputError::InvalidAmplitude);
    }
    let amplitude = left_a.0 * right_a.0;
    let delta = (left_f.0 - right_f.0).abs();
    let roughness = match curve {
        PsychoacousticCurveFamily::PlompLevelt => critical_band_curve(left_f, right_f, 3.5, 5.75),
        PsychoacousticCurveFamily::Sethares => critical_band_curve(left_f, right_f, 5.0, 5.0),
        PsychoacousticCurveFamily::HelmholtzBeating => {
            if delta < 30.0 {
                1.0 - delta / 30.0
            } else {
                0.0
            }
        }
        PsychoacousticCurveFamily::HarmonicEntropy { spread } => {
            let ratio = (left_f.0.max(right_f.0) / left_f.0.min(right_f.0)).max(1.0);
            let nearest = simple_ratios()
                .into_iter()
                .map(|simple| 1200.0 * (ratio / simple).log2().abs())
                .fold(f64::INFINITY, f64::min);
            (-(nearest * nearest) / (2.0 * spread.max(1.0).powi(2))).exp()
        }
    };
    finite(roughness)?;
    Ok(amplitude * roughness.abs())
}

fn critical_band_curve(left_f: Frequency, right_f: Frequency, a: f64, b: f64) -> f64 {
    let min_freq = left_f.0.min(right_f.0).max(1.0);
    let s = 0.24 / (0.021 * min_freq + 19.0);
    let x = (right_f.0 - left_f.0).abs() * s;
    (-a * x).exp() - (-b * x).exp()
}

fn harmonic_entropy_score(
    bins: &[(Frequency, Amplitude)],
    spread: f64,
) -> Result<f64, DissonanceInputError> {
    finite(spread)?;
    if bins.len() < 2 {
        return Ok(0.0);
    }
    let mut entropy = 0.0;
    let mut pairs: f64 = 0.0;
    for i in 0..bins.len() {
        for j in (i + 1)..bins.len() {
            let ratio = (bins[i].0.0.max(bins[j].0.0) / bins[i].0.0.min(bins[j].0.0)).max(1.0);
            let weights = simple_ratios().map(|simple| {
                let cents = 1200.0 * (ratio / simple).log2().abs();
                (-(cents * cents) / (2.0 * spread.max(1.0).powi(2))).exp()
            });
            let sum = weights.iter().sum::<f64>().max(f64::EPSILON);
            for weight in weights {
                let probability = weight / sum;
                if probability > f64::EPSILON {
                    entropy -= probability * probability.log2();
                }
            }
            pairs += 1.0;
        }
    }
    Ok(entropy / pairs.max(1.0))
}

fn simple_ratios() -> [f64; 11] {
    [
        1.0,
        16.0 / 15.0,
        10.0 / 9.0,
        9.0 / 8.0,
        6.0 / 5.0,
        5.0 / 4.0,
        4.0 / 3.0,
        3.0 / 2.0,
        5.0 / 3.0,
        15.0 / 8.0,
        2.0,
    ]
}

fn finite(value: f64) -> Result<(), DissonanceInputError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(DissonanceInputError::NonFiniteInput)
    }
}

fn finite_curve(curve: PsychoacousticCurveFamily) -> Result<(), DissonanceInputError> {
    match curve {
        PsychoacousticCurveFamily::PlompLevelt
        | PsychoacousticCurveFamily::Sethares
        | PsychoacousticCurveFamily::HelmholtzBeating => Ok(()),
        PsychoacousticCurveFamily::HarmonicEntropy { spread } => finite(spread),
    }
}
