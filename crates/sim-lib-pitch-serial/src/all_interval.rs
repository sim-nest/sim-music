//! All-interval evidence for strict tone rows.

use crate::ToneRow;

/// One duplicated adjacent directed interval together with its multiplicity.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AllIntervalMultiplicity {
    /// The directed interval value in semitones modulo twelve.
    pub interval: u8,
    /// The number of times `interval` occurs in the row.
    pub count: u8,
}

/// Whether a row uses each non-zero directed adjacent interval exactly once.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AllIntervalReport {
    /// Whether the row contains each interval `1..=11` exactly once.
    pub is_all_interval: bool,
    /// Intervals whose multiplicity exceeds one.
    pub duplicates: Vec<AllIntervalMultiplicity>,
    /// Intervals in `1..=11` that do not occur.
    pub missing: Vec<u8>,
}

/// Computes all-interval evidence from the row's directed adjacent intervals.
pub fn analyze_all_interval(row: &ToneRow) -> AllIntervalReport {
    let mut counts = [0u8; 12];
    for interval in row.ordered_intervals().intervals() {
        counts[usize::from(*interval)] += 1;
    }
    let duplicates = (1..12)
        .filter_map(|interval| {
            (counts[interval] > 1).then_some(AllIntervalMultiplicity {
                interval: interval as u8,
                count: counts[interval],
            })
        })
        .collect::<Vec<_>>();
    let missing = (1..12)
        .filter_map(|interval| (counts[interval] == 0).then_some(interval as u8))
        .collect::<Vec<_>>();
    AllIntervalReport {
        is_all_interval: duplicates.is_empty() && missing.is_empty(),
        duplicates,
        missing,
    }
}
