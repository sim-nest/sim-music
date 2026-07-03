//! Core music model layer for the SIM music constellation.
//!
//! SIM is a Rust runtime with multiple codec surfaces; this crate supplies the
//! concrete music domain that those surfaces operate on. It defines the music
//! object model (notes, chords, melodies, scores), the descriptor metadata that
//! describes music components and their ports and parameters, events and lanes,
//! the piano roll and time grid, players and playables, performances and takes,
//! the arranger, freeze surfaces, traces, the component registry, and the
//! citizen integration that registers these types with the runtime.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod arranger;
mod arranger_expr;
mod arranger_music_expr;
mod arranger_render;
mod browse;
mod citizen;
mod descriptor;
mod descriptor_defaults;
mod descriptor_modulators;
mod descriptor_step_players;
mod event;
mod freeze;
mod lane;
mod model;
mod objects;
mod performance;
mod piano_roll;
mod playable;
mod player;
mod player_chain;
mod registry;
mod time;
mod trace;

pub use arranger::*;
pub use browse::*;
pub use citizen::*;
pub use descriptor::*;
pub use descriptor_defaults::*;
pub use descriptor_modulators::*;
pub use descriptor_step_players::*;
pub use event::*;
pub use freeze::*;
pub use lane::*;
pub use model::*;
pub use performance::*;
pub use piano_roll::*;
pub use playable::*;
pub use player::*;
pub use player_chain::*;
pub use registry::*;
pub use time::*;
pub use trace::*;

#[cfg(test)]
mod arranger_tests;

#[cfg(test)]
mod player_tests;

#[cfg(test)]
mod performance_tests;

#[cfg(test)]
mod piano_roll_tests;

#[cfg(test)]
mod recipe_tests;

#[cfg(test)]
mod registry_tests;

#[cfg(test)]
mod tests;
