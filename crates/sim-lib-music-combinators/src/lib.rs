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
//! ergonomic assembly of music objects.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod arp_lab;
mod arpeggio;
mod bassline;
mod beat_map;
mod builders;
mod drum;
mod euclid;
mod player;
mod polystep;
mod quad_note;

pub use arp_lab::*;
pub use arpeggio::*;
pub use bassline::*;
pub use beat_map::*;
pub use builders::*;
pub use drum::*;
pub use euclid::*;
pub use player::*;
pub use polystep::*;
pub use quad_note::*;
pub use sim_lib_music_core::{
    Articulation, Chord, Counterpoint, Melody, MelodyItem, MidiFileObj, MidiTrackObj, Music,
    MusicError, MusicObject, Note, Par, PianoRoll, Progression, Rest, Score, Seq, Time, TimedNote,
};

#[cfg(test)]
mod recipe_tests;

#[cfg(test)]
mod tests;
