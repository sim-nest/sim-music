//! Bounds-checked ordered row segments with reused pitch-set evidence.

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_namer_forte::lookup_forte_label;
use sim_lib_pitch_set::{IntervalVector, PitchClassMask, SetClass, SetEquivalence, classify_set};

use crate::{OrderedIntervalString, RowError, ToneRow, interval::ordered_intervals_vec};

/// How a [`RowSegment`] was extracted from its source row.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RowSegmentSource {
    /// A contiguous slice `[start, start + len)`.
    Contiguous {
        /// Zero-based ordinal at which the segment starts.
        start: usize,
        /// Number of row positions included.
        len: usize,
    },
    /// A wrapping slice starting at `start` and continuing modulo twelve.
    Wrapped {
        /// Zero-based ordinal at which the wrapped segment starts.
        start: usize,
        /// Number of row positions included.
        len: usize,
    },
    /// An explicit ordinal sequence.
    Indexed,
}

/// An order-preserving segment of a tone row plus unordered pitch-set facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowSegment {
    source: RowSegmentSource,
    ordinals: Vec<u8>,
    classes: Vec<PitchClass>,
    mask: PitchClassMask,
    interval_vector: IntervalVector,
    set_class: SetClass,
    forte_label: Option<&'static str>,
}

impl RowSegment {
    /// Returns how the segment was extracted from its source row.
    pub const fn source(&self) -> &RowSegmentSource {
        &self.source
    }

    /// Returns the source ordinals in retained presentation order.
    pub fn ordinals(&self) -> &[u8] {
        &self.ordinals
    }

    /// Returns the pitch classes in retained presentation order.
    pub fn classes(&self) -> &[PitchClass] {
        &self.classes
    }

    /// Returns the derived unordered pitch-class mask.
    pub const fn mask(&self) -> PitchClassMask {
        self.mask
    }

    /// Returns the interval-class census of the unordered projection.
    pub const fn interval_vector(&self) -> IntervalVector {
        self.interval_vector
    }

    /// Returns the transposition-and-inversion set class of the unordered projection.
    pub fn set_class(&self) -> &SetClass {
        &self.set_class
    }

    /// Returns the reused Forte label when the existing naming table has one.
    pub const fn forte_label(&self) -> Option<&'static str> {
        self.forte_label
    }

    /// Returns the directed ordered intervals between adjacent segment members.
    pub fn ordered_intervals(&self) -> Vec<u8> {
        ordered_intervals_vec(&self.classes)
    }

    pub(crate) fn new(
        source: RowSegmentSource,
        ordinals: Vec<u8>,
        classes: Vec<PitchClass>,
    ) -> Self {
        let mask = PitchClassMask::from_pitch_classes(&classes);
        let set_class = classify_set(mask, SetEquivalence::TranspositionInversion);
        Self {
            source,
            ordinals,
            classes,
            mask,
            interval_vector: mask.interval_vector(),
            forte_label: lookup_forte_label(mask),
            set_class,
        }
    }
}

impl ToneRow {
    /// Extracts a contiguous segment `[start, start + len)` from the row.
    pub fn segment(&self, start: usize, len: usize) -> Result<RowSegment, RowError> {
        if start > self.classes().len() || start + len > self.classes().len() {
            return Err(RowError::SegmentOutOfBounds { start, len });
        }
        let ordinals = (start..start + len).map(|ordinal| ordinal as u8).collect();
        let classes = self.classes()[start..start + len].to_vec();
        Ok(RowSegment::new(
            RowSegmentSource::Contiguous { start, len },
            ordinals,
            classes,
        ))
    }

    /// Extracts a wrapping segment starting at `start` and continuing modulo twelve.
    pub fn wrapped_segment(&self, start: usize, len: usize) -> Result<RowSegment, RowError> {
        if start >= self.classes().len() {
            return Err(RowError::InvalidOrdinal { ordinal: start });
        }
        if len > self.classes().len() {
            return Err(RowError::WrappedSegmentTooLong { len });
        }
        let ordinals = (0..len)
            .map(|offset| ((start + offset) % self.classes().len()) as u8)
            .collect::<Vec<_>>();
        let classes = ordinals
            .iter()
            .map(|ordinal| self.classes()[usize::from(*ordinal)])
            .collect();
        Ok(RowSegment::new(
            RowSegmentSource::Wrapped { start, len },
            ordinals,
            classes,
        ))
    }

    /// Extracts a segment from an explicit ordinal sequence.
    pub fn indexed_segment(&self, ordinals: &[usize]) -> Result<RowSegment, RowError> {
        let mut classes = Vec::with_capacity(ordinals.len());
        let mut stored_ordinals = Vec::with_capacity(ordinals.len());
        for ordinal in ordinals {
            let Some(class) = self.classes().get(*ordinal).copied() else {
                return Err(RowError::InvalidOrdinal { ordinal: *ordinal });
            };
            classes.push(class);
            stored_ordinals.push(*ordinal as u8);
        }
        Ok(RowSegment::new(
            RowSegmentSource::Indexed,
            stored_ordinals,
            classes,
        ))
    }

    /// Returns the directed ordered intervals between adjacent row positions.
    pub fn ordered_intervals(&self) -> OrderedIntervalString {
        OrderedIntervalString::of_row(self)
    }
}
