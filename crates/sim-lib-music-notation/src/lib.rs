#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Score and notation model for the SIM music libraries.
//!
//! This crate is the notation codec surface: it converts between a
//! `sim_lib_music_core::Score` (and related music objects such as melodies,
//! progressions, and counterpoint) and a LilyPond-subset text rendering. The
//! [`NotationCodec`] type is the codec entry point, exposing import and export
//! in both plain and report (diagnostic-carrying) forms, and
//! [`install_music_notation_lib`] registers the codec as a loadable runtime lib.
#![allow(deprecated)]

mod export;
mod import;
mod model;
mod runtime;
mod spell;

pub use export::{
    export_counterpoint_lilypond, export_lilypond, export_lilypond_report, export_melody_lilypond,
    export_progression_lilypond,
};
pub use import::{import_lilypond, import_lilypond_report};
pub use model::{NotationCodec, NotationError, NotationReport};
pub use runtime::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
