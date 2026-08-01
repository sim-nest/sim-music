//! Row-class reports that retain aliases, stabilizers, and invariance evidence.

use crate::{
    OrderedIntervalString, RowFamilySet, RowOperation, SegmentInvariant, ToneRow,
    analyze_invariance,
};

/// One alias entry in a [`RowClassReport`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct RowClassAlias {
    /// The represented row-family operation.
    pub operation: RowOperation,
    /// The index of the corresponding row in [`RowClassReport::distinct_forms`].
    pub distinct_form_index: usize,
}

/// One distinct form together with the operations and invariance facts that realize it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormEquivalence {
    /// The deduplicated form index.
    pub distinct_form_index: usize,
    /// The distinct row value.
    pub row: ToneRow,
    /// Every alias operation that realizes `row`.
    pub operations: Vec<RowOperation>,
    /// Whole-row invariance facts comparing the source row to `row`.
    pub invariant: SegmentInvariant,
}

/// Ordered, symmetric, and equivalence evidence for one strict tone row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowClassReport {
    /// The analyzed source row.
    pub row: ToneRow,
    /// The source row's directed ordered-interval string.
    pub ordered_intervals: OrderedIntervalString,
    /// Every operation alias paired with its deduplicated row index.
    pub aliases: Vec<RowClassAlias>,
    /// The deduplicated row values in first-alias order.
    pub distinct_forms: Vec<ToneRow>,
    /// Operations that stabilize the source row exactly.
    pub stabilizers: Vec<RowOperation>,
    /// Distinct forms grouped with their alias and invariance evidence.
    pub form_equivalences: Vec<FormEquivalence>,
}

/// Analyzes one row's ordered intervals, symmetry stabilizers, and form equivalences.
pub fn analyze_row_class(row: &ToneRow) -> RowClassReport {
    let family = RowFamilySet::of(row);
    let aliases = family
        .aliases()
        .iter()
        .map(|alias| RowClassAlias {
            operation: alias.operation,
            distinct_form_index: alias.distinct_form_index(),
        })
        .collect::<Vec<_>>();
    let distinct_forms = family.distinct_forms().to_vec();
    let stabilizers = family
        .aliases()
        .iter()
        .filter(|alias| alias.form.row() == row)
        .map(|alias| alias.operation)
        .collect::<Vec<_>>();
    let left = row
        .indexed_segment(&(0..row.classes().len()).collect::<Vec<_>>())
        .expect("full-row indexed segment is valid");
    let form_equivalences = distinct_forms
        .iter()
        .enumerate()
        .map(|(distinct_form_index, form)| {
            let right = form
                .indexed_segment(&(0..form.classes().len()).collect::<Vec<_>>())
                .expect("full-row indexed segment is valid");
            let operations = family
                .aliases_for_distinct_form(distinct_form_index)
                .map(|alias| alias.operation)
                .collect::<Vec<_>>();
            FormEquivalence {
                distinct_form_index,
                row: form.clone(),
                operations,
                invariant: analyze_invariance(&left, &right),
            }
        })
        .collect();

    RowClassReport {
        row: row.clone(),
        ordered_intervals: row.ordered_intervals(),
        aliases,
        distinct_forms,
        stabilizers,
        form_equivalences,
    }
}
