//! Core sound primitives for the SIM music constellation.
//!
//! This crate defines the foundational acoustic types shared by the
//! sound/synthesis layer: [`Frequency`], [`Amplitude`], and [`Phase`]
//! quantities; the [`Partial`], [`Envelope`], and [`Tone`] models that build a
//! spectral tone from sinusoidal components; and helpers for default envelopes
//! and equal-temperament pitch-to-frequency conversion.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;

pub use model::*;

#[cfg(test)]
mod tests;
