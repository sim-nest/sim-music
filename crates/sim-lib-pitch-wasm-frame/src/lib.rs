//! Pitch binary-frame facade descriptors.
//!
//! Despite the `-wasm` suffix, this crate does not define WebAssembly module
//! entrypoints or wasm-bindgen glue. It provides data descriptors for
//! frame-safe pitch surfaces that can be serialized and handed to web or wasm
//! ABI adapters. The modeled (descriptor-only) tier is flagged by the
//! default-on `model` feature.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;

pub use model::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
