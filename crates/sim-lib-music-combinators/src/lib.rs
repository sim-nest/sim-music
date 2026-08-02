//! Composable music-transform combinators for the SIM music constellation.
//!
//! This crate layers reusable generative players and combinators on top of the
//! `sim-lib-music-core` material types. Each module renders musical input --
//! chords, scales, drum kits, step lanes -- into deterministic `PlayEvent`
//! streams with parallel trace data, so the same configuration always produces
//! the same output. The players cover arpeggiation ([`DualArpeggiator`],
//! [`ArpLab`]), basslines ([`BasslinePlayer`]), drum patterns
//! ([`BeatMapPlayer`], [`EuclideanPlayer`]), polyphonic step sequencing
//! ([`PolyStepPlayer`]), and multi-stream note generation
//! ([`QuadNotePlayer`]). The `builders` helpers wrap core constructors for
//! ergonomic assembly of music objects. [`DeclarativeHarmonyResolver`] composes
//! pitch-chord rules with exact-ratio and named-sonance registries, while
//! [`harmonize`] runs the shared recursive, factored, layered-DP, or beam planner
//! under explicit bounds and inspectable receipts. Finally,
//! [`render_harmony_progression`] adapts data-only harmony programs to canonical
//! progressions without hiding their export profile. [`MusicCarpet`] adds a
//! sparse, rank-addressed composition surface over those same exact music
//! objects, with algebraic layout transforms and audited relative pitch/time
//! forms. [`LSystem`] delegates bounded parallel rewrite exploration to the
//! shared discrete search engine, while [`ProgressionTreeCatalog`] delegates
//! stable finite-tree identities to the shared discrete rank adapter.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod arp_lab;
mod arpeggio;
mod bassline;
mod beat_map;
mod builders;
mod carpet;
mod carpet_algebra;
mod drum;
mod euclid;
mod harmony;
mod lsystem;
mod lsystem_engine;
mod player;
mod polystep;
mod progression_tree;
mod quad_note;
mod relative;
mod scale_rewrite;

pub use arp_lab::*;
pub use arpeggio::*;
pub use bassline::*;
pub use beat_map::*;
pub use builders::*;
pub use carpet::*;
pub use drum::*;
pub use euclid::*;
pub use harmony::*;
pub use lsystem::*;
pub use lsystem_engine::*;
pub use player::*;
pub use polystep::*;
pub use progression_tree::*;
pub use quad_note::*;
pub use relative::*;
pub use scale_rewrite::*;
pub use sim_lib_music_core::{
    Articulation, Chord, Counterpoint, Melody, MelodyItem, MidiFileObj, MidiTrackObj, Music,
    MusicError, MusicObject, Note, Par, PianoRoll, Progression, Rest, Score, Seq, Time, TimedNote,
};

#[cfg(test)]
mod recipe_tests;

#[cfg(test)]
mod carpet_conformance;

#[cfg(test)]
mod relative_conformance;

#[cfg(test)]
mod rewrite_conformance;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
