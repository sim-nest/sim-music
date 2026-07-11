//! Music binary-frame facade descriptors.
//!
//! Despite the `-wasm` suffix, this crate does not define WebAssembly module
//! entrypoints or wasm-bindgen glue. It provides frame-safe music descriptors
//! and stable wasm-engine entrypoint *names* (string identifiers) for browser
//! and ABI adapters to bind to; it ships no compiled wasm and runs no engine.
//! The modeled (descriptor-only) tier is flagged by the default-on `model`
//! feature.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;

pub use model::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
