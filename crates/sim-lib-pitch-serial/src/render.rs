//! Structured and ASCII projections of a row matrix.

use std::fmt::Write;

use crate::{
    MatrixCoordinate, ROW_MATRIX_SIZE, RowLabelConvention, RowMatrix, RowMatrixCell,
    RowMatrixEdgeLabels, RowOperation, ToneRow,
};

/// A complete structured projection of a [`RowMatrix`].
///
/// Cells are row-major and retain explicit coordinates. Source identity,
/// operations, edge labels, and the selected convention travel with the data,
/// so a consumer never has to infer matrix semantics from display text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowMatrixData {
    source: ToneRow,
    convention: RowLabelConvention,
    cells: Vec<RowMatrixCell>,
    row_operations: [RowOperation; ROW_MATRIX_SIZE],
    column_operations: [RowOperation; ROW_MATRIX_SIZE],
    edge_labels: RowMatrixEdgeLabels,
}

impl RowMatrixData {
    /// Returns the matrix's explicit source row.
    pub const fn source(&self) -> &ToneRow {
        &self.source
    }

    /// Returns the convention used to derive the edge labels.
    pub const fn convention(&self) -> RowLabelConvention {
        self.convention
    }

    /// Returns the 144 coordinate-bearing cells in row-major order.
    pub fn cells(&self) -> &[RowMatrixCell] {
        &self.cells
    }

    /// Returns a cell by its validated coordinate.
    pub fn cell(&self, coordinate: MatrixCoordinate) -> &RowMatrixCell {
        &self.cells[coordinate.row() * ROW_MATRIX_SIZE + coordinate.column()]
    }

    /// Returns the P operations for rows ordered top to bottom.
    pub const fn row_operations(&self) -> &[RowOperation; ROW_MATRIX_SIZE] {
        &self.row_operations
    }

    /// Returns the I operations for columns ordered left to right.
    pub const fn column_operations(&self) -> &[RowOperation; ROW_MATRIX_SIZE] {
        &self.column_operations
    }

    /// Returns all labels on the top, right, bottom, and left edges.
    pub const fn edge_labels(&self) -> &RowMatrixEdgeLabels {
        &self.edge_labels
    }
}

impl RowMatrix {
    /// Projects this matrix to structured, coordinate-bearing data.
    pub fn render_data(&self) -> RowMatrixData {
        let cells = (0..ROW_MATRIX_SIZE)
            .flat_map(|row| {
                (0..ROW_MATRIX_SIZE).map(move |column| {
                    let coordinate = MatrixCoordinate::new(row, column)
                        .expect("fixed matrix iteration always yields a valid coordinate");
                    self.cell(coordinate)
                })
            })
            .collect();
        let row_operations = std::array::from_fn(|row| {
            self.row_operation(row)
                .expect("fixed matrix iteration always yields a row operation")
        });
        let column_operations = std::array::from_fn(|column| {
            self.column_operation(column)
                .expect("fixed matrix iteration always yields a column operation")
        });

        RowMatrixData {
            source: self.source().clone(),
            convention: self.convention(),
            cells,
            row_operations,
            column_operations,
            edge_labels: *self.edge_labels(),
        }
    }

    /// Renders a self-describing ASCII matrix from the structured projection.
    pub fn render_ascii(&self) -> String {
        let data = self.render_data();
        let mut output = String::new();
        writeln!(output, "label-convention: {}", data.convention().as_str())
            .expect("writing to a String cannot fail");
        write!(output, "source:").expect("writing to a String cannot fail");
        for class in data.source().classes() {
            write!(output, " {:>2}", class.value()).expect("writing to a String cannot fail");
        }
        output.push('\n');

        output.push_str("     ");
        for label in data.edge_labels().top() {
            write!(output, " {:>4}", label).expect("writing to a String cannot fail");
        }
        output.push('\n');

        for row in 0..ROW_MATRIX_SIZE {
            write!(output, "{:>4} |", data.edge_labels().left()[row])
                .expect("writing to a String cannot fail");
            for column in 0..ROW_MATRIX_SIZE {
                let coordinate = MatrixCoordinate::new(row, column)
                    .expect("fixed matrix iteration always yields a valid coordinate");
                write!(output, " {:>4}", data.cell(coordinate).class().value())
                    .expect("writing to a String cannot fail");
            }
            writeln!(output, " | {:<4}", data.edge_labels().right()[row])
                .expect("writing to a String cannot fail");
        }

        output.push_str("     ");
        for label in data.edge_labels().bottom() {
            write!(output, " {:>4}", label).expect("writing to a String cannot fail");
        }
        output.push('\n');
        output
    }
}
