use std::collections::BTreeMap;

use sim_lib_music_core::{
    Articulation, Channel, ConversionLoss, ConversionLossKind, Music, MusicConversion, MusicObject,
    Note, Par, PianoRoll, Pitch, Rest, Time, TimeGrid, TimedNote,
};
use sim_lib_music_transform::TransformChain;

use crate::{CarpetError, CarpetIndex, CarpetPolicy, MusicCarpet};

/// Scope at which an absolute origin is established for relative events.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RelativeScope {
    /// Retain and reset an origin in every occupied cell.
    Cell,
    /// Retain one origin for the complete stable rank order of the carpet.
    Carpet,
    /// Omit the carpet origin and require it from the decoding context.
    External,
}

/// Reference used for each pitch or time delta after a scope origin exists.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RelativeReference {
    /// Measure every delta from the scope's first event.
    Anchor,
    /// Measure every delta from the preceding event.
    Previous,
}

/// Policy controlling an absolute-to-relative conversion.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RelativePolicy {
    /// Origin lifetime and retention.
    pub scope: RelativeScope,
    /// Reference used by pitch intervals.
    pub pitch: RelativeReference,
    /// Reference used by onset intervals.
    pub time: RelativeReference,
}

impl RelativePolicy {
    /// Lossless per-cell delta encoding relative to each preceding event.
    pub const CELL_DELTAS: Self = Self {
        scope: RelativeScope::Cell,
        pitch: RelativeReference::Previous,
        time: RelativeReference::Previous,
    };

    /// Lossless carpet-wide encoding relative to one retained origin.
    pub const CARPET_ANCHOR: Self = Self {
        scope: RelativeScope::Carpet,
        pitch: RelativeReference::Anchor,
        time: RelativeReference::Anchor,
    };
}

/// Absolute pitch/time value that grounds a relative event sequence.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RelativeOrigin {
    /// Absolute pitch of the first event in scope.
    pub pitch: Pitch,
    /// Absolute onset of the first event in scope.
    pub onset: Time,
}

impl RelativeOrigin {
    /// Builds an explicit decoding origin.
    pub fn new(pitch: Pitch, onset: Time) -> Self {
        Self { pitch, onset }
    }
}

/// One exact note represented by relative pitch and onset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelativeEvent {
    /// Signed semitone distance from the selected pitch reference.
    pub pitch_delta: i32,
    /// Exact rational time distance from the selected onset reference.
    pub onset_delta: Time,
    /// Exact sounding duration.
    pub duration: Time,
    /// MIDI velocity.
    pub velocity: u8,
    /// MIDI channel.
    pub channel: Channel,
    /// Performance articulation.
    pub articulation: Articulation,
}

/// Relative event data for one carpet cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelativeCell {
    /// Retained scope origin when this cell starts a cell or carpet scope.
    pub origin: Option<RelativeOrigin>,
    /// Canonically ordered relative note events.
    pub events: Vec<RelativeEvent>,
    /// Exact semantic duration of the source music object.
    pub extent: Time,
    /// Piano-roll grid retained for canonical piano-roll sources.
    pub time_grid: TimeGrid,
}

/// Relative pitch/time form of a [`MusicCarpet`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelativeCarpet {
    /// Axis data copied without alteration.
    pub axes: Vec<crate::CarpetAxis>,
    /// Relative cell data in stable coordinate order.
    pub cells: BTreeMap<CarpetIndex, RelativeCell>,
    /// Original carpet shape policy.
    pub shape_policy: CarpetPolicy,
    /// Relative reference policy.
    pub relative_policy: RelativePolicy,
}

#[derive(Copy, Clone)]
struct RelativeState {
    origin: RelativeOrigin,
    previous: RelativeOrigin,
}

/// Encodes every occupied cell into exact relative pitch/time events.
///
/// Cells are traversed in the same lexicographic order as the shared
/// mixed-radix discrete rank adapter. The existing [`TransformChain`] canonical
/// projection supplies event order. Canonical note-only piano rolls round-trip
/// byte-for-value; other [`Music`] structures receive
/// [`ConversionLossKind::SourceStructure`] because the relative form preserves
/// sounding semantics and exact extent, not tree syntax or non-note lanes.
pub fn encode_relative(
    carpet: &MusicCarpet,
    policy: RelativePolicy,
) -> Result<MusicConversion<RelativeCarpet>, CarpetError> {
    let canonicalizer = TransformChain::default();
    let mut cells = BTreeMap::new();
    let mut losses = Vec::new();
    let mut carpet_state = None;
    let mut has_events = false;

    for (index, music) in &carpet.cells {
        let canonical = canonicalizer
            .apply(music)
            .map_err(|error| CarpetError::Transform {
                index: index.clone(),
                detail: error.to_string(),
            })?;
        let Music::PianoRoll(roll) = canonical else {
            unreachable!("an empty transform chain canonicalizes to PianoRoll");
        };
        if !matches!(music, Music::PianoRoll(source) if source == &roll) {
            losses.push(ConversionLoss::new(
                ConversionLossKind::SourceStructure,
                None,
                format!(
                    "cell {:?} preserves canonical notes and extent but not {} structure",
                    index.coordinates,
                    music.kind()
                ),
            ));
        }

        let mut state = match policy.scope {
            RelativeScope::Cell => None,
            RelativeScope::Carpet | RelativeScope::External => carpet_state,
        };
        let mut retained_origin = None;
        let mut events = Vec::with_capacity(roll.items.len());
        for timed in &roll.items {
            has_events = true;
            let absolute = RelativeOrigin::new(timed.note.pitch, timed.onset);
            let current = match state {
                Some(current) => current,
                None => {
                    if policy.scope != RelativeScope::External {
                        retained_origin = Some(absolute);
                    }
                    RelativeState {
                        origin: absolute,
                        previous: absolute,
                    }
                }
            };
            let pitch_base = reference_value(policy.pitch, current).pitch.semitone();
            let time_base = reference_value(policy.time, current).onset;
            events.push(RelativeEvent {
                pitch_delta: absolute.pitch.semitone() - pitch_base,
                onset_delta: absolute.onset - time_base,
                duration: timed.note.duration,
                velocity: timed.note.velocity,
                channel: timed.note.channel,
                articulation: timed.note.articulation,
            });
            state = Some(RelativeState {
                origin: current.origin,
                previous: absolute,
            });
        }
        if policy.scope != RelativeScope::Cell {
            carpet_state = state;
        }
        cells.insert(
            index.clone(),
            RelativeCell {
                origin: retained_origin,
                events,
                extent: music.duration(),
                time_grid: roll.time,
            },
        );
    }
    if policy.scope == RelativeScope::External && has_events {
        losses.push(ConversionLoss::new(
            ConversionLossKind::RelativeAnchor,
            None,
            "external relative policy omits the carpet's absolute pitch/time origin",
        ));
    }
    Ok(MusicConversion {
        value: RelativeCarpet {
            axes: carpet.axes.clone(),
            cells,
            shape_policy: carpet.policy,
            relative_policy: policy,
        },
        preserved: Vec::new(),
        losses,
    })
}

/// Decodes relative events into canonical exact music cells.
///
/// `external_origin` is used only for [`RelativeScope::External`]. When it is
/// absent, decoding remains deterministic by using pitch C-1 and time zero, and
/// the returned conversion records [`ConversionLossKind::RelativeAnchor`].
pub fn decode_relative(
    relative: &RelativeCarpet,
    external_origin: Option<RelativeOrigin>,
) -> Result<MusicConversion<MusicCarpet>, CarpetError> {
    let policy = relative.relative_policy;
    let mut losses = Vec::new();
    let mut cells = BTreeMap::new();
    let mut carpet_state = match policy.scope {
        RelativeScope::External => Some(RelativeState::from_origin(
            external_origin.unwrap_or_else(zero_origin),
        )),
        RelativeScope::Cell | RelativeScope::Carpet => None,
    };
    if policy.scope == RelativeScope::External
        && external_origin.is_none()
        && relative.cells.values().any(|cell| !cell.events.is_empty())
    {
        losses.push(ConversionLoss::new(
            ConversionLossKind::RelativeAnchor,
            None,
            "decoded omitted external origin at pitch C-1 and time zero",
        ));
    }

    for (index, cell) in &relative.cells {
        let mut state = match policy.scope {
            RelativeScope::Cell => None,
            RelativeScope::Carpet | RelativeScope::External => carpet_state,
        };
        let mut timed = Vec::with_capacity(cell.events.len());
        for event in &cell.events {
            let current = match state {
                Some(current) => current,
                None => {
                    let origin = cell.origin.unwrap_or_else(|| {
                        losses.push(ConversionLoss::new(
                            ConversionLossKind::RelativeAnchor,
                            None,
                            format!(
                                "cell {:?} omitted a required retained origin; decoded at zero",
                                index.coordinates
                            ),
                        ));
                        zero_origin()
                    });
                    RelativeState::from_origin(origin)
                }
            };
            let pitch_base = reference_value(policy.pitch, current).pitch.semitone();
            let onset_base = reference_value(policy.time, current).onset;
            let semitone = pitch_base.checked_add(event.pitch_delta).ok_or_else(|| {
                relative_error(
                    index,
                    "relative pitch overflowed the absolute semitone range",
                )
            })?;
            let pitch = Pitch::from_semitone(semitone);
            if pitch.semitone() != semitone {
                return Err(relative_error(
                    index,
                    "relative pitch exceeded the canonical Pitch octave range",
                ));
            }
            let absolute = RelativeOrigin::new(pitch, onset_base + event.onset_delta);
            timed.push(TimedNote {
                onset: absolute.onset,
                note: Note::new(
                    event.duration,
                    absolute.pitch,
                    event.velocity,
                    event.channel,
                    event.articulation,
                )
                .map_err(|error| relative_error(index, error.to_string()))?,
            });
            state = Some(RelativeState {
                origin: current.origin,
                previous: absolute,
            });
        }
        if policy.scope != RelativeScope::Cell {
            carpet_state = state;
        }
        cells.insert(index.clone(), decoded_music(index, cell, timed)?);
    }

    Ok(MusicConversion {
        value: MusicCarpet::new(relative.axes.clone(), cells, relative.shape_policy)?,
        preserved: Vec::new(),
        losses,
    })
}

impl RelativeState {
    fn from_origin(origin: RelativeOrigin) -> Self {
        Self {
            origin,
            previous: origin,
        }
    }
}

fn reference_value(reference: RelativeReference, state: RelativeState) -> RelativeOrigin {
    match reference {
        RelativeReference::Anchor => state.origin,
        RelativeReference::Previous => state.previous,
    }
}

fn zero_origin() -> RelativeOrigin {
    RelativeOrigin::new(Pitch::from_semitone(0), Time::from_integer(0))
}

fn decoded_music(
    index: &CarpetIndex,
    cell: &RelativeCell,
    timed: Vec<TimedNote>,
) -> Result<Music, CarpetError> {
    let mut roll =
        PianoRoll::new(timed).map_err(|error| relative_error(index, error.to_string()))?;
    roll.time = cell.time_grid.clone();
    let sounding_extent = roll.duration();
    if cell.extent < sounding_extent {
        return Err(relative_error(
            index,
            "stored cell extent ends before its sounding notes",
        ));
    }
    if cell.extent == sounding_extent {
        return Ok(Music::PianoRoll(roll));
    }
    let rest = Rest::new(cell.extent).map_err(|error| relative_error(index, error.to_string()))?;
    Ok(Music::Par(Par {
        children: vec![Box::new(roll), Box::new(rest)],
    }))
}

fn relative_error(index: &CarpetIndex, detail: impl Into<String>) -> CarpetError {
    CarpetError::Relative {
        index: index.clone(),
        detail: detail.into(),
    }
}
