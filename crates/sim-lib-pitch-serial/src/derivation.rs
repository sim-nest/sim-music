//! Generator-cell derivation analysis for strict tone rows.

use crate::{RowError, RowFamily, RowOperation, ToneRow};

const DERIVATION_PARTITIONS: [usize; 4] = [2, 3, 4, 6];

/// One detected derivation family for a fixed generator-cell size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivationMatch {
    /// The generator-cell size in row positions.
    pub generator_size: usize,
    /// The detected named derivation kind.
    pub kind: DerivationKind,
    /// The operation relating each partition cell back to the generator cell.
    pub cells: Vec<DerivationCellRelation>,
}

/// Stable derivation names for the classical equal-cell partitions of a row.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DerivationKind {
    /// Two-note generator cells.
    Dyadic,
    /// Three-note generator cells.
    Trichordal,
    /// Four-note generator cells.
    Tetrachordal,
    /// Six-note generator cells.
    Hexachordal,
}

impl DerivationKind {
    fn of_size(size: usize) -> Option<Self> {
        match size {
            2 => Some(Self::Dyadic),
            3 => Some(Self::Trichordal),
            4 => Some(Self::Tetrachordal),
            6 => Some(Self::Hexachordal),
            _ => None,
        }
    }
}

/// One partition cell together with the operation that derives it from the generator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivationCellRelation {
    /// Zero-based contiguous cell index in source-row order.
    pub cell_index: usize,
    /// The contiguous row ordinals belonging to this cell.
    pub ordinals: Vec<u8>,
    /// The exact affine/reversal operation mapping the generator onto this cell.
    pub operation: RowOperation,
}

/// Complete derivation evidence for one strict row.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct DerivationReport {
    /// The largest detected generator-cell size, when any derivation is present.
    pub generator_size: Option<usize>,
    /// Every detected derivation family in ascending partition-size order.
    pub matches: Vec<DerivationMatch>,
}

/// Detects dyadic, trichordal, tetrachordal, and hexachordal derivation.
pub fn analyze_derivation(row: &ToneRow) -> DerivationReport {
    let matches = DERIVATION_PARTITIONS
        .into_iter()
        .filter_map(|size| analyze_derivation_partition(row, size).ok().flatten())
        .collect::<Vec<_>>();
    let generator_size = matches.iter().map(|entry| entry.generator_size).max();
    DerivationReport {
        generator_size,
        matches,
    }
}

/// Detects derivation for one supported generator-cell size.
pub fn analyze_derivation_partition(
    row: &ToneRow,
    generator_size: usize,
) -> Result<Option<DerivationMatch>, RowError> {
    let Some(kind) = DerivationKind::of_size(generator_size) else {
        return Err(RowError::InvalidPartitionSize {
            size: generator_size,
        });
    };
    let generator = &row.classes()[0..generator_size];
    let mut cells = Vec::with_capacity(12 / generator_size);
    for (cell_index, chunk) in row.classes().chunks(generator_size).enumerate() {
        let Some(operation) = related_operation(generator, chunk) else {
            return Ok(None);
        };
        let start = cell_index * generator_size;
        cells.push(DerivationCellRelation {
            cell_index,
            ordinals: (start..start + generator_size)
                .map(|ordinal| ordinal as u8)
                .collect(),
            operation,
        });
    }
    Ok(Some(DerivationMatch {
        generator_size,
        kind,
        cells,
    }))
}

fn related_operation(
    left: &[sim_lib_pitch_core::PitchClass],
    right: &[sim_lib_pitch_core::PitchClass],
) -> Option<RowOperation> {
    [RowFamily::P, RowFamily::I, RowFamily::R, RowFamily::RI]
        .into_iter()
        .flat_map(|family| (0..12).map(move |addend| RowOperation::new(family, addend)))
        .find(|operation| apply_operation(left, *operation) == right)
}

fn apply_operation(
    source: &[sim_lib_pitch_core::PitchClass],
    operation: RowOperation,
) -> Vec<sim_lib_pitch_core::PitchClass> {
    let mut classes = source.to_vec();
    if operation.family.is_retrograde() {
        classes.reverse();
    }
    classes
        .into_iter()
        .map(|class| {
            let class = if operation.family.is_inverted() {
                class.invert(sim_lib_pitch_core::PitchClass::C)
            } else {
                class
            };
            class.transpose(i32::from(operation.addend))
        })
        .collect()
}
