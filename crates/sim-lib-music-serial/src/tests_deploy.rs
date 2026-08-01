use std::collections::{BTreeMap, BTreeSet};

use sim_lib_music_core::{Articulation, Channel, ObjectId, PitchClass, Time};
use sim_lib_pitch_serial::{BlockOrder, RowFamily, RowOperation, ToneRow, try_partition};

use crate::{
    SerialDeployerKind, SimultaneousFormsSpec, StrictEventSpec, StrictRealizationContext,
    StructuralLicense, StructuralReadingId, TechniquePlan, VerticalBlocksSpec, realize_strict,
    schoenberg_partitioned, simultaneous_forms, strict_aggregate, verticalize_selected_blocks,
};

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

fn voice(name: &str) -> ObjectId {
    ObjectId::new(name).expect("voice id")
}

fn quarter() -> Time {
    Time::new(1, 4)
}

fn license(id: &str, rationale: &str) -> StructuralLicense {
    StructuralLicense::new(StructuralReadingId::new(id).expect("reading id"), rationale)
        .expect("license")
}

fn schoenberg_rows() -> BTreeMap<crate::RowInstanceId, sim_lib_pitch_serial::RowForm> {
    [
        "row/schoenberg/primary",
        "row/schoenberg/partner",
        "row/schoenberg/partition",
        "row/schoenberg/vertical",
        "row/schoenberg/interlock",
        "row/schoenberg/melody",
        "row/schoenberg/rotation",
    ]
    .into_iter()
    .map(|id| (crate::RowInstanceId::new(id).expect("row id"), op25_form()))
    .collect()
}

#[test]
fn schoenberg_partitioned_is_inspectable_and_preserves_structural_coverage() {
    let technique = schoenberg_partitioned().expect("technique");
    let specs = technique.deployer_specs();
    assert_eq!(specs.len(), 7);
    assert_eq!(
        specs[0].kind,
        SerialDeployerKind::CompleteHorizontalStatement
    );
    assert_eq!(specs[1].kind, SerialDeployerKind::MotivicPartition);
    assert_eq!(specs[2].kind, SerialDeployerKind::VerticalBlocks);
    assert_eq!(specs[6].kind, SerialDeployerKind::SimultaneousForms);

    let plan = technique.deploy(schoenberg_rows()).expect("plan");
    assert_eq!(plan.rows().len(), 7);
    assert!(plan.simultaneous_groups().len() >= 2);
    for row_id in plan.rows().keys() {
        let covered = plan
            .events()
            .values()
            .filter(|event| {
                event
                    .ordinals
                    .iter()
                    .any(|ordinal| &ordinal.row_id == row_id)
            })
            .flat_map(|event| {
                event
                    .ordinals
                    .iter()
                    .filter(move |ordinal| &ordinal.row_id == row_id)
                    .map(|ordinal| ordinal.ordinal)
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(covered, (0usize..12).collect());
    }
}

#[test]
fn vertical_blocks_realize_tetrachords_as_chordal_events() {
    let tetrachords = try_partition(
        vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7], vec![8, 9, 10, 11]],
        BlockOrder::total(),
    )
    .expect("partition");
    let row_id = crate::RowInstanceId::new("row/vertical/test").expect("row id");
    let technique = TechniquePlan::builder("vertical-test")
        .expect("builder")
        .rule(strict_aggregate())
        .deployer(verticalize_selected_blocks(VerticalBlocksSpec {
            row_id: row_id.clone(),
            partition: tetrachords,
            selected_blocks: vec![0, 1, 2],
            voice: voice("voice/chordal"),
            event_prefix: "event/vertical/test".to_owned(),
            rationale: "tetrachord blocks".to_owned(),
            license: license("reading/chordal", "tetrachord chord reading"),
        }))
        .build()
        .expect("technique");
    let plan = technique
        .deploy([(row_id, op25_form())].into_iter().collect())
        .expect("plan");
    let events = plan.events().values().collect::<Vec<_>>();
    assert_eq!(events.len(), 3);
    assert!(events.iter().all(|event| event.ordinals.len() == 4));

    let channel = Channel::new(0).expect("channel");
    let specs = events
        .iter()
        .map(|event| {
            (
                event.id.clone(),
                StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Marcato),
            )
        })
        .collect();
    let realization =
        realize_strict(&plan, &StrictRealizationContext::new(specs)).expect("realization");
    assert_eq!(
        realization
            .notes()
            .iter()
            .filter(|note| note.event_id == events[0].id)
            .count(),
        4
    );
}

#[test]
fn simultaneous_forms_preserve_each_form_identity() {
    let primary = crate::RowInstanceId::new("row/forms/a").expect("row id");
    let partner = crate::RowInstanceId::new("row/forms/b").expect("row id");
    let technique = TechniquePlan::builder("simultaneous-forms-test")
        .expect("builder")
        .rule(strict_aggregate())
        .deployer(simultaneous_forms(SimultaneousFormsSpec {
            row_ids: vec![primary.clone(), partner.clone()],
            voices: vec![voice("voice/a"), voice("voice/b")],
            block_size: 6,
            event_prefix: "event/forms".to_owned(),
            rationale: "combinatorial forms".to_owned(),
            license: license("reading/forms", "simultaneous form reading"),
        }))
        .build()
        .expect("technique");
    let plan = technique
        .deploy(
            [(primary, op25_form()), (partner, op25_form())]
                .into_iter()
                .collect(),
        )
        .expect("plan");
    assert_eq!(plan.simultaneous_groups().len(), 2);
    for members in plan.simultaneous_groups().values() {
        assert_eq!(members.len(), 2);
        let row_ids = members
            .iter()
            .map(|event| event.ordinals[0].row_id.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(row_ids.len(), 2);
    }
}

#[test]
fn realized_notes_keep_structural_license_reports() {
    let technique = schoenberg_partitioned().expect("technique");
    let plan = technique.deploy(schoenberg_rows()).expect("plan");
    let channel = Channel::new(0).expect("channel");
    let specs = plan
        .events()
        .values()
        .map(|event| {
            (
                event.id.clone(),
                StrictEventSpec::notes(4, quarter(), 88, channel, Articulation::Normal),
            )
        })
        .collect();
    let realization =
        realize_strict(&plan, &StrictRealizationContext::new(specs)).expect("realization");
    assert!(
        realization
            .notes()
            .iter()
            .all(|note| !note.origin.licenses.is_empty())
    );
    assert!(realization.notes().iter().any(|note| {
        note.origin
            .licenses
            .iter()
            .any(|item| item.reading_id.as_str() == "reading/simultaneous")
    }));
}
