//! Sound-shape codec surface for the SIM music constellation.
//!
//! This crate gives the sound layer a text codec surface. It provides
//! `encode_*`/`decode_*` functions that round-trip the sound types (frequency,
//! amplitude, phase, partial, envelope, tone, spectrum, timbre, filters,
//! tuning descriptors, dissonance models, bridge/renderer/audio-lift options)
//! through their `#(...)` sound-shape text forms, citizen descriptors that wrap
//! those forms as first-class objects, and a runtime surface that installs the
//! shape definitions as a SIM lib.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod citizen;
mod codec;
mod parse;
mod parse_lift;
mod parse_surface;
mod runtime;

pub use citizen::{
    SoundEnvelopeDescriptor, SoundPartialDescriptor, SoundSpectrumDescriptor,
    SoundTimbreDescriptor, SoundToneDescriptor, SoundTuningDescriptor, sound_envelope_class_symbol,
    sound_partial_class_symbol, sound_spectrum_class_symbol, sound_timbre_class_symbol,
    sound_tone_class_symbol, sound_tuning_descriptor_class_symbol,
};
pub use codec::*;
pub use runtime::*;

#[cfg(test)]
mod tests;
