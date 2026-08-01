use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{
    ROW_MATRIX_SIZE, RowFamilySet, RowLabelConvention, RowMatrix, ToneRow,
};

pub fn row_family_matrix() -> Result<(), Box<dyn std::error::Error>> {
    let row = ToneRow::try_from_classes([
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
    ])?;

    let family = RowFamilySet::of(&row);
    assert_eq!(family.aliases().len(), 48);
    assert!(family.distinct_forms().len() <= family.aliases().len());
    for alias in family.aliases() {
        assert_eq!(alias.form, row.apply(alias.operation));
    }

    let matrix = RowMatrix::new(&row, RowLabelConvention::FirstLastPitch);
    let data = matrix.render_data();
    assert_eq!(data.cells().len(), ROW_MATRIX_SIZE * ROW_MATRIX_SIZE);
    assert_eq!(data.source(), &row);
    assert!(matrix.render_ascii().contains("label-convention: first-last-pitch"));
    Ok(())
}
