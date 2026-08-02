use sim_lib_music_core::{ObjectId, PitchClass};
use sim_lib_music_serial::{
    RowInstanceId, SerialPlan, SimultaneousFormsSpec, StructuralLicense, StructuralReadingId,
    TechniquePlan, simultaneous_forms, strict_aggregate,
};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

pub fn simultaneous_forms_recipe() -> Result<(), Box<dyn std::error::Error>> {
    let form = ToneRow::try_from_classes([
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
    ])?
    .apply(RowOperation::new(RowFamily::P, 0));
    let primary = RowInstanceId::new("row/forms/a")?;
    let partner = RowInstanceId::new("row/forms/b")?;
    let technique = TechniquePlan::builder("simultaneous-forms-recipe")?
        .rule(strict_aggregate())
        .deployer(simultaneous_forms(SimultaneousFormsSpec {
            row_ids: vec![primary.clone(), partner.clone()],
            voices: vec![ObjectId::new("voice/a")?, ObjectId::new("voice/b")?],
            block_size: 6,
            event_prefix: "event/forms".to_owned(),
            rationale: "combinatorial forms".to_owned(),
            license: StructuralLicense::new(
                StructuralReadingId::new("reading/forms")?,
                "simultaneous form recipe",
            )?,
        }))
        .build()?;
    let plan = technique.deploy([(primary, form.clone()), (partner, form)].into_iter().collect())?;

    assert_eq!(plan.events().len(), 4);
    assert_eq!(plan.simultaneous_groups().len(), 2);
    let reparsed = SerialPlan::try_new(
        plan.rows().clone(),
        plan.events().clone(),
        plan.precedence()
            .edges()
            .map(|(before, after)| (before.clone(), after.clone()))
            .collect::<Vec<_>>(),
    )?;
    assert_eq!(reparsed.events().len(), plan.events().len());
    Ok(())
}
