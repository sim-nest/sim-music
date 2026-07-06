#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Playable pure-Rust software synthesizer primitives for the SIM audio graph.
//!
//! This crate implements audio-synthesis voices and the discrete components
//! they are built from: oscillators, filters, envelopes, LFOs, modulation
//! routing, control-voltage conventions, and polyphonic voice allocation. On
//! top of these primitives it models several classic instruments -- a Yamaha
//! DX7-style FM engine, a Korg PS-3300-style synthesizer, and the System 55 and
//! System 700 modular systems -- together with their patches, render fixtures,
//! and a component [`ComponentRegistry`] that exposes them to the runtime via
//! [`AudioSynthLib`]. Components plug into the shared SIM audio graph; the
//! `system55`, `system700`, `ps3300`, and `daw` modules are the public
//! instrument and arrangement surfaces.
//!
//! # Examples
//!
//! MIDI key 69 (A4) maps to 440 Hz under equal temperament:
//!
//! ```
//! use sim_lib_music_synth::midi_key_to_hz;
//!
//! assert!((midi_key_to_hz(69) - 440.0).abs() < 1e-3);
//! ```

mod algorithm;
mod backend;
mod builder;
mod citizen;
mod component;
mod cv;
mod dac_float;
/// DAW-style arrangement and transport surface built on the synth components.
pub mod daw;
mod dsp_fixed;
mod dx7;
mod dx7_envelope;
mod dx7_fixture;
mod dx7_inspection;
mod dx7_lfo;
mod dx7_operator;
mod dx7_patch;
mod dx7_pitch;
mod dx7_scaling;
mod dx7_velocity;
mod editor;
mod egs;
mod envelope;
mod fixture;
mod graph_host;
mod lfo;
mod lut;
mod modeled;
mod modulation;
mod modulator;
mod modules;
mod ops;
mod oscillator;
mod param;
mod patch;
mod poly;
mod port;
mod preset;
mod processor;
/// Korg PS-3300-style synthesizer model and its building blocks.
pub mod ps3300;
mod ps3300_fixture;
mod ps3300_patch;
mod ps3300_wrapper;
mod registry;
mod registry_ps3300;
mod registry_system55;
mod runtime;
/// Roland System 55-style modular synthesizer model and modules.
pub mod system55;
mod system55_fixture;
mod system55_patch;
mod system55_wrapper;
/// Roland System 700-style modular synthesizer model and modules.
pub mod system700;
mod system700_fixture;
mod system700_wrapper;
mod trace;
mod voice;

mod exports;
pub use exports::*;

#[cfg(test)]
mod tests;
