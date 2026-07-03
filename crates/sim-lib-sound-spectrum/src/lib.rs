//! Spectral analysis for the SIM music constellation.
//!
//! This crate defines the [`Spectrum`] type -- a frequency-domain magnitude
//! representation built either from a synthesized [`Tone`](sim_lib_sound_core::Tone)
//! or from PCM samples -- along with common spectral descriptors: peaks,
//! centroid, flatness, rolloff, and flux.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;

pub use model::*;

#[cfg(test)]
mod tests;
