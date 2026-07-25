//! Exact positive musical ratio intervals.
//!
//! [`PitchRatio`] stores a reduced positive rational interval over `u64`.
//! [`RatioPolicy`] controls octave equivalence and prime-limit admissibility,
//! [`rank_ratio`] / [`unrank_ratio`] map bounded prime-exponent vectors through
//! the discrete mixed-radix helpers, and [`approximate_ratio`] searches bounded
//! candidates under `sim-lib-discrete-search` receipts.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod approximate;
mod error;
mod model;
mod rank;

pub use approximate::{
    ApproximationStrategy, DEFAULT_APPROXIMATION_BOUND, RatioApproximation, approximate_ratio,
    approximate_ratio_with_strategy,
};
pub use error::PitchRatioError;
pub use model::{FactorVector, MAX_PRIME_LIMIT, PitchRatio, RatioPolicy};
pub use rank::{MAX_RANK_EXPONENT, rank_ratio, unrank_ratio};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
