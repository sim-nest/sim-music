//! Total prime, inversion, retrograde, and retrograde-inversion operations.

use std::fmt::{Display, Formatter};

/// One of the four classical twelve-tone row-operation families.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RowFamily {
    /// Prime order under transposition.
    P,
    /// Inversion order under transposition.
    I,
    /// Retrograde of the prime order under transposition.
    R,
    /// Retrograde of the inversion order under transposition.
    RI,
}

impl RowFamily {
    /// Returns the conventional short family token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::P => "P",
            Self::I => "I",
            Self::R => "R",
            Self::RI => "RI",
        }
    }

    pub(crate) const fn is_inverted(self) -> bool {
        matches!(self, Self::I | Self::RI)
    }

    pub(crate) const fn is_retrograde(self) -> bool {
        matches!(self, Self::R | Self::RI)
    }
}

impl Display for RowFamily {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A total affine/reversal operation on a strict tone row.
///
/// The `addend` is the affine constant in `x -> x + addend` for P/R and
/// `x -> -x + addend` for I/RI. Values are reduced modulo twelve when the
/// operation is applied, so even a struct literal containing an arbitrary `u8`
/// remains total.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct RowOperation {
    /// Prime, inversion, retrograde, or retrograde-inversion family.
    pub family: RowFamily,
    /// Affine addend, interpreted modulo twelve.
    pub addend: u8,
}

impl RowOperation {
    /// Constructs a row operation with a canonical modulo-twelve addend.
    pub const fn new(family: RowFamily, addend: u8) -> Self {
        Self {
            family,
            addend: addend % 12,
        }
    }

    /// Returns the canonical operation identity with its addend reduced modulo twelve.
    pub const fn normalized(self) -> Self {
        Self::new(self.family, self.addend)
    }

    /// Returns the exact inverse operation.
    pub const fn inverse(self) -> Self {
        let operation = self.normalized();
        let addend = match operation.family {
            RowFamily::P | RowFamily::R => (12 - operation.addend) % 12,
            RowFamily::I | RowFamily::RI => operation.addend,
        };
        Self::new(operation.family, addend)
    }
}

impl Display for RowOperation {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let operation = self.normalized();
        write!(formatter, "{}{}", operation.family, operation.addend)
    }
}
