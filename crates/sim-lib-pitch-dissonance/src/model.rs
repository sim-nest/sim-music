use sim_lib_pitch_namer::LabelContext;
use sim_lib_pitch_scale::Scale;
use sim_lib_pitch_set::PitchClassMask;

/// Named three-component sonance result produced by a dissonance model.
#[derive(Clone, Debug, PartialEq)]
pub struct Sonance {
    /// Accumulated roughness or interval-conflict mass before density scaling.
    pub roughness_mass: f64,
    /// Density normalized against the selected interval-difference and merge
    /// policy.
    pub normalized_density: f64,
    /// Key, scale, or harmonic-context contribution.
    pub harmonic_context: f64,
    /// Model, normalization, aggregation, and compatibility provenance.
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
    pub model: &'static str,
    /// The named normalization policy.
    pub normalization: &'static str,
    /// The named aggregation policy.
    pub aggregation: &'static str,
    /// The named compatibility or standard dialect.
    pub dialect: &'static str,
    /// Human-readable source facts used to produce the result.
    pub provenance: Vec<String>,
}

/// The dissonance score produced by one named model.
#[derive(Clone, Debug, PartialEq)]
pub struct PitchDissonanceScore {
    /// The name of the model that produced the score.
    pub model: &'static str,
    /// The computed dissonance score; higher means more dissonant.
    pub score: f64,
    /// The typed sonance components behind [`PitchDissonanceScore::score`].
    pub sonance: Sonance,
}

/// Interval-difference catalog used by interval-density models.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IntervalDifferenceMode {
    /// Standard interval-class bins where the tritone is interval class 6.
    IntervalClass,
    /// Directed clockwise pitch-class differences from `1..=11`.
    DirectedSemitone,
    /// Legacy compatibility mode that treats interval-class 5 as the tritone
    /// density bin.
    LegacyTritoneIc5,
}

impl IntervalDifferenceMode {
    fn name(self) -> &'static str {
        match self {
            Self::IntervalClass => "interval-class",
            Self::DirectedSemitone => "directed-semitone",
            Self::LegacyTritoneIc5 => "legacy-tritone-ic5",
        }
    }
}

/// Merge policy for pairwise interval evidence.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IntervalMergeMode {
    /// Sum all pair contributions.
    SumPairs,
    /// Average over audible interval pairs.
    MeanPairs,
    /// Keep only the highest single interval contribution.
    MaxPair,
}

impl IntervalMergeMode {
    fn name(self) -> &'static str {
        match self {
            Self::SumPairs => "sum-pairs",
            Self::MeanPairs => "mean-pairs",
            Self::MaxPair => "max-pair",
        }
    }
}

/// Compatibility dialect for pitch dissonance analysis.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PitchDissonanceDialect {
    /// Standard catalog semantics.
    Standard,
    /// Compatibility dialect preserving the historical tritone-density binning
    /// quirk as a named choice.
    LegacyTritoneIc5,
}

impl PitchDissonanceDialect {
    fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::LegacyTritoneIc5 => "legacy-tritone-ic5",
        }
    }
}

/// Analysis options shared by built-in pitch dissonance models.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PitchDissonanceOptions {
    /// Interval-difference catalog.
    pub difference: IntervalDifferenceMode,
    /// Interval merge policy.
    pub merge: IntervalMergeMode,
    /// Compatibility dialect.
    pub dialect: PitchDissonanceDialect,
}

impl PitchDissonanceOptions {
    /// Returns options for the standard catalog dialect.
    pub const fn standard() -> Self {
        Self {
            difference: IntervalDifferenceMode::IntervalClass,
            merge: IntervalMergeMode::SumPairs,
            dialect: PitchDissonanceDialect::Standard,
        }
    }

    /// Returns options preserving the historical tritone-density quirk.
    pub const fn legacy_tritone_ic5() -> Self {
        Self {
            difference: IntervalDifferenceMode::LegacyTritoneIc5,
            merge: IntervalMergeMode::SumPairs,
            dialect: PitchDissonanceDialect::LegacyTritoneIc5,
        }
    }
}

/// A pluggable model that scores the dissonance of a pitch-class set.
pub trait PitchDissonanceModel {
    /// Returns the model's stable name.
    fn name(&self) -> &'static str;

    /// Scores `mask` for dissonance, optionally using key context from `context`.
    fn score(&self, mask: PitchClassMask, context: &LabelContext) -> f64;

    /// Returns typed sonance components using the standard dialect.
    fn sonance(&self, mask: PitchClassMask, context: &LabelContext) -> Sonance {
        self.sonance_with_options(mask, context, PitchDissonanceOptions::standard())
    }

    /// Returns typed sonance components using explicit catalog options.
    fn sonance_with_options(
        &self,
        mask: PitchClassMask,
        context: &LabelContext,
        options: PitchDissonanceOptions,
    ) -> Sonance {
        let score = self.score(mask, context);
        pitch_sonance(
            self.name(),
            score,
            0.0,
            0.0,
            mask,
            options,
            vec![format!("pitch-class-mask=0x{:03x}", mask.bits())],
        )
    }
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
        self.analyze_all_with_options(mask, context, PitchDissonanceOptions::standard())
    }

    /// Scores `mask` with every registered model and explicit analysis options.
    pub fn analyze_all_with_options(
        &self,
        mask: PitchClassMask,
        context: &LabelContext,
        options: PitchDissonanceOptions,
    ) -> Vec<PitchDissonanceScore> {
        self.models
            .iter()
            .map(|model| {
                let sonance = model.sonance_with_options(mask, context, options);
                PitchDissonanceScore {
                    model: model.name(),
                    score: sonance.compatibility_score(),
                    sonance,
                }
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
        self.sonance(mask, &LabelContext::default())
            .compatibility_score()
    }

    fn sonance_with_options(
        &self,
        mask: PitchClassMask,
        _context: &LabelContext,
        options: PitchDissonanceOptions,
    ) -> Sonance {
        let weights = [0.5, 0.25, 0.15, 0.4, 0.2, 1.0];
        let contributions = interval_contributions(mask, options.difference, &weights);
        let roughness = merge_contributions(&contributions, options.merge);
        pitch_sonance(
            self.name(),
            roughness,
            normalized_density(mask, options.difference),
            0.0,
            mask,
            options,
            vec![format!("interval-contributions={contributions:?}")],
        )
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
        self.sonance(mask, &LabelContext::default())
            .compatibility_score()
    }

    fn sonance_with_options(
        &self,
        mask: PitchClassMask,
        _context: &LabelContext,
        options: PitchDissonanceOptions,
    ) -> Sonance {
        let cardinality = mask.count_bits() as f64;
        let vector_mass: f64 = mask.interval_vector().0.into_iter().map(f64::from).sum();
        pitch_sonance(
            self.name(),
            cardinality,
            normalize_pair_mass(vector_mass, mask),
            vector_mass / 8.0,
            mask,
            options,
            vec![format!("cardinality={cardinality}")],
        )
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
        self.sonance(mask, context).compatibility_score()
    }

    fn sonance_with_options(
        &self,
        mask: PitchClassMask,
        context: &LabelContext,
        options: PitchDissonanceOptions,
    ) -> Sonance {
        let tritones = tritone_pairs(mask, options.dialect) as f64;
        let Some(key) = context.key else {
            return pitch_sonance(
                self.name(),
                tritones,
                normalized_density(mask, options.difference),
                1.5,
                mask,
                options,
                vec!["no-key-context".to_owned()],
            );
        };
        let scale = Scale::new(key.tonic, key.mode);
        let off_scale = mask
            .pitch_classes()
            .into_iter()
            .filter(|pc| scale.degree_of(*pc).is_none())
            .count() as f64;
        pitch_sonance(
            self.name(),
            tritones * 0.75,
            normalized_density(mask, options.difference),
            off_scale,
            mask,
            options,
            vec![format!("key={:?}-{:?}", key.tonic, key.mode)],
        )
    }
}

/// A model that scores the fraction of interval pairs that are tritones.
pub struct TritoneDensity;

impl PitchDissonanceModel for TritoneDensity {
    fn name(&self) -> &'static str {
        "tritone-density"
    }

    fn score(&self, mask: PitchClassMask, _context: &LabelContext) -> f64 {
        self.sonance(mask, &LabelContext::default())
            .compatibility_score()
    }

    fn sonance_with_options(
        &self,
        mask: PitchClassMask,
        _context: &LabelContext,
        options: PitchDissonanceOptions,
    ) -> Sonance {
        pitch_sonance(
            self.name(),
            0.0,
            tritone_density(mask, options.dialect),
            0.0,
            mask,
            options,
            vec![format!("tritone-dialect={}", options.dialect.name())],
        )
    }
}

fn pitch_sonance(
    model: &'static str,
    roughness_mass: f64,
    normalized_density: f64,
    harmonic_context: f64,
    mask: PitchClassMask,
    options: PitchDissonanceOptions,
    mut provenance: Vec<String>,
) -> Sonance {
    provenance.push(format!("pitch-count={}", mask.count_bits()));
    Sonance {
        roughness_mass,
        normalized_density,
        harmonic_context,
        evidence: SonanceEvidence {
            model,
            normalization: options.difference.name(),
            aggregation: options.merge.name(),
            dialect: options.dialect.name(),
            provenance,
        },
    }
}

fn interval_contributions(
    mask: PitchClassMask,
    mode: IntervalDifferenceMode,
    weights: &[f64; 6],
) -> Vec<f64> {
    match mode {
        IntervalDifferenceMode::IntervalClass => mask
            .interval_vector()
            .0
            .into_iter()
            .zip(*weights)
            .map(|(count, weight)| count as f64 * weight)
            .collect(),
        IntervalDifferenceMode::DirectedSemitone => directed_differences(mask)
            .into_iter()
            .map(|difference| {
                let interval_class = difference.min(12 - difference);
                weights[(interval_class - 1) as usize]
            })
            .collect(),
        IntervalDifferenceMode::LegacyTritoneIc5 => {
            let mut legacy_weights = *weights;
            legacy_weights.swap(4, 5);
            mask.interval_vector()
                .0
                .into_iter()
                .zip(legacy_weights)
                .map(|(count, weight)| count as f64 * weight)
                .collect()
        }
    }
}

fn directed_differences(mask: PitchClassMask) -> Vec<u8> {
    let pitch_classes = mask.pitch_classes();
    let mut differences = Vec::new();
    for (index, left) in pitch_classes.iter().enumerate() {
        for right in pitch_classes.iter().skip(index + 1) {
            let difference = (right.value() + 12 - left.value()) % 12;
            if difference > 0 {
                differences.push(difference);
            }
        }
    }
    differences
}

fn merge_contributions(contributions: &[f64], mode: IntervalMergeMode) -> f64 {
    match mode {
        IntervalMergeMode::SumPairs => contributions.iter().sum(),
        IntervalMergeMode::MeanPairs => {
            if contributions.is_empty() {
                0.0
            } else {
                contributions.iter().sum::<f64>() / contributions.len() as f64
            }
        }
        IntervalMergeMode::MaxPair => contributions.iter().copied().fold(0.0, f64::max),
    }
}

fn normalize_pair_mass(mass: f64, mask: PitchClassMask) -> f64 {
    let pairs = total_pairs(mask);
    if pairs == 0.0 { 0.0 } else { mass / pairs }
}

fn normalized_density(mask: PitchClassMask, mode: IntervalDifferenceMode) -> f64 {
    match mode {
        IntervalDifferenceMode::IntervalClass | IntervalDifferenceMode::LegacyTritoneIc5 => {
            normalize_pair_mass(
                mask.interval_vector().0.into_iter().map(f64::from).sum(),
                mask,
            )
        }
        IntervalDifferenceMode::DirectedSemitone => normalize_pair_mass(
            directed_differences(mask).into_iter().map(|_| 1.0).sum(),
            mask,
        ),
    }
}

fn tritone_density(mask: PitchClassMask, dialect: PitchDissonanceDialect) -> f64 {
    let pairs = total_pairs(mask);
    if pairs == 0.0 {
        0.0
    } else {
        tritone_pairs(mask, dialect) as f64 / pairs
    }
}

fn tritone_pairs(mask: PitchClassMask, dialect: PitchDissonanceDialect) -> u16 {
    match dialect {
        PitchDissonanceDialect::Standard => mask.interval_vector().0[5],
        PitchDissonanceDialect::LegacyTritoneIc5 => mask.interval_vector().0[4],
    }
}

fn total_pairs(mask: PitchClassMask) -> f64 {
    mask.interval_vector().0.into_iter().map(f64::from).sum()
}
