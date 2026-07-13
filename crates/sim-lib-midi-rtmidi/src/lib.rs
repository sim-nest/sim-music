#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! RtMidi MIDI adapter surface for SIM.
//!
//! The default `model` feature is hardware-independent and serves deterministic
//! fake RtMidi ports. The opt-in `rtmidi-hardware` feature exposes provider and
//! catalog seams for native port enumeration while keeping opened streams on the
//! same MIDI source/sink and stream-host contracts.
//!
//! # Examples
//!
//! Converting a backend microsecond timestamp into MIDI tick time:
//!
//! ```
//! use sim_lib_midi_rtmidi::RtmidiTiming;
//!
//! let timing = RtmidiTiming::new(960, 500_000).unwrap();
//! let tick = timing.timestamp_to_ticks(500_000);
//! assert_eq!(tick.ticks, 960);
//! assert_eq!(tick.tpq, 960);
//! ```

#[cfg(feature = "rtmidi-hardware")]
mod alsa_provider;
mod backend;
#[cfg(feature = "rtmidi-hardware")]
mod fixture;
mod io;
mod model;
#[cfg(feature = "rtmidi-hardware")]
mod native;
mod runtime;

/// RtMidi ALSA sequencer candidate name used by safe config probes.
pub const RTMIDI_ALSA_SEQ_MIDI_BACKEND_CANDIDATE: &str = "alsa-seq";
/// RtMidi CoreMIDI candidate name used by safe config probes.
pub const RTMIDI_COREMIDI_MIDI_BACKEND_CANDIDATE: &str = "coremidi";
/// RtMidi Windows multimedia candidate name used by safe config probes.
pub const RTMIDI_WINMM_MIDI_BACKEND_CANDIDATE: &str = "winmm";

/// Returns the RtMidi hardware MIDI backend candidate names.
pub fn rtmidi_midi_backend_candidates() -> [&'static str; 3] {
    [
        RTMIDI_ALSA_SEQ_MIDI_BACKEND_CANDIDATE,
        RTMIDI_COREMIDI_MIDI_BACKEND_CANDIDATE,
        RTMIDI_WINMM_MIDI_BACKEND_CANDIDATE,
    ]
}

#[cfg(feature = "rtmidi-hardware")]
pub use alsa_provider::{
    AlsaMidiDuplexEvalSite, AlsaMidiInputEvalSite, AlsaMidiOutputEvalSite, AlsaMidiProvider,
    CoreMidiDuplexEvalSite, CoreMidiInputEvalSite, CoreMidiOutputEvalSite, CoreMidiProvider,
    WinMmDuplexEvalSite, WinMmInputEvalSite, WinMmOutputEvalSite, WinMmProvider,
    alsa_seq_midi_backend_candidate,
};
pub use backend::{RtmidiBackend, rtmidi_backend_symbol, rtmidi_transport_symbol};
#[cfg(feature = "rtmidi-hardware")]
pub use fixture::FixtureRtmidiProvider;
pub use io::{RtmidiMidiSink, RtmidiMidiSource, bytes_from_payload, payload_from_bytes};
pub use model::{RtmidiEvent, RtmidiPort, RtmidiTiming};
#[cfg(feature = "rtmidi-hardware")]
pub use native::{
    NativeRtmidiProvider, RtmidiHardwareConfig, RtmidiInputDriver, RtmidiInputSource,
    RtmidiOutputDriver, RtmidiOutputSink, RtmidiProvider, input_ring,
};
pub use runtime::{MidiRtmidiLib, install_midi_rtmidi_lib, missing_rtmidi_dependency_card};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
