//! Strict tone-row values and invariant-preserving operation results.

use sim_lib_pitch_core::PitchClass;
use sim_lib_serial_core::{AggregateRule, Series};

use crate::{PitchClassAlphabet, RowError, RowLabel, RowLabelConvention, RowOperation};

/// An ordered aggregate containing every canonical pitch class exactly once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToneRow {
    classes: [PitchClass; 12],
}

impl ToneRow {
    /// Constructs a row after exact membership and multiplicity validation.
    ///
    /// The fixed array establishes cardinality. [`Series`] with the exhaustive
    /// exactly-once rule supplies the canonical alphabet membership and
    /// multiplicity proof.
    pub fn try_from_classes(classes: [PitchClass; 12]) -> Result<Self, RowError> {
        let alphabet = PitchClassAlphabet::try_new()?;
        Series::try_new(
            alphabet,
            AggregateRule::exhaustive_exactly_once(),
            classes.to_vec(),
        )?;
        Ok(Self::from_valid_classes(classes))
    }

    /// Returns the twelve classes in row order.
    pub const fn classes(&self) -> &[PitchClass; 12] {
        &self.classes
    }

    /// Applies a total P/I/R/RI operation and retains its normalized identity.
    pub fn apply(&self, operation: RowOperation) -> RowForm {
        let operation = operation.normalized();
        let mut classes = std::array::from_fn(|position| {
            let class = self.classes[position];
            let class = if operation.family.is_inverted() {
                class.invert(PitchClass::C)
            } else {
                class
            };
            class.transpose(i32::from(operation.addend))
        });
        if operation.family.is_retrograde() {
            classes.reverse();
        }
        RowForm {
            row: Self::from_valid_classes(classes),
            operation,
        }
    }

    pub(crate) const fn from_valid_classes(classes: [PitchClass; 12]) -> Self {
        Self { classes }
    }
}

/// A strict tone row paired with the operation identity that produced it.
///
/// The operation is algebraic provenance. Printed labels are derived separately
/// with [`RowForm::label`] and an explicit [`RowLabelConvention`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowForm {
    row: ToneRow,
    operation: RowOperation,
}

impl RowForm {
    /// Returns the normalized operation identity that produced this form.
    pub const fn operation(&self) -> RowOperation {
        self.operation
    }

    /// Returns the strict row value carried by this form.
    pub const fn row(&self) -> &ToneRow {
        &self.row
    }

    /// Returns the twelve pitch classes in form order.
    pub const fn classes(&self) -> &[PitchClass; 12] {
        self.row.classes()
    }

    /// Derives a printed label under the selected convention.
    pub fn label(&self, convention: RowLabelConvention) -> RowLabel {
        convention.label(self)
    }

    /// Discards operation provenance and returns the strict row value.
    pub fn into_row(self) -> ToneRow {
        self.row
    }
}
