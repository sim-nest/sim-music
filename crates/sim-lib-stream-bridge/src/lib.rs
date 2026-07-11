#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! MIDI/audio bridge helpers for STREAM 6.
//!
//! The bridge crate adapts finite stream packet spines between MIDI and PCM
//! using the existing sound and audio-lift libraries. It does not talk to host
//! audio or MIDI devices.

mod lift;
mod model;
mod render;
mod runtime;

pub use lift::{lift_pcm_items_to_midi, lift_pcm_stream_to_midi};
pub use model::{
    BridgeOutput, StreamBridgeLiftMidiOptions, StreamBridgeRenderOptions,
    stream_bridge_lift_midi_options_class_symbol, stream_bridge_render_options_class_symbol,
    stream_bridge_symbol,
};
pub use render::{render_midi_items_to_pcm, render_midi_stream_to_pcm};
pub use runtime::{StreamBridgeLib, install_stream_bridge_lib};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
