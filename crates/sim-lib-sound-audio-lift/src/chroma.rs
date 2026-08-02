//! Octave-folded pitch-class profiles over tuning-aligned constant-Q bins.

use crate::{AudioTransformError, ConstantQ, CqtReference, CqtWeighting, invalid};

/// How octave-related constant-Q bins combine into each chroma degree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromaFoldPolicy {
    /// Sum every admitted bin assigned to the degree.
    Sum,
    /// Average every admitted bin assigned to the degree.
    Mean,
    /// Retain the strongest admitted bin assigned to the degree.
    Maximum,
}

/// Post-fold normalization applied independently to each chroma frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromaNormalization {
    /// Preserve folded values.
    None,
    /// Normalize the absolute sum to one.
    L1,
    /// Normalize Euclidean energy to one.
    L2,
    /// Normalize the strongest degree to one.
    Maximum,
}

/// Explicit chroma folding and normalization policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChromaPlan {
    /// Octave-fold aggregation rule.
    pub folding: ChromaFoldPolicy,
    /// Per-frame normalization rule.
    pub normalization: ChromaNormalization,
}

impl Default for ChromaPlan {
    fn default() -> Self {
        Self {
            folding: ChromaFoldPolicy::Sum,
            normalization: ChromaNormalization::L1,
        }
    }
}

/// One octave-folded pitch-class profile.
#[derive(Clone, Debug, PartialEq)]
pub struct ChromaFrame {
    /// Zero-based frame index inherited from constant-Q analysis.
    pub index: usize,
    /// Signed frame timestamp in source-sample coordinates.
    pub onset_sample: i64,
    /// Tuning-degree values beginning with degree zero.
    pub bins: Vec<f64>,
}

/// Chroma output retaining its tuning, weighting, and folding policies.
#[derive(Clone, Debug, PartialEq)]
pub struct Chroma {
    /// Tuning/reference facts inherited from constant-Q analysis.
    pub reference: CqtReference,
    /// Constant-Q weighting used before folding.
    pub weighting: CqtWeighting,
    /// Explicit fold and normalization policy.
    pub plan: ChromaPlan,
    /// Octave-folded frames.
    pub frames: Vec<ChromaFrame>,
}

/// Folds tuning-aligned constant-Q bins into octave-independent chroma frames.
pub fn chroma(cqt: &ConstantQ, plan: &ChromaPlan) -> Result<Chroma, AudioTransformError> {
    let divisions = usize::try_from(cqt.reference.divisions)
        .map_err(|_| invalid("tuning divisions", "division count exceeds platform limits"))?;
    if divisions == 0
        || !cqt
            .plan
            .bins_per_octave
            .is_multiple_of(cqt.reference.divisions)
    {
        return Err(invalid(
            "chroma grid",
            "bins per octave must be a multiple of the tuning divisions",
        ));
    }
    let bins_per_degree = i64::from(cqt.plan.bins_per_octave / cqt.reference.divisions);
    let anchor = i64::from(cqt.reference.degree);
    let mut frames = Vec::with_capacity(cqt.frames.len());
    for frame in &cqt.frames {
        let initial = if plan.folding == ChromaFoldPolicy::Maximum {
            f64::NEG_INFINITY
        } else {
            0.0
        };
        let mut values = vec![initial; divisions];
        let mut counts = vec![0usize; divisions];
        for bin in &frame.bins {
            let degree_offset = rounded_div(i64::from(bin.reference_offset), bins_per_degree);
            let degree = (anchor + degree_offset).rem_euclid(divisions as i64) as usize;
            match plan.folding {
                ChromaFoldPolicy::Sum | ChromaFoldPolicy::Mean => values[degree] += bin.value,
                ChromaFoldPolicy::Maximum => values[degree] = values[degree].max(bin.value),
            }
            counts[degree] += 1;
        }
        if plan.folding == ChromaFoldPolicy::Mean {
            for (value, count) in values.iter_mut().zip(&counts) {
                if *count > 0 {
                    *value /= *count as f64;
                }
            }
        }
        for (value, count) in values.iter_mut().zip(&counts) {
            if *count == 0 {
                *value = 0.0;
            }
        }
        normalize_chroma(&mut values, plan.normalization);
        frames.push(ChromaFrame {
            index: frame.index,
            onset_sample: frame.onset_sample,
            bins: values,
        });
    }
    Ok(Chroma {
        reference: cqt.reference.clone(),
        weighting: cqt.plan.weighting,
        plan: *plan,
        frames,
    })
}

fn normalize_chroma(values: &mut [f64], policy: ChromaNormalization) {
    let divisor = match policy {
        ChromaNormalization::None => return,
        ChromaNormalization::L1 => values.iter().map(|value| value.abs()).sum::<f64>(),
        ChromaNormalization::L2 => values.iter().map(|value| value * value).sum::<f64>().sqrt(),
        ChromaNormalization::Maximum => values.iter().copied().fold(0.0_f64, f64::max),
    };
    if divisor.is_finite() && divisor > f64::EPSILON {
        for value in values {
            *value /= divisor;
        }
    }
}

fn rounded_div(numerator: i64, denominator: i64) -> i64 {
    let quotient = numerator.div_euclid(denominator);
    let remainder = numerator.rem_euclid(denominator);
    if 2 * remainder >= denominator {
        quotient + 1
    } else {
        quotient
    }
}
