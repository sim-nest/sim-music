//! Exact positive musical ratio intervals.
//!
//! [`PitchRatio`] stores a reduced positive rational interval over `u64`.
//! [`RatioPolicy`] controls octave equivalence and prime-limit admissibility,
//! [`rank_ratio`] / [`unrank_ratio`] map bounded prime-exponent vectors through
//! the discrete mixed-radix helpers, [`analyze_ratio_chord`] builds exact chord
//! interval matrices, and [`expand_ratio_relation_tree`] searches cycle-safe
//! relation paths under `sim-lib-discrete-search` receipts.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod approximate;
mod chord;
mod error;
mod model;
mod rank;
mod relation;

pub use approximate::{
    ApproximationStrategy, DEFAULT_APPROXIMATION_BOUND, RatioApproximation, approximate_ratio,
    approximate_ratio_with_strategy,
};
pub use chord::{
    MeanDialect, RatioChordReport, RatioCoverage, analyze_ratio_chord,
    analyze_ratio_chord_with_root, generalized_mean_chord_cost, ratio_coverage,
    ratio_interval_matrix, root_normalized_tones,
};
pub use error::PitchRatioError;
pub use model::{FactorVector, MAX_PRIME_LIMIT, PitchRatio, RatioPolicy};
pub use rank::{MAX_RANK_EXPONENT, rank_ratio, unrank_ratio};
pub use relation::{RatioRelation, RatioRelationPath, expand_ratio_relation_tree};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
