//! Standard MIDI File (SMF) reading and writing for the SIM music stack.
//!
//! This crate parses the on-disk `.mid`/`.smf` byte format into the in-memory
//! [`SmfFile`] model and serialises it back, reusing the event types from
//! [`sim_lib_midi_core`]. It covers the three SMF formats ([`SmfFormat`]), the
//! variable-length quantity encoding ([`encode_vlq`]/[`decode_vlq`]), running
//! status, and track canonicalisation/merging. Reading is [`read_smf`];
//! writing is [`write_smf`] (or [`write_smf_with_options`] for running-status
//! control). Only metric (ticks-per-quarter) division is supported; SMPTE
//! division is rejected.
//!
//! # Examples
//!
//! Round-tripping a minimal single-track file:
//!
//! ```
//! use sim_lib_midi_smf::{read_smf, write_smf, SmfFile, SmfFormat, SmfTrack};
//! use sim_lib_midi_core::{
//!     MetaEvent, MidiEvent, MidiPayload, TickTime, synthetic_origin,
//! };
//!
//! let file = SmfFile {
//!     format: SmfFormat::SingleTrack,
//!     tpq: 480,
//!     tracks: vec![SmfTrack {
//!         events: vec![MidiEvent {
//!             time: TickTime::new(0, 480).unwrap(),
//!             origin: synthetic_origin(),
//!             payload: MidiPayload::Meta(MetaEvent::EndOfTrack),
//!         }],
//!     }],
//! };
//! let bytes = write_smf(&file).unwrap();
//! let parsed = read_smf(&bytes).unwrap();
//! assert_eq!(parsed.format, SmfFormat::SingleTrack);
//! assert_eq!(parsed.tpq, 480);
//! ```
//!
//! The variable-length quantity codec is reversible:
//!
//! ```
//! use std::io::Cursor;
//! use sim_lib_midi_smf::{decode_vlq, encode_vlq};
//!
//! let bytes = encode_vlq(0x4000);
//! assert_eq!(decode_vlq(&mut Cursor::new(bytes)).unwrap(), 0x4000);
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod model;
mod reader;
mod vlq;
mod writer;

pub use error::*;
pub use model::*;
pub use reader::*;
pub use vlq::*;
pub use writer::*;

#[cfg(test)]
mod recipe_tests;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
