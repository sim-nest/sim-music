//! Psychoacoustic dissonance models for the SIM music constellation.
//!
//! This crate defines the [`DissonanceModel`] trait and a family of sensory
//! dissonance estimators -- Plomp-Levelt, Sethares, Helmholtz beating, and
//! harmonic entropy -- plus a [`DissonanceRegistry`] for looking them up by
//! name and a runtime surface that installs them as a SIM lib.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;
mod runtime;

pub use model::*;
pub use runtime::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
