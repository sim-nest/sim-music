#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Score and notation model for the SIM music libraries.
//!
//! This crate is the notation codec surface: it converts between a
//! `sim_lib_music_core::Score` (and related music objects such as melodies,
//! progressions, and counterpoint) and bounded LilyPond or MusicXML notation.
//! MusicXML remains a fail-closed profile with explicit resource limits and
//! identity/loss sidecars; it is not a second score model or a general XML
//! codec. [`NotationCodec`] is the Rust entry point, while
//! [`install_music_notation_lib`] registers the Shape-described
//! `music/notation/import` runtime callable.
#![allow(deprecated)]

mod export;
mod import;
mod model;
mod musicxml;
mod musicxml_export;
mod musicxml_import;
mod musicxml_note;
mod musicxml_support;
mod runtime;
mod spell;

pub use export::{
    export_counterpoint_lilypond, export_lilypond, export_lilypond_report, export_melody_lilypond,
    export_progression_lilypond,
};
pub use import::{import_lilypond, import_lilypond_report};
pub use model::{
    MusicXmlLimits, NotationCodec, NotationError, NotationIdentity, NotationIdentityKind,
    NotationLoss, NotationLossKind, NotationReport,
};
pub use musicxml::{
    export_musicxml_partwise, export_musicxml_partwise_report, import_musicxml_partwise,
    import_musicxml_partwise_report,
};
pub use runtime::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
