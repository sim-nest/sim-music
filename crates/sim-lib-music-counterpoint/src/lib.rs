//! Exact counterpoint analysis and bounded constraint generation.
//!
//! [`analyze_counterpoint`] inspects material that already exists, while
//! [`generate_counterpoint`] compiles the same rule data to explicit finite CSP
//! variables and pitch domains consumed by `sim-lib-discrete-search`. Generated
//! voices are strictly additive [`sim_lib_music_consonance::ConsonancePatch`]
//! values whose inverse restores the fixed cantus exactly. [`stretto_graph`]
//! remains a separate derived-analysis surface backed by
//! `sim-lib-discrete-graph`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod analysis;
mod generator;
mod generator_materialization;
mod model;
mod rule;
mod runtime;
mod runtime_generation;
mod runtime_graph_expr;
mod stretto;

pub use analysis::analyze_counterpoint;
pub use generator::{compile_counterpoint_csp, generate_counterpoint};
pub use model::*;
pub use rule::*;
pub use runtime::{
    MusicCounterpointLib, install_music_counterpoint_lib, music_counterpoint_analyze_symbol,
    music_counterpoint_generate_symbol, music_stretto_graph_symbol,
};
pub use stretto::{cluster_overlap, fuse_stretto_entries, materialize_transform, stretto_graph};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
