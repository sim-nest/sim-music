//! Bounded ratio approximation search.

use sim_lib_discrete_search::{
    NeverInterrupt, SearchControl, SearchProblem, SearchRun, SearchStep, solve,
};

use crate::{PitchRatio, RatioPolicy};

/// Default numerator/denominator bound used by approximation search.
pub const DEFAULT_APPROXIMATION_BOUND: u64 = 256;

/// Approximation search ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ApproximationStrategy {
    /// Return candidates ordered by absolute cents error.
    Nearest,
    /// Return the first admissible candidates in ascending denominator/numerator order.
    First,
    /// Balance low error with smaller numerator/denominator complexity.
    Balanced,
}

/// One bounded approximation result.
#[derive(Clone, Debug, PartialEq)]
pub struct RatioApproximation {
    /// Candidate exact ratio.
    pub ratio: PitchRatio,
    /// Candidate cents.
    pub cents: f64,
    /// Signed cents error against the target.
    pub error_cents: f64,
    /// Strategy score used for deterministic ordering.
    pub score: i64,
}

/// Approximate a cents value using nearest-error ordering.
pub fn approximate_ratio(
    cents: f64,
    policy: RatioPolicy,
    control: SearchControl,
) -> SearchRun<RatioApproximation> {
    approximate_ratio_with_strategy(cents, policy, control, ApproximationStrategy::Nearest)
}

/// Approximate a cents value using an explicit deterministic strategy.
pub fn approximate_ratio_with_strategy(
    cents: f64,
    policy: RatioPolicy,
    control: SearchControl,
    strategy: ApproximationStrategy,
) -> SearchRun<RatioApproximation> {
    solve(
        &ApproximationProblem {
            target_cents: cents,
            policy,
            strategy,
            bound: DEFAULT_APPROXIMATION_BOUND,
        },
        control,
        &NeverInterrupt,
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ApproximationState {
    Root,
    Candidate(RatioApproximationKey),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RatioApproximationKey {
    score: i64,
    denominator: u64,
    numerator: u64,
}

struct ApproximationProblem {
    target_cents: f64,
    policy: RatioPolicy,
    strategy: ApproximationStrategy,
    bound: u64,
}

impl SearchProblem for ApproximationProblem {
    type State = ApproximationState;
    type Choice = RatioApproximationKey;
    type Output = RatioApproximation;

    fn initial_state(&self) -> Self::State {
        ApproximationState::Root
    }

    fn expand(&self, state: &Self::State, out: &mut Vec<Self::Choice>) {
        if !matches!(state, ApproximationState::Root) {
            return;
        }
        for denominator in 1..=self.bound {
            for numerator in 1..=self.bound {
                let Ok(ratio) = PitchRatio::new(numerator, denominator) else {
                    continue;
                };
                let Ok(ratio) = ratio.canonical(self.policy) else {
                    continue;
                };
                if ratio.numerator() != numerator || ratio.denominator() != denominator {
                    continue;
                }
                let error = ratio.cents() - self.target_cents;
                out.push(RatioApproximationKey {
                    score: approximation_score(ratio, error, self.strategy),
                    denominator,
                    numerator,
                });
            }
        }
    }

    fn apply(&self, state: &Self::State, choice: &Self::Choice) -> SearchStep<Self::State> {
        if !matches!(state, ApproximationState::Root) {
            return SearchStep::pruned("candidate leaves do not expand");
        }
        SearchStep::Continue(ApproximationState::Candidate(choice.clone()))
    }

    fn finish(&self, state: &Self::State) -> Option<Self::Output> {
        let ApproximationState::Candidate(candidate) = state else {
            return None;
        };
        let ratio = PitchRatio::new(candidate.numerator, candidate.denominator)
            .ok()?
            .canonical(self.policy)
            .ok()?;
        let cents = ratio.cents();
        let error_cents = cents - self.target_cents;
        Some(RatioApproximation {
            ratio,
            cents,
            error_cents,
            score: candidate.score,
        })
    }

    fn score_state(&self, state: &Self::State) -> i64 {
        match state {
            ApproximationState::Root => 0,
            ApproximationState::Candidate(candidate) => candidate.score,
        }
    }

    fn output_score(&self, output: &Self::Output) -> Option<i64> {
        Some(output.score)
    }
}

fn approximation_score(
    ratio: PitchRatio,
    error_cents: f64,
    strategy: ApproximationStrategy,
) -> i64 {
    match strategy {
        ApproximationStrategy::Nearest => (error_cents.abs() * 1_000_000.0).round() as i64,
        ApproximationStrategy::First => {
            let denominator = i64::try_from(ratio.denominator()).unwrap_or(i64::MAX / 2);
            let numerator = i64::try_from(ratio.numerator()).unwrap_or(i64::MAX / 2);
            denominator.saturating_mul(10_000).saturating_add(numerator)
        }
        ApproximationStrategy::Balanced => {
            let complexity = ratio.numerator().saturating_add(ratio.denominator()) as f64;
            (error_cents.abs() * 1_000_000.0 + complexity * 1_000.0).round() as i64
        }
    }
}
