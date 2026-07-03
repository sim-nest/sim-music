//! Citizen descriptors, text codecs, and runtime shapes for SIM pitch types.
//!
//! This crate is the codec and runtime-integration surface for the pitch theory
//! libraries. It provides string round-trips for pitches, intervals, pitch-class
//! masks, scales, keys, chords, and chord symbols ([`encode_pitch`],
//! [`decode_pitch`], and siblings), wraps each canonical form in a citizen
//! descriptor ([`PitchDescriptor`] and friends) for read-construct evaluation, and
//! exposes the types as SIM `Shape`s through [`PitchShapesLib`], installable via
//! [`install_pitch_shapes_lib`].

#![forbid(unsafe_code)]
#![allow(deprecated)]
#![deny(missing_docs)]

mod citizen;
mod codec;
mod runtime;

pub use citizen::{
    PitchChordDescriptor, PitchClassMaskDescriptor, PitchDescriptor, PitchIntervalDescriptor,
    PitchScaleDescriptor, pitch_chord_class_symbol, pitch_class_mask_class_symbol,
    pitch_class_symbol, pitch_interval_class_symbol, pitch_scale_class_symbol,
};
pub use codec::*;
pub use runtime::{PitchShapesLib, install_pitch_shapes_lib};

#[cfg(test)]
mod tests;
