//! Directed ordered-interval analysis for strict tone rows and row segments.

use sim_lib_pitch_core::PitchClass;

use crate::{RowFamily, ToneRow};

/// The directed ordered intervals between adjacent pitches in source order.
///
/// Values are modulo-twelve semitone distances in `0..12`, so they preserve
/// the row's ordinal contour without collapsing inversional complements.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct OrderedIntervalString {
    intervals: [u8; 11],
}

impl OrderedIntervalString {
    /// Computes the directed ordered intervals for `row`.
    pub fn of_row(row: &ToneRow) -> Self {
        Self {
            intervals: ordered_intervals(row.classes()),
        }
    }

    /// Returns the eleven directed intervals in source order.
    pub const fn intervals(&self) -> &[u8; 11] {
        &self.intervals
    }

    /// Returns the ordered-interval string implied by a row-family operation.
    ///
    /// Prime preserves order and direction, inversion negates every interval,
    /// retrograde reverses the order and negates the direction, and
    /// retrograde-inversion reverses the order only.
    pub const fn under_family(self, family: RowFamily) -> Self {
        let mut intervals = self.intervals;
        match family {
            RowFamily::P => {}
            RowFamily::I => {
                let mut index = 0;
                while index < intervals.len() {
                    intervals[index] = (12 - intervals[index]) % 12;
                    index += 1;
                }
            }
            RowFamily::R => {
                intervals.reverse();
                let mut index = 0;
                while index < intervals.len() {
                    intervals[index] = (12 - intervals[index]) % 12;
                    index += 1;
                }
            }
            RowFamily::RI => intervals.reverse(),
        }
        Self { intervals }
    }
}

pub(crate) fn ordered_intervals(classes: &[PitchClass]) -> [u8; 11] {
    debug_assert_eq!(classes.len(), 12);
    std::array::from_fn(|index| {
        (12 + i16::from(classes[index + 1].value()) - i16::from(classes[index].value())) as u8 % 12
    })
}

pub(crate) fn ordered_intervals_vec(classes: &[PitchClass]) -> Vec<u8> {
    classes
        .windows(2)
        .map(|window| (12 + i16::from(window[1].value()) - i16::from(window[0].value())) as u8 % 12)
        .collect()
}
