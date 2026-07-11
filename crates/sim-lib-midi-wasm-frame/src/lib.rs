//! MIDI binary-frame facade descriptors.
//!
//! Despite the `-wasm` suffix, this crate does not define WebAssembly module
//! entrypoints or wasm-bindgen glue. It provides data descriptors for
//! frame-safe MIDI surfaces that can be serialized and handed to web or wasm ABI
//! adapters. The modeled (descriptor-only) tier is flagged by the default-on
//! `model` feature.
//!
//! # Examples
//!
//! A MIDI event frame round-trips through its byte encoding:
//!
//! ```
//! use sim_lib_midi_wasm_frame::{
//!     decode_frame_array, encode_frame_array, MidiEventFrame, MidiFrameKind,
//! };
//!
//! let frame = MidiEventFrame {
//!     ticks: 0,
//!     tpq: 480,
//!     kind: MidiFrameKind::Channel,
//!     status: 0x90,
//!     data: vec![60, 100],
//! };
//! let bytes = encode_frame_array(&[frame.clone()]).unwrap();
//! assert_eq!(decode_frame_array(&bytes).unwrap(), vec![frame]);
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;

pub use model::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
