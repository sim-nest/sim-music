//! Complete row families with alias-preserving symmetry reduction.

use crate::{RowFamily, RowForm, RowOperation, ToneRow};

const FAMILIES: [RowFamily; 4] = [RowFamily::P, RowFamily::I, RowFamily::R, RowFamily::RI];

/// One operation alias in a complete twelve-tone row family.
///
/// Aliases are never removed when two operations produce the same row. The
/// [`RowAlias::distinct_form_index`] links the operation-bearing form to the
/// corresponding deduplicated row in [`RowFamilySet::distinct_forms`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowAlias {
    /// The normalized operation represented by this alias.
    pub operation: RowOperation,
    /// The strict row form produced by applying `operation` to the source row.
    pub form: RowForm,
    distinct_form_index: usize,
}

impl RowAlias {
    /// Returns the index of this alias's row in the deduplicated form collection.
    pub const fn distinct_form_index(&self) -> usize {
        self.distinct_form_index
    }
}

/// All 48 P/I/R/RI aliases for one row, plus its distinct resulting rows.
///
/// The alias order is stable: P0..P11, I0..I11, R0..R11, then RI0..RI11.
/// Symmetric rows may have fewer than 48 distinct values, but every operation
/// remains addressable through [`RowFamilySet::aliases`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowFamilySet {
    source: ToneRow,
    aliases: Vec<RowAlias>,
    distinct_forms: Vec<ToneRow>,
}

impl RowFamilySet {
    /// Builds the complete operation family for `source`.
    pub fn of(source: &ToneRow) -> Self {
        let mut aliases = Vec::with_capacity(48);
        let mut distinct_forms = Vec::with_capacity(48);

        for family in FAMILIES {
            for addend in 0..12 {
                let operation = RowOperation::new(family, addend);
                let form = source.apply(operation);
                let distinct_form_index = distinct_forms
                    .iter()
                    .position(|distinct| distinct == form.row())
                    .unwrap_or_else(|| {
                        distinct_forms.push(form.row().clone());
                        distinct_forms.len() - 1
                    });
                aliases.push(RowAlias {
                    operation,
                    form,
                    distinct_form_index,
                });
            }
        }

        Self {
            source: source.clone(),
            aliases,
            distinct_forms,
        }
    }

    /// Returns the row from which every alias was derived.
    pub const fn source(&self) -> &ToneRow {
        &self.source
    }

    /// Returns all 48 aliases in stable family-and-addend order.
    pub fn aliases(&self) -> &[RowAlias] {
        &self.aliases
    }

    /// Returns the deduplicated row values in first-alias order.
    pub fn distinct_forms(&self) -> &[ToneRow] {
        &self.distinct_forms
    }

    /// Iterates over every alias that resolves to one distinct form.
    ///
    /// An out-of-range index yields an empty iterator.
    pub fn aliases_for_distinct_form(
        &self,
        distinct_form_index: usize,
    ) -> impl Iterator<Item = &RowAlias> {
        self.aliases.iter().filter(move |alias| {
            alias.distinct_form_index == distinct_form_index
                && distinct_form_index < self.distinct_forms.len()
        })
    }
}
