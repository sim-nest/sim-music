//! Jazz chord-symbol naming for the SIM music libraries.
//!
//! This crate implements the jazz naming school: it parses jazz chord symbols
//! such as `Cmaj7` or `Am7/G` into a [`JazzChordSymbol`] ([`parse_jazz_symbol`]),
//! realizes them as concrete chords, and recognizes a pitch-class set as a jazz
//! chord by brute-force matching every root and [`JazzQuality`]
//! ([`match_jazz_symbol`]).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;
mod parser;

pub use model::*;
pub use parser::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
