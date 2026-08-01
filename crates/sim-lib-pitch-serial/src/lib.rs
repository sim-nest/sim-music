//! Strict twelve-tone rows, total operations, and explicit label conventions.
//!
//! This crate is the pitch-specific layer over [`sim_lib_serial_core`]. It uses
//! the canonical [`sim_lib_pitch_core::PitchClass`] values, admits each of the
//! twelve classes exactly once, and keeps affine/reversal operation identity
//! separate from convention-dependent printed labels.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod alphabet;
mod error;
mod label;
mod operation;
mod row;

pub use alphabet::PitchClassAlphabet;
pub use error::RowError;
pub use label::{RowLabel, RowLabelConvention};
pub use operation::{RowFamily, RowOperation};
pub use row::{RowForm, ToneRow};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));
