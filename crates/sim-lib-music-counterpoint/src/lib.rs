//! Exact counterpoint rule reports and graph-backed stretto analysis.
//!
//! This crate keeps two concerns deliberately distinct. [`analyze_counterpoint`]
//! inspects material that already exists, aligns every voice at exact rational
//! note boundaries, and returns one evidence-bearing [`Violation`] per failed
//! rule. [`stretto_graph`] derives bounded transform candidates from a subject
//! and relates compatible entries through `sim-lib-discrete-graph`; those
//! candidates are analysis values, not generated counterpoint. Species and open
//! policies are ordinary [`RuleSet`] data, while pitch/time transformations are
//! delegated to `sim-lib-music-transform`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod analysis;
mod model;
mod rule;
mod runtime;
mod runtime_graph_expr;
mod stretto;

pub use analysis::analyze_counterpoint;
pub use model::*;
pub use rule::*;
pub use runtime::{
    MusicCounterpointLib, install_music_counterpoint_lib, music_counterpoint_analyze_symbol,
    music_stretto_graph_symbol,
};
pub use stretto::{cluster_overlap, fuse_stretto_entries, materialize_transform, stretto_graph};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
