//! Finite symbolic alphabets and validated ordered series.
//!
//! This crate owns the pitch-independent serial contract: stable alphabet
//! identity, symbol-bearing series, aggregate policy data, validation evidence,
//! and permutation rank delegation. Pitch rows, score realization, search, and
//! transforms live in their domain owners.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod aggregate;
mod alphabet;
mod error;
mod series;

pub use aggregate::{
    AggregateLedger, AggregateRule, AggregateRuleKind, ProjectedClassEvidence, ProjectedClassSpec,
    ProjectionId, SymbolCount,
};
pub use alphabet::{AlphabetId, AlphabetRegistry, FiniteAlphabet, SerialAlphabet};
pub use error::{AggregateRuleError, AlphabetError, SeriesError};
pub use series::Series;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
