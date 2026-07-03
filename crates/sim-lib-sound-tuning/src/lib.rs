//! Tuning systems and temperaments for the SIM music constellation.
//!
//! This crate provides the [`Tuning`] trait and a family of concrete tuning
//! systems -- equal temperament, just intonation, Pythagorean, quarter-comma
//! meantone, Werckmeister III, Young, and Scala cents tables -- that map
//! pitches to and from frequencies. [`TuningDescriptor`] is a serializable
//! description that builds a boxed `Tuning`, and the runtime surface installs
//! the built-in tuning cards as a SIM lib.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;
mod runtime;
mod surface;

pub use model::*;
pub use runtime::*;
pub use surface::*;

#[cfg(test)]
mod tests;
