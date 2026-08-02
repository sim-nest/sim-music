//! Convention-explicit twelve-tone row matrices.

use sim_lib_pitch_core::PitchClass;

use crate::{RowFamily, RowForm, RowLabel, RowLabelConvention, RowOperation, ToneRow};

/// The width and height of every twelve-tone row matrix.
pub const ROW_MATRIX_SIZE: usize = 12;

/// A validated zero-based coordinate in a twelve-tone matrix.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MatrixCoordinate {
    row: u8,
    column: u8,
}

impl MatrixCoordinate {
    /// Constructs a coordinate, returning `None` outside the 12-by-12 matrix.
    pub const fn new(row: usize, column: usize) -> Option<Self> {
        if row < ROW_MATRIX_SIZE && column < ROW_MATRIX_SIZE {
            Some(Self {
                row: row as u8,
                column: column as u8,
            })
        } else {
            None
        }
    }

    /// Returns the zero-based row index.
    pub const fn row(self) -> usize {
        self.row as usize
    }

    /// Returns the zero-based column index.
    pub const fn column(self) -> usize {
        self.column as usize
    }
}

/// One pitch-class cell paired with its matrix coordinate.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct RowMatrixCell {
    coordinate: MatrixCoordinate,
    class: PitchClass,
}

impl RowMatrixCell {
    pub(crate) const fn new(coordinate: MatrixCoordinate, class: PitchClass) -> Self {
        Self { coordinate, class }
    }

    /// Returns this cell's zero-based coordinate.
    pub const fn coordinate(self) -> MatrixCoordinate {
        self.coordinate
    }

    /// Returns the pitch class stored in this cell.
    pub const fn class(self) -> PitchClass {
        self.class
    }
}

/// Labels printed on the four edges of a row matrix.
///
/// Left-to-right rows are P forms and right-to-left rows are R forms. Top-to-
/// bottom columns are I forms and bottom-to-top columns are RI forms.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RowMatrixEdgeLabels {
    top: [RowLabel; ROW_MATRIX_SIZE],
    right: [RowLabel; ROW_MATRIX_SIZE],
    bottom: [RowLabel; ROW_MATRIX_SIZE],
    left: [RowLabel; ROW_MATRIX_SIZE],
}

impl RowMatrixEdgeLabels {
    /// Returns the I labels above the columns, ordered left to right.
    pub const fn top(&self) -> &[RowLabel; ROW_MATRIX_SIZE] {
        &self.top
    }

    /// Returns the R labels beside the rows, ordered top to bottom.
    pub const fn right(&self) -> &[RowLabel; ROW_MATRIX_SIZE] {
        &self.right
    }

    /// Returns the RI labels below the columns, ordered left to right.
    pub const fn bottom(&self) -> &[RowLabel; ROW_MATRIX_SIZE] {
        &self.bottom
    }

    /// Returns the P labels beside the rows, ordered top to bottom.
    pub const fn left(&self) -> &[RowLabel; ROW_MATRIX_SIZE] {
        &self.left
    }
}

/// A conventional twelve-tone matrix retaining its source and label policy.
///
/// The first row is the source row (`P0` by operation identity). Each matrix
/// row is a P form, each column is an I form, and the reverse readings are the
/// corresponding R and RI forms. Operations stay algebraic; edge labels are
/// projections through the matrix's explicit [`RowLabelConvention`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowMatrix {
    source: ToneRow,
    convention: RowLabelConvention,
    cells: [[PitchClass; ROW_MATRIX_SIZE]; ROW_MATRIX_SIZE],
    row_operations: [RowOperation; ROW_MATRIX_SIZE],
    column_operations: [RowOperation; ROW_MATRIX_SIZE],
    edge_labels: RowMatrixEdgeLabels,
}

impl RowMatrix {
    /// Constructs the matrix for `source` under an explicit edge-label convention.
    pub fn new(source: &ToneRow, convention: RowLabelConvention) -> Self {
        let source_first = source.classes()[0].value();
        let row_operations = source.classes().map(|class| {
            RowOperation::new(
                RowFamily::P,
                subtract_mod_twelve(source_first, class.value()),
            )
        });
        let column_operations = source
            .classes()
            .map(|class| RowOperation::new(RowFamily::I, (source_first + class.value()) % 12));
        let cells = std::array::from_fn(|row| *source.apply(row_operations[row]).classes());

        debug_assert!((0..ROW_MATRIX_SIZE).all(|column| {
            let expected = source.apply(column_operations[column]);
            (0..ROW_MATRIX_SIZE).all(|row| cells[row][column] == expected.classes()[row])
        }));

        let edge_labels = RowMatrixEdgeLabels {
            top: column_operations.map(|operation| source.apply(operation).label(convention)),
            right: row_operations.map(|operation| {
                source
                    .apply(RowOperation::new(RowFamily::R, operation.addend))
                    .label(convention)
            }),
            bottom: column_operations.map(|operation| {
                source
                    .apply(RowOperation::new(RowFamily::RI, operation.addend))
                    .label(convention)
            }),
            left: row_operations.map(|operation| source.apply(operation).label(convention)),
        };

        Self {
            source: source.clone(),
            convention,
            cells,
            row_operations,
            column_operations,
            edge_labels,
        }
    }

    /// Returns the source row retained by this matrix.
    pub const fn source(&self) -> &ToneRow {
        &self.source
    }

    /// Returns the label convention used by every edge label.
    pub const fn convention(&self) -> RowLabelConvention {
        self.convention
    }

    /// Returns all matrix pitch classes in row-major layout.
    pub const fn cells(&self) -> &[[PitchClass; ROW_MATRIX_SIZE]; ROW_MATRIX_SIZE] {
        &self.cells
    }

    /// Returns one coordinate-bearing cell.
    pub const fn cell(&self, coordinate: MatrixCoordinate) -> RowMatrixCell {
        RowMatrixCell::new(
            coordinate,
            self.cells[coordinate.row()][coordinate.column()],
        )
    }

    /// Returns one left-to-right matrix row, or `None` for an invalid index.
    pub fn row(&self, row: usize) -> Option<&[PitchClass; ROW_MATRIX_SIZE]> {
        self.cells.get(row)
    }

    /// Returns one top-to-bottom matrix column, or `None` for an invalid index.
    pub fn column(&self, column: usize) -> Option<[PitchClass; ROW_MATRIX_SIZE]> {
        (column < ROW_MATRIX_SIZE).then(|| std::array::from_fn(|row| self.cells[row][column]))
    }

    /// Returns the P operation read left to right on one row.
    pub fn row_operation(&self, row: usize) -> Option<RowOperation> {
        self.row_operations.get(row).copied()
    }

    /// Returns the I operation read top to bottom on one column.
    pub fn column_operation(&self, column: usize) -> Option<RowOperation> {
        self.column_operations.get(column).copied()
    }

    /// Reconstructs the operation-bearing P form for one matrix row.
    pub fn row_form(&self, row: usize) -> Option<RowForm> {
        self.row_operation(row)
            .map(|operation| self.source.apply(operation))
    }

    /// Reconstructs the operation-bearing I form for one matrix column.
    pub fn column_form(&self, column: usize) -> Option<RowForm> {
        self.column_operation(column)
            .map(|operation| self.source.apply(operation))
    }

    /// Returns all four convention-dependent edge-label collections.
    pub const fn edge_labels(&self) -> &RowMatrixEdgeLabels {
        &self.edge_labels
    }
}

const fn subtract_mod_twelve(left: u8, right: u8) -> u8 {
    (left + 12 - right) % 12
}
