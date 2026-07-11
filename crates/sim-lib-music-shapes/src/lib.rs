//! Shape protocol surfaces for SIM music types.
//!
//! This crate applies the SIM `Shape` match/binding/dispatch protocol to the
//! music representations defined in `sim-lib-music-core`. It provides three
//! things: read-construct citizen descriptors that round-trip music objects
//! through canonical text forms ([`MusicNoteDescriptor`] and siblings), the
//! `#(...)` codec that encodes and decodes every music representation, and a
//! loadable [`MusicShapesLib`] that registers documented `Shape` values for the
//! `music` namespace.
//!
//! The codec is the canonical text bridge: `encode_*` functions render a music
//! value to its `#(...)` form, and `decode_*` functions parse that form back
//! into the corresponding `sim-lib-music-core` type.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(deprecated)]

mod citizen;
mod codec;
mod runtime;

pub use citizen::{
    MusicChordDescriptor, MusicMelodyDescriptor, MusicNoteDescriptor, MusicParDescriptor,
    MusicScoreDescriptor, MusicSeqDescriptor, music_chord_class_symbol, music_melody_class_symbol,
    music_note_class_symbol, music_par_class_symbol, music_score_class_symbol,
    music_seq_class_symbol,
};
pub use codec::*;
pub use runtime::{MusicShapesLib, install_music_shapes_lib};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
