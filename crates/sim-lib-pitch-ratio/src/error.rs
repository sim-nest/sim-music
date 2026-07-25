//! Error types for exact ratio work.

use crate::MAX_PRIME_LIMIT;

/// Error raised by pitch-ratio validation, factoring, ranking, or approximation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PitchRatioError {
    /// Ratios must have positive numerator and denominator.
    #[error("pitch ratios require positive numerator and denominator")]
    NonPositiveRatio,
    /// A checked integer operation overflowed.
    #[error("pitch ratio integer operation overflowed")]
    Overflow,
    /// Prime-limit factorization must be explicitly bounded.
    #[error("factorization requires a bounded prime limit")]
    UnboundedFactorization,
    /// The requested prime limit is outside the supported bounded domain.
    #[error("prime limit {0} is outside 2..={MAX_PRIME_LIMIT}")]
    InvalidPrimeLimit(u32),
    /// A factor remains above the configured prime limit.
    #[error(
        "ratio has factors above prime limit {prime_limit}: remaining {remaining_numerator}/{remaining_denominator}"
    )]
    PrimeLimitExceeded {
        /// Numerator remainder after bounded factoring.
        remaining_numerator: u64,
        /// Denominator remainder after bounded factoring.
        remaining_denominator: u64,
        /// Configured prime limit.
        prime_limit: u32,
    },
    /// A signed exponent exceeded the represented bound.
    #[error("prime exponent exceeded bounded rank domain")]
    ExponentOverflow,
    /// A factor vector is malformed.
    #[error("factor vector primes and exponents do not match")]
    InvalidFactorVector,
    /// A rank/unrank request exceeded the finite exponent domain.
    #[error("factor exponent {exponent} is outside {min}..={max}")]
    RankExponentOutOfRange {
        /// Observed exponent.
        exponent: i16,
        /// Minimum supported exponent.
        min: i16,
        /// Maximum supported exponent.
        max: i16,
    },
    /// Discrete mixed-radix ranking failed.
    #[error("discrete rank error: {0}")]
    DiscreteRank(String),
    /// A chord operation requires at least one ratio.
    #[error("ratio chord requires at least one ratio")]
    EmptyChord,
    /// A chord root index did not identify a ratio in the chord.
    #[error("ratio chord root index {root_index} is outside chord length {len}")]
    InvalidRootIndex {
        /// Requested root index.
        root_index: usize,
        /// Chord length.
        len: usize,
    },
    /// A generalized mean exponent must be finite and non-zero.
    #[error("generalized mean exponent must be finite and non-zero")]
    InvalidMeanExponent,
}
