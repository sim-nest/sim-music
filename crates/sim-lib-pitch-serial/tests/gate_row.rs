use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{
    MatrixCoordinate, PitchClassAlphabet, ROW_MATRIX_SIZE, RowError, RowFamily, RowFamilySet,
    RowLabel, RowLabelConvention, RowMatrix, RowOperation, ToneRow,
};
use sim_lib_serial_core::SeriesError;

const CHROMATIC: [PitchClass; 12] = [
    PitchClass::C,
    PitchClass::CS,
    PitchClass::D,
    PitchClass::DS,
    PitchClass::E,
    PitchClass::F,
    PitchClass::FS,
    PitchClass::G,
    PitchClass::GS,
    PitchClass::A,
    PitchClass::AS,
    PitchClass::B,
];

const OP_25: [PitchClass; 12] = [
    PitchClass::E,
    PitchClass::F,
    PitchClass::G,
    PitchClass::CS,
    PitchClass::FS,
    PitchClass::DS,
    PitchClass::GS,
    PitchClass::D,
    PitchClass::B,
    PitchClass::C,
    PitchClass::A,
    PitchClass::AS,
];

fn values(classes: &[PitchClass; 12]) -> [u8; 12] {
    classes.map(PitchClass::value)
}

#[test]
fn gate_row_canonical_alphabet_and_malformed_aggregates() {
    let alphabet = PitchClassAlphabet::try_new().expect("canonical pitch-class alphabet");
    assert_eq!(
        alphabet
            .classes()
            .iter()
            .copied()
            .map(PitchClass::value)
            .collect::<Vec<_>>(),
        (0..12).collect::<Vec<_>>()
    );

    let row = ToneRow::try_from_classes(CHROMATIC).expect("chromatic row");
    assert_eq!(row.classes(), &CHROMATIC);

    let malformed = [PitchClass::C; 12];
    assert!(matches!(
        ToneRow::try_from_classes(malformed),
        Err(RowError::Aggregate(SeriesError::MultiplicityMismatch {
            alphabet_position: 0,
            expected: 1,
            found: 12,
        }))
    ));
}

#[test]
fn gate_row_total_p_i_r_ri_for_zero_and_nonzero_starts() {
    let zero = ToneRow::try_from_classes(CHROMATIC).expect("zero-starting row");
    assert_eq!(
        values(zero.apply(RowOperation::new(RowFamily::P, 5)).classes()),
        [5, 6, 7, 8, 9, 10, 11, 0, 1, 2, 3, 4]
    );
    assert_eq!(
        values(zero.apply(RowOperation::new(RowFamily::I, 5)).classes()),
        [5, 4, 3, 2, 1, 0, 11, 10, 9, 8, 7, 6]
    );
    assert_eq!(
        values(zero.apply(RowOperation::new(RowFamily::R, 5)).classes()),
        [4, 3, 2, 1, 0, 11, 10, 9, 8, 7, 6, 5]
    );
    assert_eq!(
        values(zero.apply(RowOperation::new(RowFamily::RI, 5)).classes()),
        [6, 7, 8, 9, 10, 11, 0, 1, 2, 3, 4, 5]
    );

    let nonzero = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    assert_eq!(
        values(nonzero.classes()),
        [4, 5, 7, 1, 6, 3, 8, 2, 11, 0, 9, 10]
    );
    assert_eq!(
        nonzero
            .apply(RowOperation {
                family: RowFamily::P,
                addend: 255,
            })
            .operation(),
        RowOperation::new(RowFamily::P, 3)
    );
}

#[test]
fn gate_row_operations_obey_inverse_laws() {
    let source = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    for family in [RowFamily::P, RowFamily::I, RowFamily::R, RowFamily::RI] {
        for addend in 0..12 {
            let operation = RowOperation::new(family, addend);
            let transformed = source.apply(operation);
            let restored = transformed.row().apply(operation.inverse());
            assert_eq!(
                restored.classes(),
                source.classes(),
                "inverse law failed for {operation}"
            );
        }
    }
}

#[test]
fn gate_row_operation_identity_and_labels_remain_distinct() {
    let source = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    for family in [RowFamily::P, RowFamily::I, RowFamily::R, RowFamily::RI] {
        let form = source.apply(RowOperation::new(family, 0));
        assert_eq!(form.operation(), RowOperation::new(family, 0));
        assert_eq!(
            form.label(RowLabelConvention::OperationIndex),
            RowLabel::new(family, 0)
        );
        let expected_pitch_label = match family {
            RowFamily::P | RowFamily::R => 4,
            RowFamily::I | RowFamily::RI => 8,
        };
        assert_eq!(
            form.label(RowLabelConvention::FirstLastPitch),
            RowLabel::new(family, expected_pitch_label)
        );
    }

    let retrograde = source.apply(RowOperation::new(RowFamily::R, 7));
    assert_eq!(retrograde.classes()[0], PitchClass::F);
    assert_eq!(retrograde.classes()[11], PitchClass::B);
    assert_eq!(
        retrograde
            .label(RowLabelConvention::FirstLastPitch)
            .to_string(),
        "R11"
    );
    assert_eq!(
        retrograde
            .label(RowLabelConvention::OperationIndex)
            .to_string(),
        "R7"
    );
}

#[test]
fn gate_row_family_retains_every_alias_and_deduplicates_values() {
    let row = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    let family = RowFamilySet::of(&row);

    assert_eq!(family.source(), &row);
    assert_eq!(family.aliases().len(), 48);
    for alias in family.aliases() {
        assert_eq!(alias.form, row.apply(alias.operation));
        assert_eq!(
            alias.form.row(),
            &family.distinct_forms()[alias.distinct_form_index()]
        );
    }
    assert!(family.distinct_forms().len() <= 48);

    for (index, distinct) in family.distinct_forms().iter().enumerate() {
        let aliases = family.aliases_for_distinct_form(index).collect::<Vec<_>>();
        assert!(!aliases.is_empty());
        assert!(aliases.iter().all(|alias| alias.form.row() == distinct));
    }
}

#[test]
fn gate_symmetric_row_collapses_forms_without_losing_aliases() {
    let row = ToneRow::try_from_classes(CHROMATIC).expect("chromatic row");
    let family = RowFamilySet::of(&row);

    assert_eq!(family.aliases().len(), 48);
    assert_eq!(family.distinct_forms().len(), 24);
    assert!(
        family
            .distinct_forms()
            .iter()
            .enumerate()
            .all(|(index, _)| family.aliases_for_distinct_form(index).count() == 2)
    );
    for alias in family.aliases() {
        assert_eq!(alias.form, row.apply(alias.operation));
    }
}

#[test]
fn gate_matrix_rows_columns_and_reverse_edges_match_operations() {
    let source = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    for convention in [
        RowLabelConvention::FirstLastPitch,
        RowLabelConvention::OperationIndex,
    ] {
        let matrix = RowMatrix::new(&source, convention);
        assert_eq!(matrix.source(), &source);
        assert_eq!(matrix.convention(), convention);

        for row in 0..ROW_MATRIX_SIZE {
            let operation = matrix.row_operation(row).expect("matrix row");
            let form = source.apply(operation);
            assert_eq!(matrix.row(row), Some(form.classes()));
            assert_eq!(matrix.row_form(row), Some(form.clone()));
            assert_eq!(matrix.edge_labels().left()[row], form.label(convention));

            let reverse = source.apply(RowOperation::new(RowFamily::R, operation.addend));
            let matrix_row = matrix.row(row).expect("matrix row");
            assert_eq!(
                std::array::from_fn(|index| matrix_row[ROW_MATRIX_SIZE - 1 - index]),
                *reverse.classes()
            );
            assert_eq!(matrix.edge_labels().right()[row], reverse.label(convention));
        }

        for column in 0..ROW_MATRIX_SIZE {
            let operation = matrix.column_operation(column).expect("matrix column");
            let form = source.apply(operation);
            assert_eq!(matrix.column(column), Some(*form.classes()));
            assert_eq!(matrix.column_form(column), Some(form.clone()));
            assert_eq!(matrix.edge_labels().top()[column], form.label(convention));

            let reverse = source.apply(RowOperation::new(RowFamily::RI, operation.addend));
            let matrix_column = matrix.column(column).expect("matrix column");
            assert_eq!(
                std::array::from_fn(|index| matrix_column[ROW_MATRIX_SIZE - 1 - index]),
                *reverse.classes()
            );
            assert_eq!(
                matrix.edge_labels().bottom()[column],
                reverse.label(convention)
            );
        }
    }
}

#[test]
fn gate_matrix_ascii_and_data_share_coordinates_and_semantics() {
    let source = ToneRow::try_from_classes(OP_25).expect("Op. 25 row");
    let matrix = RowMatrix::new(&source, RowLabelConvention::FirstLastPitch);
    let data = matrix.render_data();

    assert_eq!(data.source(), &source);
    assert_eq!(data.convention(), matrix.convention());
    assert_eq!(data.edge_labels(), matrix.edge_labels());
    assert_eq!(data.cells().len(), ROW_MATRIX_SIZE * ROW_MATRIX_SIZE);
    for row in 0..ROW_MATRIX_SIZE {
        for column in 0..ROW_MATRIX_SIZE {
            let coordinate = MatrixCoordinate::new(row, column).expect("matrix coordinate");
            let cell = data.cell(coordinate);
            assert_eq!(cell.coordinate(), coordinate);
            assert_eq!(cell, &matrix.cell(coordinate));
        }
    }

    assert!(MatrixCoordinate::new(ROW_MATRIX_SIZE, 0).is_none());
    assert!(MatrixCoordinate::new(0, ROW_MATRIX_SIZE).is_none());
    let ascii = matrix.render_ascii();
    assert!(ascii.starts_with("label-convention: first-last-pitch\nsource:  4  5"));
    assert!(ascii.contains("P4 |"));
    assert!(ascii.lines().last().is_some_and(|line| line.contains("RI")));
}
