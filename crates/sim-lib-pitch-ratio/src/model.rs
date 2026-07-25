//! Ratio identity, policy, factors, and errors.

use crate::PitchRatioError;

/// Largest prime allowed for a bounded factor vector.
pub const MAX_PRIME_LIMIT: u32 = 97;

/// Exact positive reduced musical ratio.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PitchRatio {
    pub(crate) numerator: u64,
    pub(crate) denominator: u64,
}

impl PitchRatio {
    /// Construct a positive reduced ratio.
    pub fn new(numerator: u64, denominator: u64) -> Result<Self, PitchRatioError> {
        if numerator == 0 || denominator == 0 {
            return Err(PitchRatioError::NonPositiveRatio);
        }
        let divisor = gcd(numerator, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    /// Reduced numerator.
    pub const fn numerator(self) -> u64 {
        self.numerator
    }

    /// Reduced denominator.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    /// Return this ratio as a floating frequency multiplier.
    pub fn as_f64(self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }

    /// Exact unison ratio.
    pub const fn unison() -> Self {
        Self {
            numerator: 1,
            denominator: 1,
        }
    }

    /// Multiply two reduced ratios exactly.
    pub fn multiply(self, other: Self) -> Result<Self, PitchRatioError> {
        let numerator = self
            .numerator
            .checked_mul(other.numerator)
            .ok_or(PitchRatioError::Overflow)?;
        let denominator = self
            .denominator
            .checked_mul(other.denominator)
            .ok_or(PitchRatioError::Overflow)?;
        Self::new(numerator, denominator)
    }

    /// Divide this ratio by another reduced ratio exactly.
    pub fn divide(self, other: Self) -> Result<Self, PitchRatioError> {
        let numerator = self
            .numerator
            .checked_mul(other.denominator)
            .ok_or(PitchRatioError::Overflow)?;
        let denominator = self
            .denominator
            .checked_mul(other.numerator)
            .ok_or(PitchRatioError::Overflow)?;
        Self::new(numerator, denominator)
    }

    /// Convert this ratio to cents.
    pub fn cents(self) -> f64 {
        1200.0 * self.as_f64().log2()
    }

    /// Absolute cents error from a target.
    pub fn tuning_error_cents(self, target_cents: f64) -> f64 {
        (self.cents() - target_cents).abs()
    }

    /// Return this ratio folded into `[1, 2)` by octave equivalence.
    pub fn octave_reduced(self) -> Result<Self, PitchRatioError> {
        let mut numerator = self.numerator;
        let mut denominator = self.denominator;
        while numerator >= denominator.saturating_mul(2) {
            denominator = denominator
                .checked_mul(2)
                .ok_or(PitchRatioError::Overflow)?;
        }
        while numerator < denominator {
            numerator = numerator.checked_mul(2).ok_or(PitchRatioError::Overflow)?;
        }
        Self::new(numerator, denominator)
    }

    /// Apply a ratio policy to this interval.
    pub fn canonical(self, policy: RatioPolicy) -> Result<Self, PitchRatioError> {
        let ratio = if policy.octave_reduce {
            self.octave_reduced()?
        } else {
            self
        };
        if let Some(prime_limit) = policy.prime_limit {
            ratio.factor_vector(policy.with_prime_limit(prime_limit))?;
        }
        Ok(ratio)
    }

    /// Factor this ratio into signed exponents for primes up to the policy limit.
    pub fn factor_vector(self, policy: RatioPolicy) -> Result<FactorVector, PitchRatioError> {
        let Some(prime_limit) = policy.prime_limit else {
            return Err(PitchRatioError::UnboundedFactorization);
        };
        let primes = primes_up_to(prime_limit)?;
        let mut numerator = self.numerator;
        let mut denominator = self.denominator;
        let mut exponents = Vec::with_capacity(primes.len());
        for prime in &primes {
            let prime_u64 = u64::from(*prime);
            let mut exponent = 0i16;
            while numerator.is_multiple_of(prime_u64) {
                numerator /= prime_u64;
                exponent = exponent
                    .checked_add(1)
                    .ok_or(PitchRatioError::ExponentOverflow)?;
            }
            while denominator.is_multiple_of(prime_u64) {
                denominator /= prime_u64;
                exponent = exponent
                    .checked_sub(1)
                    .ok_or(PitchRatioError::ExponentOverflow)?;
            }
            exponents.push(exponent);
        }
        if numerator != 1 || denominator != 1 {
            return Err(PitchRatioError::PrimeLimitExceeded {
                remaining_numerator: numerator,
                remaining_denominator: denominator,
                prime_limit,
            });
        }
        Ok(FactorVector { primes, exponents })
    }
}

/// Ratio canonicalization and admissibility policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RatioPolicy {
    /// Fold ratios into one octave when true.
    pub octave_reduce: bool,
    /// Reject factors above this prime and require bounded factorization.
    pub prime_limit: Option<u32>,
}

impl RatioPolicy {
    /// Policy with octave reduction and a three-limit factor vector.
    pub const fn three_limit() -> Self {
        Self {
            octave_reduce: true,
            prime_limit: Some(3),
        }
    }

    /// Policy with octave reduction and a five-limit factor vector.
    pub const fn five_limit() -> Self {
        Self {
            octave_reduce: true,
            prime_limit: Some(5),
        }
    }

    /// Return this policy with a prime limit.
    pub const fn with_prime_limit(mut self, prime_limit: u32) -> Self {
        self.prime_limit = Some(prime_limit);
        self
    }
}

impl Default for RatioPolicy {
    fn default() -> Self {
        Self {
            octave_reduce: true,
            prime_limit: Some(13),
        }
    }
}

/// Signed prime-exponent vector for a ratio.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FactorVector {
    /// Prime basis in ascending order.
    pub primes: Vec<u32>,
    /// Signed exponents matching `primes`.
    pub exponents: Vec<i16>,
}

impl FactorVector {
    /// Rebuild the ratio represented by this factor vector.
    pub fn to_ratio(&self) -> Result<PitchRatio, PitchRatioError> {
        if self.primes.len() != self.exponents.len() {
            return Err(PitchRatioError::InvalidFactorVector);
        }
        let mut numerator = 1u64;
        let mut denominator = 1u64;
        for (&prime, &exponent) in self.primes.iter().zip(&self.exponents) {
            if exponent >= 0 {
                multiply_power(&mut numerator, prime, exponent as u16)?;
            } else {
                multiply_power(&mut denominator, prime, exponent.unsigned_abs())?;
            }
        }
        PitchRatio::new(numerator, denominator)
    }
}

pub(crate) fn primes_up_to(limit: u32) -> Result<Vec<u32>, PitchRatioError> {
    if !(2..=MAX_PRIME_LIMIT).contains(&limit) {
        return Err(PitchRatioError::InvalidPrimeLimit(limit));
    }
    Ok((2..=limit)
        .filter(|candidate| is_prime(*candidate))
        .collect())
}

fn is_prime(candidate: u32) -> bool {
    if candidate < 2 {
        return false;
    }
    let mut divisor = 2;
    while divisor * divisor <= candidate {
        if candidate.is_multiple_of(divisor) {
            return false;
        }
        divisor += 1;
    }
    true
}

pub(crate) fn multiply_power(
    target: &mut u64,
    prime: u32,
    exponent: u16,
) -> Result<(), PitchRatioError> {
    for _ in 0..exponent {
        *target = target
            .checked_mul(u64::from(prime))
            .ok_or(PitchRatioError::Overflow)?;
    }
    Ok(())
}

pub(crate) const fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}
