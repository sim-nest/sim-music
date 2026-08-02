//! Finite prime-vector rank/unrank over discrete mixed-radix ordinals.

use num_bigint::BigUint;
use sim_lib_discrete_comb::{CombError, mixed_radix_rank, mixed_radix_unrank};

use crate::{FactorVector, PitchRatio, PitchRatioError, RatioPolicy, model::primes_up_to};

/// Largest absolute exponent represented by finite rank/unrank.
pub const MAX_RANK_EXPONENT: i16 = 31;

/// Rank a ratio's bounded signed prime vector with discrete mixed-radix ranking.
pub fn rank_ratio(ratio: PitchRatio, policy: RatioPolicy) -> Result<BigUint, PitchRatioError> {
    let vector = ratio.factor_vector(policy)?;
    let (digits, radices) = vector_digits(&vector)?;
    Ok(mixed_radix_rank(&digits, &radices)?)
}

/// Unrank a ratio from a finite signed prime-exponent vector domain.
pub fn unrank_ratio(rank: &BigUint, policy: RatioPolicy) -> Result<PitchRatio, PitchRatioError> {
    let Some(prime_limit) = policy.prime_limit else {
        return Err(PitchRatioError::UnboundedFactorization);
    };
    let primes = primes_up_to(prime_limit)?;
    let radix = u64::from((MAX_RANK_EXPONENT * 2 + 1) as u16);
    let radices = vec![radix; primes.len()];
    let digits = mixed_radix_unrank(rank, &radices)?;
    let exponents = digits
        .into_iter()
        .map(|digit| {
            let shifted = i64::try_from(digit).map_err(|_| PitchRatioError::Overflow)?
                - i64::from(MAX_RANK_EXPONENT);
            i16::try_from(shifted).map_err(|_| PitchRatioError::Overflow)
        })
        .collect::<Result<Vec<_>, _>>()?;
    FactorVector { primes, exponents }
        .to_ratio()?
        .canonical(policy)
}

fn vector_digits(vector: &FactorVector) -> Result<(Vec<u64>, Vec<u64>), PitchRatioError> {
    let radix = u64::from((MAX_RANK_EXPONENT * 2 + 1) as u16);
    let mut digits = Vec::with_capacity(vector.exponents.len());
    for &exponent in &vector.exponents {
        if !(-MAX_RANK_EXPONENT..=MAX_RANK_EXPONENT).contains(&exponent) {
            return Err(PitchRatioError::RankExponentOutOfRange {
                exponent,
                min: -MAX_RANK_EXPONENT,
                max: MAX_RANK_EXPONENT,
            });
        }
        let shifted = i32::from(exponent) + i32::from(MAX_RANK_EXPONENT);
        digits.push(u64::try_from(shifted).map_err(|_| PitchRatioError::Overflow)?);
    }
    Ok((digits, vec![radix; vector.exponents.len()]))
}

impl From<CombError> for PitchRatioError {
    fn from(error: CombError) -> Self {
        Self::DiscreteRank(error.to_string())
    }
}
