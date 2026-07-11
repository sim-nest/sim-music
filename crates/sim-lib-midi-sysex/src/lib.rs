//! Typed views over MIDI system-exclusive (SysEx) messages.
//!
//! This crate interprets the opaque byte payload of a
//! [`SysExEvent`](sim_lib_midi_core::SysExEvent) as structured messages and
//! serialises them back. It covers Universal SysEx ([`UniversalSysEx`]) and the
//! MIDI Tuning Standard ([`MtsMessage`]), plus Yamaha manufacturer SysEx
//! ([`YamahaSysEx`]) and the DX7 voice/bank patch formats ([`Dx7Bulk`],
//! [`Dx7Voice`], [`Dx7VoiceBank`], [`Dx7Operator`], [`Dx7VoiceCommon`]),
//! including packed/unpacked voice conversion and Yamaha checksums. All data
//! bytes are validated as 7-bit; failures surface as [`SysExViewError`].
//!
//! # Examples
//!
//! Round-tripping a Universal SysEx message through its `F0` payload:
//!
//! ```
//! use sim_lib_midi_sysex::{UniversalRealm, UniversalSysEx};
//!
//! let message = UniversalSysEx::new(
//!     UniversalRealm::NonRealTime,
//!     0x7f,
//!     0x08,
//!     0x01,
//!     vec![0x00, 0x12],
//! )
//! .unwrap();
//! let payload = message.to_f0_payload().unwrap();
//! assert_eq!(UniversalSysEx::from_f0_payload(&payload).unwrap(), message);
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod dx7;
mod model;

pub use dx7::*;
pub use model::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
