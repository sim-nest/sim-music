//! Finite symbolic alphabets, validated series, and certified total transforms.
//!
//! This crate owns the pitch-independent serial contract: stable alphabet
//! identity, symbol-bearing series, aggregate policy data, validation evidence,
//! permutation rank delegation, and total evidence-producing transforms. Pitch
//! rows, score realization, search, and enumeration live in their domain owners.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod aggregate;
mod alphabet;
mod certificate;
mod error;
mod permutation;
mod series;
mod transform;

pub use aggregate::{
    AggregateLedger, AggregateRule, AggregateRuleKind, ProjectedClassEvidence, ProjectedClassSpec,
    ProjectionId, SymbolCount,
};
pub use alphabet::{AlphabetId, AlphabetRegistry, FiniteAlphabet, SerialAlphabet};
pub use certificate::{RelaxedInvariant, TransformCertificate, TransformedSeries};
pub use error::{
    AggregateRuleError, AlphabetError, BlockPartitionError, OrdinalMapError, SeriesError,
    SeriesTransformError, SymbolBijectionError,
};
pub use permutation::{BlockPartition, OrdinalMap, OrdinalPermutation};
pub use series::Series;
pub use transform::{SeriesTransform, SymbolBijection};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
#[cfg(test)]
mod transform_tests;
