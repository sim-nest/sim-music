use sim_lib_music_core::{ObjectId, PitchClass};
use sim_lib_music_serial::{
    InvariantRequirement, RowInstanceId, StructuralLicense, StructuralReadingId,
    SymmetryRequirement, deploy_derived_cells, forms_with_invariant,
};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

fn op25_form() -> sim_lib_pitch_serial::RowForm {
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
    ])
    .expect("row");
    row.apply(RowOperation::new(RowFamily::P, 0))
}

fn webern_op24_row() -> ToneRow {
    ToneRow::try_from_classes([
        PitchClass::C,
        PitchClass::B,
        PitchClass::DS,
        PitchClass::E,
        PitchClass::GS,
        PitchClass::G,
        PitchClass::A,
        PitchClass::F,
        PitchClass::FS,
        PitchClass::CS,
        PitchClass::D,
        PitchClass::AS,
    ])
    .expect("Webern op. 24 row")
}

pub fn derived_invariant_practice() -> Result<(), Box<dyn std::error::Error>> {
    let row = webern_op24_row().apply(RowOperation::new(RowFamily::P, 0));
    let deployment = deploy_derived_cells(
        RowInstanceId::new("row/webern/op24")?,
        row,
        3,
        vec![
            ObjectId::new("voice/clarinet")?,
            ObjectId::new("voice/violin")?,
            ObjectId::new("voice/trumpet")?,
            ObjectId::new("voice/piano")?,
        ],
        "event/webern/derived",
        "Op. 24 trichordal derivation",
        StructuralLicense::new(
            StructuralReadingId::new("reading/webern-derived")?,
            "Webern Op. 24 derived cells",
        )?,
    )?;
    assert_eq!(deployment.generator_size, 3);
    assert_eq!(deployment.occurrences.len(), 4);

    let row = op25_form().into_row();
    let segment = row.segment(0, 3).expect("segment");
    let requirement = InvariantRequirement {
        preserve_source_ordinals: true,
        preserve_pitch_identity: false,
        require_transposition: true,
        require_inversion: false,
        preserve_interval_order: true,
        preserve_set_class: true,
        symmetry: SymmetryRequirement::None,
    };
    let candidates = forms_with_invariant(&row, &segment, requirement)?;
    assert!(candidates.unsatisfied().is_none());
    assert!(!candidates.as_slice().is_empty());
    Ok(())
}
