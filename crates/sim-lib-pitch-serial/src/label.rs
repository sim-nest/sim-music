//! Convention-dependent row-form labels.

use std::fmt::{Display, Formatter};

use crate::{RowFamily, RowForm};

/// A printable family/index label such as `P0` or `RI11`.
///
/// A label does not replace [`crate::RowOperation`]; it records only the family
/// and index selected by an explicit [`RowLabelConvention`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RowLabel {
    family: RowFamily,
    index: u8,
}

impl RowLabel {
    /// Constructs a label, reducing the index modulo twelve.
    pub const fn new(family: RowFamily, index: u8) -> Self {
        Self {
            family,
            index: index % 12,
        }
    }

    /// Returns the label family.
    pub const fn family(self) -> RowFamily {
        self.family
    }

    /// Returns the modulo-twelve label index.
    pub const fn index(self) -> u8 {
        self.index
    }
}

impl Display for RowLabel {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}{}", self.family, self.index)
    }
}

/// Policy for projecting an operation-bearing row form to a printed label.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum RowLabelConvention {
    /// Label P/I from the first sounding class and R/RI from the last.
    ///
    /// Using the last class for retrogrades keeps a family and its retrograde on
    /// the same index under the common first/last-pitch convention.
    FirstLastPitch,
    /// Label every family with the affine addend of its normalized operation.
    OperationIndex,
}

impl RowLabelConvention {
    /// Projects `form` to a label without changing its operation identity.
    pub fn label(self, form: &RowForm) -> RowLabel {
        let operation = form.operation();
        let index = match self {
            Self::FirstLastPitch if operation.family.is_retrograde() => form.classes()[11].value(),
            Self::FirstLastPitch => form.classes()[0].value(),
            Self::OperationIndex => operation.addend,
        };
        RowLabel::new(operation.family, index)
    }
}
