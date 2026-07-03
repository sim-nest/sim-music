use sim_lib_pitch_namer::LabelContext;
use sim_lib_pitch_scale::Scale;
use sim_lib_pitch_set::PitchClassMask;

/// The dissonance score produced by one named model.
#[derive(Clone, Debug, PartialEq)]
pub struct PitchDissonanceScore {
    /// The name of the model that produced the score.
    pub model: &'static str,
    /// The computed dissonance score; higher means more dissonant.
    pub score: f64,
}

/// A pluggable model that scores the dissonance of a pitch-class set.
pub trait PitchDissonanceModel {
    /// Returns the model's stable name.
    fn name(&self) -> &'static str;

    /// Scores `mask` for dissonance, optionally using key context from `context`.
    fn score(&self, mask: PitchClassMask, context: &LabelContext) -> f64;
}

/// A registry of [`PitchDissonanceModel`]s that can score a set with all models at
/// once.
#[derive(Default)]
pub struct PitchDissonanceRegistry {
    models: Vec<Box<dyn PitchDissonanceModel>>,
}

impl PitchDissonanceRegistry {
    /// Builds a registry populated with the four built-in models.
    pub fn new_with_builtins() -> Self {
        Self {
            models: vec![
                Box::new(IntervalVectorModel),
                Box::new(ForteComplexity),
                Box::new(TonalFunctionDissonance),
                Box::new(TritoneDensity),
            ],
        }
    }

    /// Scores `mask` with every registered model, returning one score per model.
    pub fn analyze_all(
        &self,
        mask: PitchClassMask,
        context: &LabelContext,
    ) -> Vec<PitchDissonanceScore> {
        self.models
            .iter()
            .map(|model| PitchDissonanceScore {
                model: model.name(),
                score: model.score(mask, context),
            })
            .collect()
    }
}

/// A model that weights each interval class of the set's interval vector, scoring
/// half-steps and tritones as most dissonant.
pub struct IntervalVectorModel;

impl PitchDissonanceModel for IntervalVectorModel {
    fn name(&self) -> &'static str {
        "interval-vector"
    }

    fn score(&self, mask: PitchClassMask, _context: &LabelContext) -> f64 {
        let weights = [0.5, 0.25, 0.15, 0.4, 1.0, 0.2];
        mask.interval_vector()
            .0
            .into_iter()
            .zip(weights)
            .map(|(count, weight)| count as f64 * weight)
            .sum()
    }
}

/// A model that scores complexity from set cardinality plus total interval-vector
/// mass.
pub struct ForteComplexity;

impl PitchDissonanceModel for ForteComplexity {
    fn name(&self) -> &'static str {
        "forte-complexity"
    }

    fn score(&self, mask: PitchClassMask, _context: &LabelContext) -> f64 {
        let cardinality = mask.count_bits() as f64;
        let vector_mass: f64 = mask.interval_vector().0.into_iter().map(f64::from).sum();
        cardinality + vector_mass / 8.0
    }
}

/// A key-relative model that scores out-of-scale pitch classes and tritone pairs;
/// without a key it falls back to a tritone-weighted baseline.
pub struct TonalFunctionDissonance;

impl PitchDissonanceModel for TonalFunctionDissonance {
    fn name(&self) -> &'static str {
        "tonal-function"
    }

    fn score(&self, mask: PitchClassMask, context: &LabelContext) -> f64 {
        let Some(key) = context.key else {
            return 1.5 + tritone_pairs(mask) as f64;
        };
        let scale = Scale::new(key.tonic, key.mode);
        let off_scale = mask
            .pitch_classes()
            .into_iter()
            .filter(|pc| scale.degree_of(*pc).is_none())
            .count() as f64;
        off_scale + tritone_pairs(mask) as f64 * 0.75
    }
}

/// A model that scores the fraction of interval pairs that are tritones.
pub struct TritoneDensity;

impl PitchDissonanceModel for TritoneDensity {
    fn name(&self) -> &'static str {
        "tritone-density"
    }

    fn score(&self, mask: PitchClassMask, _context: &LabelContext) -> f64 {
        let total_pairs = mask
            .interval_vector()
            .0
            .into_iter()
            .map(f64::from)
            .sum::<f64>();
        if total_pairs == 0.0 {
            0.0
        } else {
            tritone_pairs(mask) as f64 / total_pairs
        }
    }
}

fn tritone_pairs(mask: PitchClassMask) -> u16 {
    mask.interval_vector().0[4]
}
