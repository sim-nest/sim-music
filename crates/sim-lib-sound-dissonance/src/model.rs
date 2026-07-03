use std::collections::HashMap;
use std::sync::Arc;

use sim_lib_sound_core::{Amplitude, Frequency, Tone};
use sim_lib_sound_spectrum::Spectrum;

/// A named dissonance model's score for some input.
#[derive(Clone, Debug, PartialEq)]
pub struct DissonanceScore {
    /// Name of the model that produced the score.
    pub model: String,
    /// Computed dissonance value (higher is more dissonant).
    pub score: f64,
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
        sensory_roughness(tones, 3.5, 5.75)
    }

    fn dissonance_of_spectrum(&self, spectrum: &Spectrum) -> Option<f64> {
        Some(sensory_roughness_from_bins(&spectrum.bins, 3.5, 5.75))
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
        sensory_roughness(tones, 5.0, 5.0)
    }

    fn dissonance_of_spectrum(&self, spectrum: &Spectrum) -> Option<f64> {
        Some(sensory_roughness_from_bins(&spectrum.bins, 5.0, 5.0))
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
        beating_score(&chord_bins(tones))
    }

    fn dissonance_of_spectrum(&self, spectrum: &Spectrum) -> Option<f64> {
        Some(beating_score(&spectrum.bins))
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
        harmonic_entropy_score(&chord_bins(tones), self.spread)
    }

    fn dissonance_of_spectrum(&self, spectrum: &Spectrum) -> Option<f64> {
        Some(harmonic_entropy_score(&spectrum.bins, self.spread))
    }
}

/// Scores a chord against every model in `registry`, returning one
/// [`DissonanceScore`] per model.
pub fn analyze_chord(tones: &[Tone], registry: &DissonanceRegistry) -> Vec<DissonanceScore> {
    registry
        .list()
        .into_iter()
        .filter_map(|name| registry.get(&name).map(|model| (name, model)))
        .map(|(name, model)| DissonanceScore {
            model: name,
            score: model.dissonance_of_chord(tones),
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

fn sensory_roughness(tones: &[Tone], a: f64, b: f64) -> f64 {
    sensory_roughness_from_bins(&chord_bins(tones), a, b)
}

fn sensory_roughness_from_bins(bins: &[(Frequency, Amplitude)], a: f64, b: f64) -> f64 {
    let mut total = 0.0;
    for i in 0..bins.len() {
        for j in (i + 1)..bins.len() {
            let (left_f, left_a) = bins[i];
            let (right_f, right_a) = bins[j];
            let min_freq = left_f.0.min(right_f.0).max(1.0);
            let s = 0.24 / (0.021 * min_freq + 19.0);
            let x = (right_f.0 - left_f.0).abs() * s;
            let curve = (-a * x).exp() - (-b * x).exp();
            total += (left_a.0 * right_a.0) * curve.abs();
        }
    }
    total.max(0.0)
}

fn beating_score(bins: &[(Frequency, Amplitude)]) -> f64 {
    let mut total = 0.0;
    for i in 0..bins.len() {
        for j in (i + 1)..bins.len() {
            let delta = (bins[i].0.0 - bins[j].0.0).abs();
            if delta < 30.0 {
                total += (1.0 - delta / 30.0) * bins[i].1.0 * bins[j].1.0;
            }
        }
    }
    total
}

fn harmonic_entropy_score(bins: &[(Frequency, Amplitude)], spread: f64) -> f64 {
    if bins.len() < 2 {
        return 0.0;
    }
    let simple_ratios = [
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
    ];
    let mut entropy = 0.0;
    let mut pairs: f64 = 0.0;
    for i in 0..bins.len() {
        for j in (i + 1)..bins.len() {
            let ratio = (bins[i].0.0.max(bins[j].0.0) / bins[i].0.0.min(bins[j].0.0)).max(1.0);
            let weights = simple_ratios.map(|simple| {
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
    entropy / pairs.max(1.0)
}
