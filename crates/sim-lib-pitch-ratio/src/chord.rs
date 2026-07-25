//! Exact ratio chord matrices, costs, and coverage.

use std::collections::BTreeSet;

use crate::{PitchRatio, PitchRatioError, RatioPolicy};

/// Generalized-mean cost dialect.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MeanDialect {
    /// Normalize the generalized mean by the number of measured intervals.
    #[default]
    Standard,
    /// Preserve the legacy tuned-recipe sum of powers without dividing by count.
    LegacyTunedNoDivision,
}

/// Coverage summary for a ratio chord matrix.
#[derive(Clone, Debug, PartialEq)]
pub struct RatioCoverage {
    /// Policy used while admitting root-normalized tones and intervals.
    pub policy: RatioPolicy,
    /// Number of input tones admitted under the policy.
    pub admitted_tones: usize,
    /// Number of input tones rejected under the policy.
    pub rejected_tones: usize,
    /// Number of directed matrix entries.
    pub matrix_entries: usize,
    /// Number of distinct exact directed intervals in the matrix.
    pub distinct_intervals: usize,
    /// Number of distinct octave-reduced interval classes in the matrix.
    pub octave_classes: usize,
    /// Number of matrix intervals rejected by the policy.
    pub rejected_intervals: usize,
}

/// Exact chord-ratio analysis report.
#[derive(Clone, Debug, PartialEq)]
pub struct RatioChordReport {
    /// Directed interval matrix, where `matrix[i][j] = tone[j] / tone[i]`.
    pub matrix: Vec<Vec<PitchRatio>>,
    /// Generalized-mean interval complexity cost.
    pub cost: f64,
    /// Exact coverage of admitted tones and intervals.
    pub covered: RatioCoverage,
}

/// Analyze a chord using root index 0, standard generalized mean, and exponent 2.
pub fn analyze_ratio_chord(
    tones: &[PitchRatio],
    policy: RatioPolicy,
) -> Result<RatioChordReport, PitchRatioError> {
    analyze_ratio_chord_with_root(tones, 0, policy, 2.0, MeanDialect::Standard)
}

/// Analyze a chord with an explicit root/reference tone and cost dialect.
pub fn analyze_ratio_chord_with_root(
    tones: &[PitchRatio],
    root_index: usize,
    policy: RatioPolicy,
    mean_exponent: f64,
    dialect: MeanDialect,
) -> Result<RatioChordReport, PitchRatioError> {
    let normalized = root_normalized_tones(tones, root_index, policy)?;
    let matrix = ratio_interval_matrix(&normalized, policy)?;
    let cost = generalized_mean_chord_cost(&matrix, policy, mean_exponent, dialect)?;
    let covered = ratio_coverage(tones, &matrix, policy);
    Ok(RatioChordReport {
        matrix,
        cost,
        covered,
    })
}

/// Normalize every tone against the declared root/reference tone.
pub fn root_normalized_tones(
    tones: &[PitchRatio],
    root_index: usize,
    policy: RatioPolicy,
) -> Result<Vec<PitchRatio>, PitchRatioError> {
    if tones.is_empty() {
        return Err(PitchRatioError::EmptyChord);
    }
    let root = tones
        .get(root_index)
        .copied()
        .ok_or(PitchRatioError::InvalidRootIndex {
            root_index,
            len: tones.len(),
        })?;
    tones
        .iter()
        .map(|tone| tone.divide(root)?.canonical(policy))
        .collect()
}

/// Build a directed interval matrix, where `matrix[i][j] = tone[j] / tone[i]`.
pub fn ratio_interval_matrix(
    tones: &[PitchRatio],
    policy: RatioPolicy,
) -> Result<Vec<Vec<PitchRatio>>, PitchRatioError> {
    if tones.is_empty() {
        return Err(PitchRatioError::EmptyChord);
    }
    tones
        .iter()
        .map(|from| {
            tones
                .iter()
                .map(|to| to.divide(*from)?.canonical(policy))
                .collect()
        })
        .collect()
}

/// Compute generalized-mean chord cost from matrix interval complexity.
pub fn generalized_mean_chord_cost(
    matrix: &[Vec<PitchRatio>],
    policy: RatioPolicy,
    mean_exponent: f64,
    dialect: MeanDialect,
) -> Result<f64, PitchRatioError> {
    if !mean_exponent.is_finite() || mean_exponent == 0.0 {
        return Err(PitchRatioError::InvalidMeanExponent);
    }
    let mut sum = 0.0;
    let mut count = 0usize;
    for (row_index, row) in matrix.iter().enumerate() {
        for (column_index, ratio) in row.iter().enumerate() {
            if row_index == column_index {
                continue;
            }
            let complexity = ratio_complexity(*ratio, policy)? as f64;
            sum += complexity.powf(mean_exponent);
            count += 1;
        }
    }
    if count == 0 {
        return Ok(0.0);
    }
    let mean_power = match dialect {
        MeanDialect::Standard => sum / count as f64,
        MeanDialect::LegacyTunedNoDivision => sum,
    };
    Ok(mean_power.powf(1.0 / mean_exponent))
}

/// Measure exact and octave-class coverage for a matrix.
pub fn ratio_coverage(
    input_tones: &[PitchRatio],
    matrix: &[Vec<PitchRatio>],
    policy: RatioPolicy,
) -> RatioCoverage {
    let mut distinct_intervals = BTreeSet::new();
    let mut octave_classes = BTreeSet::new();
    let mut rejected_intervals = 0usize;

    for ratio in matrix.iter().flatten().copied() {
        distinct_intervals.insert(ratio);
        if let Ok(canonical) = ratio.canonical(policy) {
            octave_classes.insert(canonical);
        } else {
            rejected_intervals += 1;
        }
    }

    let admitted_tones = input_tones
        .iter()
        .filter(|tone| tone.canonical(policy).is_ok())
        .count();

    RatioCoverage {
        policy,
        admitted_tones,
        rejected_tones: input_tones.len().saturating_sub(admitted_tones),
        matrix_entries: matrix.iter().map(Vec::len).sum(),
        distinct_intervals: distinct_intervals.len(),
        octave_classes: octave_classes.len(),
        rejected_intervals,
    }
}

fn ratio_complexity(ratio: PitchRatio, policy: RatioPolicy) -> Result<u32, PitchRatioError> {
    Ok(ratio
        .factor_vector(policy)?
        .exponents
        .iter()
        .map(|exponent| u32::from(exponent.unsigned_abs()))
        .sum())
}
