//! Core MIDI data model and in-memory I/O for the SIM music stack.
//!
//! This crate defines the protocol-agnostic MIDI types shared across the
//! constellation: tick-based timing ([`TickTime`]), the bounded integer
//! domains used by MIDI bytes ([`U7`], [`U14`], [`Channel`]), the event model
//! ([`MidiEvent`], [`MidiPayload`], [`ChannelMessage`], [`MetaEvent`],
//! [`SysExEvent`]), and the streaming [`MidiSource`]/[`MidiSink`] traits with
//! in-memory implementations. It also provides the [`NoteEchoPlayer`]
//! transform, controller-number constants, tempo conversions, and the
//! host-registered [`MidiIoLib`] that exposes the in-memory cards to a running
//! SIM [`Cx`](sim_kernel::Cx).
//!
//! Higher layers (Standard MIDI File, SysEx, live transports) build on this
//! model rather than redefining it.
//!
//! # Examples
//!
//! ```
//! use sim_lib_midi_core::{Channel, ChannelMessage, U7};
//!
//! let note = ChannelMessage::NoteOn {
//!     ch: Channel::new(0).unwrap(),
//!     key: U7(60),
//!     vel: U7(100),
//! };
//! assert!(matches!(note, ChannelMessage::NoteOn { .. }));
//! ```
//!
//! ```
//! use sim_lib_midi_core::TickTime;
//!
//! // 480 ticks at 480 tpq is exactly one quarter note.
//! let one_quarter = TickTime::new(480, 480).unwrap();
//! assert_eq!(one_quarter.as_f64_quarters(), 1.0);
//! // Rebasing to a coarser resolution is exact here.
//! assert_eq!(one_quarter.rebase(96).unwrap().ticks, 96);
//! ```

#![forbid(unsafe_code)]
#![allow(deprecated)]
#![deny(missing_docs)]

mod cc;
mod error;
mod io;
mod model;
mod player;
mod runtime;
mod tempo;

pub mod meta_view;
pub mod wire;

pub use cc::*;
pub use error::*;
pub use io::*;
pub use model::*;
pub use player::*;
pub use runtime::*;
pub use tempo::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
