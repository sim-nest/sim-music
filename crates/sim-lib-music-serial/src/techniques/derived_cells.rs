use std::collections::BTreeMap;

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{DerivationCellRelation, DerivationMatch, RowForm};

use crate::{
    EventPlacement, PlannedSerialEvent, RowInstanceId, SerialDeployError, SerialEventId,
    SerialOrigin, SerialPlan, SerialRole, StructuralLicense, VoiceId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DerivedCellPlanOccurrence {
    pub(crate) event_id: SerialEventId,
    pub(crate) voice: VoiceId,
    pub(crate) occurrence_index: usize,
    pub(crate) source_ordinals: Vec<u8>,
    pub(crate) generator_ordinals: Vec<u8>,
    pub(crate) generator_classes: Vec<PitchClass>,
    pub(crate) operation: sim_lib_pitch_serial::RowOperation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DerivedCellPlan {
    pub(crate) plan: SerialPlan,
    pub(crate) derivation: DerivationMatch,
    pub(crate) occurrences: Vec<DerivedCellPlanOccurrence>,
}

pub(crate) fn build_derived_cell_plan(
    row_id: RowInstanceId,
    row_form: RowForm,
    derivation: DerivationMatch,
    voices: Vec<VoiceId>,
    event_prefix: &str,
    rationale: &str,
    license: StructuralLicense,
) -> Result<DerivedCellPlan, SerialDeployError> {
    if derivation.cells.len() != voices.len() {
        return Err(SerialDeployError::VoiceCountMismatch {
            deployer: "derived-cells".to_owned(),
            expected: derivation.cells.len(),
            actual: voices.len(),
        });
    }

    let generator = generator_classes(&row_form, &derivation.cells[0]);
    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), row_form);

    let mut events = BTreeMap::new();
    let mut precedence = Vec::new();
    let mut previous = None::<SerialEventId>;
    let mut occurrences = Vec::with_capacity(derivation.cells.len());

    for (occurrence_index, (cell, voice)) in derivation.cells.iter().zip(voices).enumerate() {
        let event_id = SerialEventId::new(format!("{event_prefix}/occurrence-{occurrence_index}"))
            .map_err(|error| SerialDeployError::Plan(error.to_string()))?;
        let event = PlannedSerialEvent {
            id: event_id.clone(),
            ordinals: cell
                .ordinals
                .iter()
                .copied()
                .map(|ordinal| crate::OrdinalRef::new(row_id.clone(), usize::from(ordinal)))
                .collect(),
            role: SerialRole::Structural,
            origin: SerialOrigin::Structural {
                rationale: format!(
                    "{rationale}; derived cell {occurrence_index} via {}",
                    cell.operation
                ),
            },
            voice: voice.clone(),
            placement: EventPlacement::independent(),
            parents: Vec::new(),
            licenses: vec![license.clone()],
        };
        events.insert(event_id.clone(), event);
        if let Some(previous_id) = previous.as_ref() {
            precedence.push((previous_id.clone(), event_id.clone()));
        }
        previous = Some(event_id.clone());
        occurrences.push(DerivedCellPlanOccurrence {
            event_id,
            voice,
            occurrence_index,
            source_ordinals: cell.ordinals.clone(),
            generator_ordinals: derivation.cells[0].ordinals.clone(),
            generator_classes: generator.clone(),
            operation: cell.operation,
        });
    }

    Ok(DerivedCellPlan {
        plan: SerialPlan::try_new(rows, events, precedence)
            .map_err(|error| SerialDeployError::Plan(error.to_string()))?,
        derivation,
        occurrences,
    })
}

fn generator_classes(row_form: &RowForm, cell: &DerivationCellRelation) -> Vec<PitchClass> {
    cell.ordinals
        .iter()
        .map(|ordinal| row_form.classes()[usize::from(*ordinal)])
        .collect()
}
