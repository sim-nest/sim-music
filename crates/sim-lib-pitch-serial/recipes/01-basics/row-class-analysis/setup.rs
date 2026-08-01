use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{DerivationKind, RowFamily, RowOperation, ToneRow, analyze_row_class};

pub fn row_class_analysis() -> Result<(), Box<dyn std::error::Error>> {
    let row = ToneRow::try_from_classes([
        PitchClass::C,
        PitchClass::CS,
        PitchClass::D,
        PitchClass::DS,
        PitchClass::E,
        PitchClass::F,
        PitchClass::FS,
        PitchClass::G,
        PitchClass::B,
        PitchClass::AS,
        PitchClass::A,
        PitchClass::GS,
    ])?;
    let report = analyze_row_class(&row);

    assert_eq!(report.derivation.generator_size, Some(4));
    assert!(
        report
            .derivation
            .matches
            .iter()
            .any(|entry| entry.kind == DerivationKind::Tetrachordal && entry.generator_size == 4)
    );
    assert!(
        report
            .combinatoriality
            .iter()
            .all(|partner| { partner.source.union(partner.complement).count_bits() == 12 })
    );
    assert!(
        report
            .combinatoriality
            .iter()
            .any(|partner| partner.operation == RowOperation::new(RowFamily::P, 0))
    );
    Ok(())
}
