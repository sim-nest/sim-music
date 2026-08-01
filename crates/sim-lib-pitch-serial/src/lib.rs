//! Strict twelve-tone rows, complete families, matrices, and explicit labels.
//!
//! This crate is the pitch-specific layer over [`sim_lib_serial_core`]. It uses
//! the canonical [`sim_lib_pitch_core::PitchClass`] values, admits each of the
//! twelve classes exactly once, and keeps affine/reversal operation identity
//! separate from convention-dependent printed labels.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod all_interval;
mod alphabet;
mod class;
mod combinatoriality;
mod derivation;
mod error;
mod family;
mod interval;
mod invariance;
mod label;
mod matrix;
mod operation;
mod render;
mod row;
mod segment;

pub use all_interval::{AllIntervalMultiplicity, AllIntervalReport, analyze_all_interval};
pub use alphabet::PitchClassAlphabet;
pub use class::{FormEquivalence, RowClassAlias, RowClassReport, analyze_row_class};
pub use combinatoriality::{
    CombinatorialBlockEvidence, CombinatorialPartition, CombinatorialPartner,
    CombinatorialityReport, analyze_combinatoriality, analyze_combinatoriality_partition,
};
pub use derivation::{
    DerivationCellRelation, DerivationKind, DerivationMatch, DerivationReport, analyze_derivation,
    analyze_derivation_partition,
};
pub use error::RowError;
pub use family::{RowAlias, RowFamilySet};
pub use interval::OrderedIntervalString;
pub use invariance::{SegmentInvariant, analyze_invariance};
pub use label::{RowLabel, RowLabelConvention};
pub use matrix::{
    MatrixCoordinate, ROW_MATRIX_SIZE, RowMatrix, RowMatrixCell, RowMatrixEdgeLabels,
};
pub use operation::{RowFamily, RowOperation};
pub use render::RowMatrixData;
pub use row::{RowForm, ToneRow};
pub use segment::{RowSegment, RowSegmentSource};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));
