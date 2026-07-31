//! Bounded MusicXML `score-partwise` exchange profile.
//!
//! This is deliberately a profile on the notation surface, not a general XML
//! codec. Parsing is delegated to `roxmltree` with DTDs disabled and a node
//! ceiling; music-specific modules add depth, text, part, and event ceilings
//! plus a fail-closed element/attribute vocabulary.

pub use crate::musicxml_export::{export_musicxml_partwise, export_musicxml_partwise_report};
pub use crate::musicxml_import::{import_musicxml_partwise, import_musicxml_partwise_report};
