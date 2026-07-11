#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Headless DAW session surface for SIM audio graph workspaces.
//!
//! Sessions are portable data: an audio graph `Patch` plus tracks, buses,
//! clips, transport, plugin-chain state, and recording metadata. The crate is
//! hardware-free and can render deterministic offline buffers for tests,
//! previews, and agent inspection.
//!
//! ```rust
//! use sim_lib_daw_session::{
//!     DawClip, DawSession, DawTrack, daw_session_topology_package,
//!     render_session_offline,
//! };
//!
//! let mut session = DawSession::new("doc", "Doc Session", 48_000).unwrap();
//! let track = DawTrack::audio("lead", "Lead", 2)
//!     .unwrap()
//!     .with_clip(DawClip::constant("tone", 0, 4, 0.25).unwrap());
//! session.add_track(track).unwrap();
//!
//! let render = render_session_offline(&session, 4).unwrap();
//! assert_eq!(render.tracks_rendered(), 1);
//! assert_eq!(render.clips_rendered(), 1);
//!
//! let package = daw_session_topology_package(&session);
//! assert_eq!(package.tests.len(), 1);
//! ```

mod browse;
mod builder;
mod citizen;
mod codec;
mod expr_util;
mod integration;
mod integration_stream;
mod model;
mod prelude;
mod render;
mod runtime;
mod topology;

pub use browse::{browse_session_graph, session_card_expr, session_help_card_expr};
pub use builder::{
    COMPONENT_BUILDER_ACTIONS, COMPONENT_BUILDER_PATCH_FORMAT, component_builder_action_symbols,
};
pub use citizen::{DawSessionDescriptor, daw_session_class_symbol};
pub use integration::{
    DawInstrumentBinding, DawIntegratedPerformance, DawLiveSchedule, DawPatternAutomation,
    integrate_session_performance,
};
pub use integration_stream::DawPluginEventExport;
pub use model::{
    ClipSource, DawBus, DawClip, DawInstrumentInstance, DawInstrumentKind, DawSession,
    DawSessionRoute, DawSessionRouteKind, DawTrack, DawTrackKind, DawTransport, PluginChain,
    PluginSlot, RecordingMetadata, instrument_session_fixture, instrument_session_fixture_names,
    instrument_session_render_smoke_command,
};
pub use prelude::{daw_prelude_card_expr, daw_prelude_operations};
pub use render::{DawOfflineRender, render_session_offline};
pub use runtime::{DawSessionLib, daw_session_symbols, install_daw_session_lib};
pub use topology::daw_session_topology_package;

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod recipe_tests;
/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
