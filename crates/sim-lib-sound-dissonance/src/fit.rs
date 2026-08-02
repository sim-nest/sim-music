//! Bounded fitting and certification reports for sonance model parameters.

use sim_lib_discrete_search::{
    NeverInterrupt, SearchControl, SearchProblem, SearchReceipt, SearchRun, SearchStep, solve,
};
use sim_lib_sound_core::{Amplitude, Frequency};

use crate::fit_digest::{corpus_digest, report_digest, stable_digest_value};
use crate::{
    DissonanceInputError, PairRoughness, PsychoacousticCurveFamily, partial_pair_roughness,
};

/// Objective function used while fitting sonance parameters.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SonanceFitObjective {
    /// Maximize Spearman rank correlation between predicted and target ordering.
    RankCorrelation,
}

impl SonanceFitObjective {
    /// Stable objective label used in reports and recipe text.
    pub fn label(self) -> &'static str {
        match self {
            Self::RankCorrelation => "rank-correlation",
        }
    }
}

/// Search strategy used to generate candidate parameter sets.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SonanceFitStrategy {
    /// Visit the full declared grid in deterministic order.
    BruteForce,
    /// Visit a midpoint plus one-axis sweeps before the corners.
    Coordinate,
    /// Visit the declared grid in a deterministic seed-shuffled order.
    BoundedStochastic,
}

impl SonanceFitStrategy {
    /// Stable strategy label used in reports.
    pub fn label(self) -> &'static str {
        match self {
            Self::BruteForce => "brute-force",
            Self::Coordinate => "coordinate",
            Self::BoundedStochastic => "bounded-stochastic",
        }
    }
}

/// Inclusive floating-point grid for one tunable parameter.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ParameterRange {
    /// Inclusive start value.
    pub start: f64,
    /// Inclusive end value.
    pub end: f64,
    /// Positive step size.
    pub step: f64,
}

impl ParameterRange {
    /// Builds a checked inclusive parameter range.
    pub fn new(start: f64, end: f64, step: f64) -> Result<Self, SonanceFitError> {
        if !start.is_finite() || !end.is_finite() || !step.is_finite() {
            return Err(SonanceFitError::NonFiniteParameter);
        }
        if step <= 0.0 || end < start {
            return Err(SonanceFitError::InvalidParameterRange);
        }
        Ok(Self { start, end, step })
    }

    fn values(self) -> Vec<f64> {
        let mut values = Vec::new();
        let mut current = self.start;
        while current <= self.end + self.step * 0.5 {
            values.push(round_parameter(current));
            current += self.step;
        }
        values
    }
}

/// Tunable two-slope partial roughness parameter set.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PartialRoughnessParameters {
    /// First exponential slope in the critical-band roughness curve.
    pub a: f64,
    /// Second exponential slope in the critical-band roughness curve.
    pub b: f64,
}

impl PartialRoughnessParameters {
    /// Returns true when the parameter pair is finite and ordered.
    pub fn is_valid(self) -> bool {
        self.a.is_finite() && self.b.is_finite() && self.a > 0.0 && self.b > self.a
    }
}

/// Parameter grid for the `partial-roughness` sonance model family.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PartialRoughnessGrid {
    /// Range for the first exponential slope.
    pub a: ParameterRange,
    /// Range for the second exponential slope.
    pub b: ParameterRange,
}

/// Metadata for a held sonance corpus split.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SonanceCorpusMeta {
    /// Stable corpus split identifier.
    pub id: &'static str,
    /// Fixture license identifier or provenance license.
    pub license: &'static str,
    /// Stable hash of the authored fixture observations, not a model weight.
    pub corpus_hash: String,
    /// Number of observations in this split.
    pub observation_count: usize,
}

/// One ranked sonance observation in a corpus split.
#[derive(Clone, Debug, PartialEq)]
pub struct SonanceObservation {
    /// Stable observation identifier.
    pub id: &'static str,
    /// Partial-frequency and amplitude bins for the observation.
    pub bins: Vec<(Frequency, Amplitude)>,
    /// Target rank where lower means more consonant.
    pub target_rank: f64,
}

/// A named split with observation data and evidence metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct SonanceCorpusSplit {
    /// Metadata for the split.
    pub meta: SonanceCorpusMeta,
    /// Ranked observations held by this split.
    pub observations: Vec<SonanceObservation>,
}

/// Training, validation, and locked conformance fixtures for a fit.
#[derive(Clone, Debug, PartialEq)]
pub struct SonanceFitCorpus {
    /// Training split used by the search objective.
    pub training: SonanceCorpusSplit,
    /// Held-out validation split used only after candidate selection.
    pub validation: SonanceCorpusSplit,
    /// Locked conformance split that publishes catalog ordering evidence.
    pub locked_conformance: SonanceCorpusSplit,
}

/// Candidate score and objective components for one parameter set.
#[derive(Clone, Debug, PartialEq)]
pub struct SonanceFitCandidate {
    /// Fitted parameters.
    pub parameters: PartialRoughnessParameters,
    /// Training objective components.
    pub training: SonanceObjectiveReport,
    /// Held-out validation objective components.
    pub validation: SonanceObjectiveReport,
    /// Locked conformance objective components.
    pub locked_conformance: SonanceObjectiveReport,
}

/// Objective components for one corpus split.
#[derive(Clone, Debug, PartialEq)]
pub struct SonanceObjectiveReport {
    /// Objective label.
    pub objective: &'static str,
    /// Spearman rank correlation in `-1.0..=1.0`.
    pub rank_correlation: f64,
    /// Mean squared rank residual.
    pub mean_squared_residual: f64,
    /// Population variance of rank residuals.
    pub residual_variance: f64,
    /// Number of scored observations.
    pub observations: usize,
}

/// Full fitting report for a bounded sonance parameter search.
#[derive(Clone, Debug, PartialEq)]
pub struct SonanceFitReport {
    /// Model family label.
    pub model: &'static str,
    /// Search strategy label.
    pub strategy: &'static str,
    /// Objective label.
    pub objective: &'static str,
    /// Candidate parameter grid.
    pub grid: PartialRoughnessGrid,
    /// Search receipt returned by `sim-lib-discrete-search`.
    pub receipt: SearchReceipt,
    /// Stable report digest covering corpus hashes, receipt, and retained candidates.
    pub digest: String,
    /// Corpus metadata for training, validation, and locked conformance.
    pub corpora: Vec<SonanceCorpusMeta>,
    /// Best retained candidates, ordered by objective and stable parameters.
    pub candidates: Vec<SonanceFitCandidate>,
}

/// Error raised when fitting cannot run or score a candidate.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum SonanceFitError {
    /// A parameter or score was non-finite.
    #[error("sonance fitting parameters and scores must be finite")]
    NonFiniteParameter,
    /// A parameter range was empty or had a non-positive step.
    #[error("sonance fitting parameter ranges must be increasing with positive steps")]
    InvalidParameterRange,
    /// A corpus split had fewer than two ranked observations.
    #[error("sonance fitting corpus splits need at least two observations")]
    InsufficientCorpus,
    /// Acoustic input failed checked dissonance validation.
    #[error("sonance fitting acoustic input failed: {0}")]
    Input(String),
}

impl From<DissonanceInputError> for SonanceFitError {
    fn from(error: DissonanceInputError) -> Self {
        Self::Input(error.to_string())
    }
}

/// Fits the `partial-roughness` sonance model family with the concrete roadmap
/// configuration.
///
/// This corresponds to:
///
/// ```text
/// (sonance/fit
///   :model 'partial-roughness
///   :parameters {:a '(2.0 5.0 0.05) :b '(4.0 12.0 0.1)}
///   :objective 'rank-correlation
///   :control {:work 50000 :results 8 :seed 7})
/// ```
pub fn fit_partial_roughness_catalog() -> Result<SonanceFitReport, SonanceFitError> {
    let control = SearchControl::default()
        .with_max_work(50_000)
        .with_max_results(8)
        .with_seed(7);
    let grid = PartialRoughnessGrid {
        a: ParameterRange::new(2.0, 5.0, 0.05)?,
        b: ParameterRange::new(4.0, 12.0, 0.1)?,
    };
    fit_sonance_model(
        SonanceFitStrategy::BruteForce,
        SonanceFitObjective::RankCorrelation,
        grid,
        control,
        locked_partial_roughness_corpus(),
    )
}

/// Fits the `partial-roughness` sonance model with a bounded SearchControl.
pub fn fit_sonance_model(
    strategy: SonanceFitStrategy,
    objective: SonanceFitObjective,
    grid: PartialRoughnessGrid,
    control: SearchControl,
    corpus: SonanceFitCorpus,
) -> Result<SonanceFitReport, SonanceFitError> {
    validate_split(&corpus.training)?;
    validate_split(&corpus.validation)?;
    validate_split(&corpus.locked_conformance)?;
    let candidates = candidate_order(strategy, grid, control.seed);
    let run = solve(
        &SonanceFitProblem {
            candidates,
            objective,
            corpus: &corpus,
        },
        search_control_for_full_scoring(&control),
        &NeverInterrupt,
    );
    Ok(report_from_run(
        strategy, objective, grid, control, corpus, run,
    ))
}

/// Returns the owner-local locked fixture corpus metadata and observations.
///
/// The fixtures are short authored ordering observations under MPL-2.0. Their
/// hashes identify the corpus content; no opaque trained model weights are
/// bundled.
pub fn locked_partial_roughness_corpus() -> SonanceFitCorpus {
    let training = vec![
        observation("train-octave", &[440.0, 880.0], 0.0),
        observation("train-fifth", &[440.0, 660.0], 1.0),
        observation("train-major-third", &[440.0, 550.0], 2.0),
        observation("train-minor-second", &[440.0, 466.1637615], 3.0),
    ];
    let validation = vec![
        observation("validation-unison", &[523.2511306, 523.2511306], 0.0),
        observation("validation-fourth", &[523.2511306, 697.6537180], 1.0),
        observation("validation-tritone", &[523.2511306, 739.9888454], 2.0),
        observation("validation-semitone", &[523.2511306, 554.3652620], 3.0),
    ];
    let locked = vec![
        observation("locked-octave", &[330.0, 660.0], 0.0),
        observation("locked-fifth", &[330.0, 495.0], 1.0),
        observation("locked-fourth", &[330.0, 440.0], 2.0),
        observation("locked-tritone", &[330.0, 466.1637615], 3.0),
        observation("locked-minor-second", &[330.0, 349.2282314], 4.0),
    ];
    SonanceFitCorpus {
        training: split("partial-roughness-training-v1", training),
        validation: split("partial-roughness-validation-v1", validation),
        locked_conformance: split("partial-roughness-locked-conformance-v1", locked),
    }
}

fn report_from_run(
    strategy: SonanceFitStrategy,
    objective: SonanceFitObjective,
    grid: PartialRoughnessGrid,
    requested_control: SearchControl,
    corpus: SonanceFitCorpus,
    run: SearchRun<SonanceFitCandidate>,
) -> SonanceFitReport {
    let mut candidates = run.outputs;
    candidates.sort_by(compare_candidates);
    if let Some(limit) = requested_control.max_results {
        candidates.truncate(limit);
    }
    let corpora = vec![
        corpus.training.meta,
        corpus.validation.meta,
        corpus.locked_conformance.meta,
    ];
    let digest = report_digest(&run.receipt, &corpora, &candidates);
    SonanceFitReport {
        model: "partial-roughness",
        strategy: strategy.label(),
        objective: objective.label(),
        grid,
        receipt: run.receipt,
        digest,
        corpora,
        candidates,
    }
}

fn search_control_for_full_scoring(control: &SearchControl) -> SearchControl {
    let mut search = control.clone();
    search.max_results = None;
    search
}

struct SonanceFitProblem<'a> {
    candidates: Vec<PartialRoughnessParameters>,
    objective: SonanceFitObjective,
    corpus: &'a SonanceFitCorpus,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct FitState {
    candidate: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FitChoice(usize);

impl SearchProblem for SonanceFitProblem<'_> {
    type State = FitState;
    type Choice = FitChoice;
    type Output = SonanceFitCandidate;

    fn initial_state(&self) -> Self::State {
        FitState::default()
    }

    fn expand(&self, state: &Self::State, out: &mut Vec<Self::Choice>) {
        if state.candidate.is_none() {
            out.extend((0..self.candidates.len()).map(FitChoice));
        }
    }

    fn apply(&self, _state: &Self::State, choice: &Self::Choice) -> SearchStep<Self::State> {
        if choice.0 >= self.candidates.len() {
            return SearchStep::infeasible("candidate outside parameter grid");
        }
        SearchStep::Continue(FitState {
            candidate: Some(choice.0),
        })
    }

    fn finish(&self, state: &Self::State) -> Option<Self::Output> {
        let parameters = self.candidates[state.candidate?];
        Some(SonanceFitCandidate {
            parameters,
            training: objective_report(parameters, self.objective, &self.corpus.training).ok()?,
            validation: objective_report(parameters, self.objective, &self.corpus.validation)
                .ok()?,
            locked_conformance: objective_report(
                parameters,
                self.objective,
                &self.corpus.locked_conformance,
            )
            .ok()?,
        })
    }

    fn output_score(&self, output: &Self::Output) -> Option<i64> {
        Some(candidate_score_key(output))
    }
}

fn candidate_order(
    strategy: SonanceFitStrategy,
    grid: PartialRoughnessGrid,
    seed: u64,
) -> Vec<PartialRoughnessParameters> {
    let brute = brute_force_candidates(grid);
    match strategy {
        SonanceFitStrategy::BruteForce => brute,
        SonanceFitStrategy::Coordinate => coordinate_candidates(grid, brute),
        SonanceFitStrategy::BoundedStochastic => stochastic_candidates(brute, seed),
    }
}

fn brute_force_candidates(grid: PartialRoughnessGrid) -> Vec<PartialRoughnessParameters> {
    let mut candidates = Vec::new();
    for a in grid.a.values() {
        for b in grid.b.values() {
            let parameters = PartialRoughnessParameters { a, b };
            if parameters.is_valid() {
                candidates.push(parameters);
            }
        }
    }
    candidates
}

fn coordinate_candidates(
    grid: PartialRoughnessGrid,
    brute: Vec<PartialRoughnessParameters>,
) -> Vec<PartialRoughnessParameters> {
    let mid_a = midpoint(grid.a);
    let mid_b = midpoint(grid.b);
    let mut ordered = Vec::new();
    for a in grid.a.values() {
        ordered.push(PartialRoughnessParameters { a, b: mid_b });
    }
    for b in grid.b.values() {
        ordered.push(PartialRoughnessParameters { a: mid_a, b });
    }
    ordered.extend(brute);
    dedup_candidates(ordered)
}

fn stochastic_candidates(
    mut candidates: Vec<PartialRoughnessParameters>,
    seed: u64,
) -> Vec<PartialRoughnessParameters> {
    candidates.sort_by_key(|candidate| seeded_candidate_key(*candidate, seed));
    candidates
}

fn dedup_candidates(
    candidates: Vec<PartialRoughnessParameters>,
) -> Vec<PartialRoughnessParameters> {
    let mut deduped = Vec::new();
    for candidate in candidates {
        if candidate.is_valid() && !deduped.contains(&candidate) {
            deduped.push(candidate);
        }
    }
    deduped
}

fn midpoint(range: ParameterRange) -> f64 {
    round_parameter((range.start + range.end) * 0.5)
}

fn objective_report(
    parameters: PartialRoughnessParameters,
    objective: SonanceFitObjective,
    split: &SonanceCorpusSplit,
) -> Result<SonanceObjectiveReport, SonanceFitError> {
    validate_split(split)?;
    let predicted = split
        .observations
        .iter()
        .map(|observation| score_observation(parameters, observation))
        .collect::<Result<Vec<_>, _>>()?;
    let targets = split
        .observations
        .iter()
        .map(|observation| observation.target_rank)
        .collect::<Vec<_>>();
    let predicted_ranks = ranks(&predicted);
    let target_ranks = ranks(&targets);
    let residuals = predicted_ranks
        .iter()
        .zip(target_ranks.iter())
        .map(|(predicted, target)| predicted - target)
        .collect::<Vec<_>>();
    let mean_squared_residual =
        residuals.iter().map(|value| value * value).sum::<f64>() / residuals.len() as f64;
    Ok(SonanceObjectiveReport {
        objective: objective.label(),
        rank_correlation: spearman(&predicted_ranks, &target_ranks),
        mean_squared_residual,
        residual_variance: variance(&residuals),
        observations: split.observations.len(),
    })
}

fn score_observation(
    parameters: PartialRoughnessParameters,
    observation: &SonanceObservation,
) -> Result<f64, SonanceFitError> {
    let pairs = partial_pair_roughness_with_parameters(&observation.bins, parameters)?;
    Ok(pairs.iter().map(|pair| pair.roughness).sum::<f64>())
}

fn partial_pair_roughness_with_parameters(
    bins: &[(Frequency, Amplitude)],
    parameters: PartialRoughnessParameters,
) -> Result<Vec<PairRoughness>, SonanceFitError> {
    if !parameters.is_valid() {
        return Err(SonanceFitError::InvalidParameterRange);
    }
    let base = partial_pair_roughness(bins, PsychoacousticCurveFamily::PlompLevelt)?;
    Ok(base
        .into_iter()
        .map(|pair| PairRoughness {
            roughness: pair.roughness
                * parameterized_curve_ratio(pair.left_frequency, pair.right_frequency, parameters),
            ..pair
        })
        .collect())
}

fn parameterized_curve_ratio(
    left: Frequency,
    right: Frequency,
    parameters: PartialRoughnessParameters,
) -> f64 {
    let baseline = critical_band_value(left, right, 3.5, 5.75).abs();
    let tuned = critical_band_value(left, right, parameters.a, parameters.b).abs();
    if baseline <= f64::EPSILON {
        tuned
    } else {
        tuned / baseline
    }
}

fn critical_band_value(left: Frequency, right: Frequency, a: f64, b: f64) -> f64 {
    let min_freq = left.0.min(right.0).max(1.0);
    let s = 0.24 / (0.021 * min_freq + 19.0);
    let x = (right.0 - left.0).abs() * s;
    (-a * x).exp() - (-b * x).exp()
}

fn validate_split(split: &SonanceCorpusSplit) -> Result<(), SonanceFitError> {
    if split.observations.len() < 2 {
        return Err(SonanceFitError::InsufficientCorpus);
    }
    for observation in &split.observations {
        if !observation.target_rank.is_finite() {
            return Err(SonanceFitError::NonFiniteParameter);
        }
        for (frequency, amplitude) in &observation.bins {
            if !frequency.0.is_finite() || !amplitude.0.is_finite() {
                return Err(SonanceFitError::NonFiniteParameter);
            }
        }
    }
    Ok(())
}

fn ranks(values: &[f64]) -> Vec<f64> {
    let mut indexed = values
        .iter()
        .copied()
        .enumerate()
        .collect::<Vec<(usize, f64)>>();
    indexed.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut ranks = vec![0.0; values.len()];
    for (rank, (index, _)) in indexed.into_iter().enumerate() {
        ranks[index] = rank as f64;
    }
    ranks
}

fn spearman(left: &[f64], right: &[f64]) -> f64 {
    let left_mean = mean(left);
    let right_mean = mean(right);
    let mut covariance = 0.0;
    let mut left_var = 0.0;
    let mut right_var = 0.0;
    for (left_value, right_value) in left.iter().zip(right.iter()) {
        let left_delta = left_value - left_mean;
        let right_delta = right_value - right_mean;
        covariance += left_delta * right_delta;
        left_var += left_delta * left_delta;
        right_var += right_delta * right_delta;
    }
    if left_var <= f64::EPSILON || right_var <= f64::EPSILON {
        0.0
    } else {
        covariance / (left_var.sqrt() * right_var.sqrt())
    }
}

fn variance(values: &[f64]) -> f64 {
    let mean = mean(values);
    values
        .iter()
        .map(|value| {
            let delta = value - mean;
            delta * delta
        })
        .sum::<f64>()
        / values.len().max(1) as f64
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len().max(1) as f64
}

fn split(id: &'static str, observations: Vec<SonanceObservation>) -> SonanceCorpusSplit {
    let corpus_hash = corpus_digest(id, &observations);
    SonanceCorpusSplit {
        meta: SonanceCorpusMeta {
            id,
            license: "MPL-2.0",
            corpus_hash,
            observation_count: observations.len(),
        },
        observations,
    }
}

fn observation(id: &'static str, frequencies: &[f64], target_rank: f64) -> SonanceObservation {
    SonanceObservation {
        id,
        bins: frequencies
            .iter()
            .map(|frequency| (Frequency(*frequency), Amplitude(1.0)))
            .collect(),
        target_rank,
    }
}

fn compare_candidates(
    left: &SonanceFitCandidate,
    right: &SonanceFitCandidate,
) -> std::cmp::Ordering {
    candidate_score_key(left)
        .cmp(&candidate_score_key(right))
        .then_with(|| left.parameters.a.total_cmp(&right.parameters.a))
        .then_with(|| left.parameters.b.total_cmp(&right.parameters.b))
}

fn candidate_score_key(candidate: &SonanceFitCandidate) -> i64 {
    let score = -candidate.training.rank_correlation * 1_000_000.0
        + candidate.training.mean_squared_residual * 1_000.0
        + candidate.validation.mean_squared_residual;
    score.round() as i64
}

fn seeded_candidate_key(candidate: PartialRoughnessParameters, seed: u64) -> u64 {
    let material = format!("{:.6}:{:.6}:{seed}", candidate.a, candidate.b);
    stable_digest_value(&[material.as_str()])
}

fn round_parameter(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}
