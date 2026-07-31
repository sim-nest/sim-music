use std::collections::{BTreeMap, BTreeSet};

use sim_lib_discrete_rank::BoundedIntVectorSpace;
use sim_lib_music_core::Music;
use sim_lib_music_transform::TransformDiagnostic;
use sim_lib_rank::Nat;
use thiserror::Error;

/// One finite coordinate axis in a [`MusicCarpet`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CarpetAxis {
    /// Stable axis name.
    pub name: String,
    /// Position labels in coordinate order.
    pub labels: Vec<String>,
    /// Whether explicit wrapping policies may wrap this axis.
    pub cyclic: bool,
}

impl CarpetAxis {
    /// Builds a finite axis. Empty axes are admitted or rejected by [`EmptyPolicy`].
    pub fn new(name: impl Into<String>, labels: Vec<String>, cyclic: bool) -> Self {
        Self {
            name: name.into(),
            labels,
            cyclic,
        }
    }

    /// Returns the number of positions on this axis.
    pub fn len(&self) -> usize {
        self.labels.len()
    }

    /// Returns whether this axis has no positions.
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }
}

/// A coordinate in a [`MusicCarpet`], ordered lexicographically by axis.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CarpetIndex {
    /// Zero-based coordinate for every carpet axis.
    pub coordinates: Vec<usize>,
}

impl CarpetIndex {
    /// Builds an index from zero-based coordinates.
    pub fn new(coordinates: impl Into<Vec<usize>>) -> Self {
        Self {
            coordinates: coordinates.into(),
        }
    }
}

/// Policy for an empty axis or an entirely empty carpet.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EmptyPolicy {
    /// Reject empty axes and carpets.
    Reject,
    /// Retain an explicitly empty data value.
    Allow,
}

/// Policy for missing cells inside the Cartesian product of all axes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RaggedPolicy {
    /// Require one cell at every coordinate.
    Reject,
    /// Preserve the supplied cells as a sparse carpet.
    Sparse,
}

/// Policy for a coordinate beyond an axis bound.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OutOfRangePolicy {
    /// Reject every out-of-range coordinate.
    Reject,
    /// Wrap only axes explicitly marked [`CarpetAxis::cyclic`].
    WrapCyclic,
}

/// Construction policy carried by a [`MusicCarpet`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CarpetPolicy {
    /// Empty-value policy.
    pub empty: EmptyPolicy,
    /// Missing-cell policy.
    pub ragged: RaggedPolicy,
    /// Out-of-range coordinate policy.
    pub out_of_range: OutOfRangePolicy,
}

impl CarpetPolicy {
    /// A non-empty, rectangular carpet with strict coordinates.
    pub const STRICT: Self = Self {
        empty: EmptyPolicy::Reject,
        ragged: RaggedPolicy::Reject,
        out_of_range: OutOfRangePolicy::Reject,
    };

    /// A non-empty sparse carpet with strict coordinates.
    pub const SPARSE: Self = Self {
        empty: EmptyPolicy::Reject,
        ragged: RaggedPolicy::Sparse,
        out_of_range: OutOfRangePolicy::Reject,
    };
}

/// Collision behavior for [`MusicCarpet::overlay`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OverlayPolicy {
    /// Fail when both carpets contain a cell at the same index.
    Reject,
    /// Keep the receiver's cell.
    KeepBase,
    /// Keep the overlaid carpet's cell.
    KeepOverlay,
    /// Place both exact music objects in a canonical parallel composition.
    Parallel,
}

/// Boundary behavior for [`MusicCarpet::slice`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SlicePolicy {
    /// Reject a slice that extends beyond the selected axis.
    Strict,
    /// Clamp the slice to the selected axis.
    Clamp,
    /// Wrap a finite slice around an explicitly cyclic axis.
    WrapCyclic,
}

/// A sparse finite arrangement of exact [`Music`] values.
#[derive(Clone, Debug)]
pub struct MusicCarpet {
    /// Ordered finite coordinate axes.
    pub axes: Vec<CarpetAxis>,
    /// Exact musical cell values keyed by coordinate.
    pub cells: BTreeMap<CarpetIndex, Music>,
    /// Explicit shape and addressing policy.
    pub policy: CarpetPolicy,
}

/// One transform diagnostic attributed to its carpet cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CarpetTransformDiagnostic {
    /// Cell whose transform emitted the diagnostic.
    pub index: CarpetIndex,
    /// Diagnostic from the existing music-transform chain.
    pub diagnostic: TransformDiagnostic,
}

/// Result of applying one transform chain to every occupied cell.
#[derive(Clone, Debug)]
pub struct CarpetTransformReport {
    /// Transformed carpet.
    pub carpet: MusicCarpet,
    /// Cell-addressed transform diagnostics.
    pub diagnostics: Vec<CarpetTransformDiagnostic>,
}

/// Error raised by carpet construction or algebra.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CarpetError {
    /// Strict empty policy rejected the carpet.
    #[error("empty carpet is not allowed")]
    EmptyCarpet,
    /// Strict empty policy rejected an axis.
    #[error("axis {axis} is empty")]
    EmptyAxis {
        /// Zero-based axis position.
        axis: usize,
    },
    /// An axis name was empty.
    #[error("axis {axis} has an empty name")]
    EmptyAxisName {
        /// Zero-based axis position.
        axis: usize,
    },
    /// Two axes shared one name.
    #[error("duplicate axis name {name}")]
    DuplicateAxisName {
        /// Duplicated name.
        name: String,
    },
    /// A cell coordinate count did not match the axis count.
    #[error("index {index:?} has arity {actual}, expected {expected}")]
    IndexArity {
        /// Offending index.
        index: CarpetIndex,
        /// Required coordinate count.
        expected: usize,
        /// Supplied coordinate count.
        actual: usize,
    },
    /// A coordinate was outside a strict or non-cyclic axis.
    #[error("coordinate {coordinate} is outside axis {axis} of length {length}")]
    CoordinateOutOfRange {
        /// Zero-based axis position.
        axis: usize,
        /// Offending coordinate.
        coordinate: usize,
        /// Axis length.
        length: usize,
    },
    /// Wrapped coordinates collided.
    #[error("multiple cells normalize to index {index:?}")]
    NormalizedCollision {
        /// Colliding normalized index.
        index: CarpetIndex,
    },
    /// Rectangular policy found missing cells.
    #[error("ragged carpet has {actual} cells, expected {expected}")]
    Ragged {
        /// Complete Cartesian cell count.
        expected: usize,
        /// Supplied cell count.
        actual: usize,
    },
    /// Axis lengths overflowed the host cell-count representation.
    #[error("carpet shape is too large")]
    ShapeTooLarge,
    /// An operation selected no such axis or selected one axis twice.
    #[error("invalid carpet axis selection")]
    InvalidAxisSelection,
    /// An overlay used different axes.
    #[error("carpet overlay requires identical axes")]
    AxisMismatch,
    /// Strict overlay found an occupied coordinate.
    #[error("carpet overlay collision at {index:?}")]
    OverlayCollision {
        /// Colliding index.
        index: CarpetIndex,
    },
    /// A slice request violated its boundary policy.
    #[error("slice start {start} length {length} is invalid for axis length {axis_length}")]
    InvalidSlice {
        /// Requested start.
        start: usize,
        /// Requested length.
        length: usize,
        /// Selected axis length.
        axis_length: usize,
    },
    /// The shared discrete rank adapter rejected an index or ordinal.
    #[error("discrete rank error: {0}")]
    Rank(String),
    /// A shared music transform rejected one cell.
    #[error("music transform failed at {index:?}: {detail}")]
    Transform {
        /// Failing cell.
        index: CarpetIndex,
        /// Underlying transform detail.
        detail: String,
    },
    /// Relative conversion found malformed event data.
    #[error("relative music is invalid at {index:?}: {detail}")]
    Relative {
        /// Failing cell.
        index: CarpetIndex,
        /// Validation detail.
        detail: String,
    },
}

impl MusicCarpet {
    /// Builds and validates a carpet, normalizing explicitly wrapped coordinates.
    pub fn new(
        axes: Vec<CarpetAxis>,
        cells: BTreeMap<CarpetIndex, Music>,
        policy: CarpetPolicy,
    ) -> Result<Self, CarpetError> {
        validate_axes(&axes, policy)?;
        if cells.is_empty() && policy.empty == EmptyPolicy::Reject {
            return Err(CarpetError::EmptyCarpet);
        }
        let mut normalized = BTreeMap::new();
        for (index, music) in cells {
            let index = normalize_index(&axes, policy.out_of_range, &index)?;
            if normalized.insert(index.clone(), music).is_some() {
                return Err(CarpetError::NormalizedCollision { index });
            }
        }
        if policy.ragged == RaggedPolicy::Reject {
            let expected = rectangular_len(&axes)?;
            if normalized.len() != expected {
                return Err(CarpetError::Ragged {
                    expected,
                    actual: normalized.len(),
                });
            }
        }
        Ok(Self {
            axes,
            cells: normalized,
            policy,
        })
    }

    /// Returns a cell by its already-normalized coordinate.
    pub fn cell(&self, index: &CarpetIndex) -> Option<&Music> {
        self.cells.get(index)
    }

    /// Ranks an index through the canonical discrete mixed-radix space.
    pub fn rank_index(&self, index: &CarpetIndex) -> Result<Nat, CarpetError> {
        let normalized = normalize_index(&self.axes, self.policy.out_of_range, index)?;
        let digits = normalized
            .coordinates
            .iter()
            .map(|value| u64::try_from(*value).map_err(|_| CarpetError::ShapeTooLarge))
            .collect::<Result<Vec<_>, _>>()?;
        rank_space(&self.axes)?
            .rank(&digits)
            .map_err(|error| CarpetError::Rank(error.to_string()))
    }

    /// Unranks one canonical discrete ordinal into a carpet index.
    pub fn index_at_rank(&self, ordinal: &Nat) -> Result<CarpetIndex, CarpetError> {
        let digits = rank_space(&self.axes)?
            .unrank(ordinal)
            .map_err(|error| CarpetError::Rank(error.to_string()))?;
        let coordinates = digits
            .into_iter()
            .map(|value| usize::try_from(value).map_err(|_| CarpetError::ShapeTooLarge))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CarpetIndex::new(coordinates))
    }
}

fn validate_axes(axes: &[CarpetAxis], policy: CarpetPolicy) -> Result<(), CarpetError> {
    if axes.is_empty() && policy.empty == EmptyPolicy::Reject {
        return Err(CarpetError::EmptyCarpet);
    }
    let mut names = BTreeSet::new();
    for (axis, value) in axes.iter().enumerate() {
        if value.name.is_empty() {
            return Err(CarpetError::EmptyAxisName { axis });
        }
        if !names.insert(value.name.clone()) {
            return Err(CarpetError::DuplicateAxisName {
                name: value.name.clone(),
            });
        }
        if value.is_empty() && policy.empty == EmptyPolicy::Reject {
            return Err(CarpetError::EmptyAxis { axis });
        }
    }
    Ok(())
}

fn normalize_index(
    axes: &[CarpetAxis],
    policy: OutOfRangePolicy,
    index: &CarpetIndex,
) -> Result<CarpetIndex, CarpetError> {
    if index.coordinates.len() != axes.len() {
        return Err(CarpetError::IndexArity {
            index: index.clone(),
            expected: axes.len(),
            actual: index.coordinates.len(),
        });
    }
    let mut normalized = index.clone();
    for (axis, (coordinate, definition)) in normalized.coordinates.iter_mut().zip(axes).enumerate()
    {
        if *coordinate < definition.len() {
            continue;
        }
        if policy == OutOfRangePolicy::WrapCyclic && definition.cyclic && !definition.is_empty() {
            *coordinate %= definition.len();
        } else {
            return Err(CarpetError::CoordinateOutOfRange {
                axis,
                coordinate: *coordinate,
                length: definition.len(),
            });
        }
    }
    Ok(normalized)
}

fn rectangular_len(axes: &[CarpetAxis]) -> Result<usize, CarpetError> {
    axes.iter().try_fold(1_usize, |count, axis| {
        count
            .checked_mul(axis.len())
            .ok_or(CarpetError::ShapeTooLarge)
    })
}

fn rank_space(axes: &[CarpetAxis]) -> Result<BoundedIntVectorSpace, CarpetError> {
    let radices = axes
        .iter()
        .map(|axis| u64::try_from(axis.len()).map_err(|_| CarpetError::ShapeTooLarge))
        .collect::<Result<Vec<_>, _>>()?;
    if radices.is_empty() || radices.contains(&0) {
        return Err(CarpetError::Rank(
            "empty axes have no rankable coordinates".to_owned(),
        ));
    }
    Ok(BoundedIntVectorSpace { radices })
}
