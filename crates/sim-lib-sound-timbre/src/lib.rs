//! Timbre models and spectral filters for the SIM music constellation.
//!
//! This crate defines the [`Timbre`] type -- a named synthesis recipe with a
//! default envelope, descriptive metadata, and a filter chain -- the
//! [`TimbreRecipe`] synthesis methods (pure sine, sawtooth, square, triangle,
//! organ pipe, Karplus-Strong, FM, and inharmonic bell), the spectral
//! [`Filter`] family, and a runtime surface that installs the built-in timbre
//! cards as a SIM lib.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod filter;
mod model;
mod runtime;

pub use filter::*;
pub use model::*;
pub use runtime::*;

#[cfg(test)]
mod tests;
