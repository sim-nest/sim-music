use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{
    PitchClassAlphabet, RowError, RowFamily, RowLabel, RowLabelConvention, RowOperation, ToneRow,
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
