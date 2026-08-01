//! Inspectable serial-spine adaptation reports.

use std::collections::{BTreeMap, BTreeSet};

use sim_lib_music_core::{Pitch, Time};
use sim_lib_music_transform::MapWitness;
use sim_lib_pitch_dissonance::ContextualSonanceReport;
use sim_lib_pitch_scale::PlayerScale;

use crate::{OrdinalRef, RealizerId, SerialEventId};

/// Stable adaptation family used by a modal-spine realizer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SerialSpineKind {
    /// Degree labels derived from a landed modal scale.
    DegreeCycle,
    /// Direct landed modal pitch identity.
    NearestScaleTone,
    /// Degree labels plus explicit chromatic-inflection deltas.
    MarkedChromaticInflection,
    /// A non-pitch spine label carried alongside landed pitch adaptation.
    NonPitchSpine,
}

/// One reported spine label, independent from the sounding pitch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SerialSpineLabel {
    /// One-based modal scale degree.
    Degree(usize),
    /// Landed pitch identity.
    LandedPitch(Pitch),
    /// Degree plus chromatic displacement from the source pitch class.
    ChromaticInflection {
        /// Landed modal degree.
        degree: usize,
        /// Signed semitone shift between source and landed classes.
        semitone_delta: i16,
    },
    /// Non-pitch serial token that still follows the landed pitch map.
    OrdinalToken {
        /// Stable ordinal source.
        ordinal: OrdinalRef,
        /// Zero-based ordinal occurrence inside the event.
        note_index: usize,
    },
}

/// One mapped sounding note in a serial-spine adaptation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialSpineEntry {
    /// Stable source event identity.
    pub event_id: SerialEventId,
    /// Stable ordinal source for this sounding note.
    pub ordinal: OrdinalRef,
    /// Stable ordinal occurrence inside the event's note list.
    pub note_index: usize,
    /// Exact onset retained from the source realization.
    pub onset: Time,
    /// Source pitch before adaptation.
    pub source_pitch: Pitch,
    /// Landed pitch after adaptation.
    pub landed_pitch: Pitch,
    /// One-based modal degree, when the landed pitch belongs to the scale.
    pub modal_degree: Option<usize>,
    /// Whether the landed pitch is a scale member.
    pub modal_member: bool,
    /// Explicit pitch-map witness for the landing step.
    pub witness: MapWitness,
    /// Independent spine label for this note.
    pub label: SerialSpineLabel,
}

/// Collision where multiple source classes land on the same target class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialSpineCollision {
    /// Landed pitch class value.
    pub landed_class: u8,
    /// Distinct source pitch classes that collided.
    pub source_classes: Vec<u8>,
}

/// Repeated modal degree observed in canonical note order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialRepeatedDegree {
    /// Degree that repeated.
    pub degree: usize,
    /// Events that carried the repetition.
    pub events: Vec<SerialEventId>,
}

/// Aggregate comparison between source and landed pitch-class collections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChromaticAggregateIdentity {
    /// Source pitch classes heard before adaptation.
    pub source_classes: Vec<u8>,
    /// Landed pitch classes heard after adaptation.
    pub landed_classes: Vec<u8>,
    /// Source classes missing after landing.
    pub lost_source_classes: Vec<u8>,
    /// Whether the landed result preserved the chromatic aggregate exactly.
    pub preserved: bool,
}

/// Sonance report for one adjacent adapted window.
#[derive(Clone, Debug, PartialEq)]
pub struct SerialSonanceContext {
    /// Stable prior event.
    pub from_event: SerialEventId,
    /// Stable next event.
    pub to_event: SerialEventId,
    /// Contextual sonance comparison over the landed pitches.
    pub report: ContextualSonanceReport,
}

/// Complete inspectable serial-spine report attached to one realization.
#[derive(Clone, Debug, PartialEq)]
pub struct SerialSpineReport {
    /// Realizer that produced the report.
    pub realizer_id: RealizerId,
    /// Adaptation family used by the realizer.
    pub kind: SerialSpineKind,
    /// Effective modal or custom performance scale.
    pub scale: PlayerScale,
    /// One note-level entry in canonical note order.
    pub entries: Vec<SerialSpineEntry>,
    /// Collisions recorded while landing the chromatic source.
    pub collisions: Vec<SerialSpineCollision>,
    /// Repeated modal degrees in canonical note order.
    pub repeated_degrees: Vec<SerialRepeatedDegree>,
    /// Notes that remained outside the scale after landing.
    pub out_of_mode: Vec<SerialEventId>,
    /// Notes whose pitch changed under landing.
    pub pitch_changes: Vec<SerialEventId>,
    /// Aggregate identity report for source versus landed pitch classes.
    pub aggregate_identity: ChromaticAggregateIdentity,
    /// Canonical ordinal order retained by the landed adaptation.
    pub ordinal_order: Vec<OrdinalRef>,
    /// Adjacent contextual sonance comparisons over landed windows.
    pub sonance_context: Vec<SerialSonanceContext>,
}

impl SerialSpineReport {
    /// Returns the modal-membership view independently of other report facets.
    pub fn modal_membership(&self) -> Vec<(SerialEventId, bool, Option<usize>)> {
        self.entries
            .iter()
            .map(|entry| {
                (
                    entry.event_id.clone(),
                    entry.modal_member,
                    entry.modal_degree,
                )
            })
            .collect()
    }

    /// Returns the pitch-identity view independently of other report facets.
    pub fn pitch_identity(&self) -> Vec<(SerialEventId, Pitch, Pitch, MapWitness)> {
        self.entries
            .iter()
            .map(|entry| {
                (
                    entry.event_id.clone(),
                    entry.source_pitch,
                    entry.landed_pitch,
                    entry.witness.clone(),
                )
            })
            .collect()
    }

    /// Returns the aggregate identity report independently of other report facets.
    pub fn chromatic_aggregate_identity(&self) -> &ChromaticAggregateIdentity {
        &self.aggregate_identity
    }

    /// Returns the retained ordinal order independently of other report facets.
    pub fn ordinal_order(&self) -> &[OrdinalRef] {
        &self.ordinal_order
    }

    /// Returns the contextual sonance view independently of other report facets.
    pub fn sonance_context(&self) -> &[SerialSonanceContext] {
        &self.sonance_context
    }
}

pub(crate) fn collect_collisions(entries: &[SerialSpineEntry]) -> Vec<SerialSpineCollision> {
    let mut by_target = BTreeMap::<u8, BTreeSet<u8>>::new();
    for entry in entries {
        by_target
            .entry(entry.landed_pitch.class.value())
            .or_default()
            .insert(entry.source_pitch.class.value());
    }
    by_target
        .into_iter()
        .filter_map(|(landed_class, source_classes)| {
            (source_classes.len() > 1).then(|| SerialSpineCollision {
                landed_class,
                source_classes: source_classes.into_iter().collect(),
            })
        })
        .collect()
}

pub(crate) fn collect_repeated_degrees(entries: &[SerialSpineEntry]) -> Vec<SerialRepeatedDegree> {
    let mut out = Vec::new();
    let mut current_degree = None;
    let mut current_events = Vec::<SerialEventId>::new();
    for entry in entries {
        if let Some(degree) = entry.modal_degree {
            if current_degree == Some(degree) {
                current_events.push(entry.event_id.clone());
            } else {
                if let Some(previous_degree) = current_degree.take()
                    && current_events.len() > 1
                {
                    out.push(SerialRepeatedDegree {
                        degree: previous_degree,
                        events: current_events.clone(),
                    });
                }
                current_degree = Some(degree);
                current_events = vec![entry.event_id.clone()];
            }
        }
    }
    if let Some(degree) = current_degree
        && current_events.len() > 1
    {
        out.push(SerialRepeatedDegree {
            degree,
            events: current_events,
        });
    }
    out
}

pub(crate) fn aggregate_identity(entries: &[SerialSpineEntry]) -> ChromaticAggregateIdentity {
    let source = entries
        .iter()
        .map(|entry| entry.source_pitch.class.value())
        .collect::<BTreeSet<_>>();
    let landed = entries
        .iter()
        .map(|entry| entry.landed_pitch.class.value())
        .collect::<BTreeSet<_>>();
    let lost_source_classes = source.difference(&landed).copied().collect::<Vec<_>>();
    ChromaticAggregateIdentity {
        source_classes: source.iter().copied().collect(),
        landed_classes: landed.iter().copied().collect(),
        lost_source_classes: lost_source_classes.clone(),
        preserved: lost_source_classes.is_empty() && source == landed,
    }
}
