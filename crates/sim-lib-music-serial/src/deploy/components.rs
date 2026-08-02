use std::collections::BTreeMap;

use sim_lib_pitch_serial::{
    RowForm, RowPartition, analyze_combinatoriality_partition, analyze_interlocking_partitions,
};

use crate::{
    EventPlacement, RowInstanceId, SerialEventId, SimultaneousGroupId, StructuralLicense, VoiceId,
};

use super::core::{
    PlanBuilder, SerialDeployError, SerialDeployer, SerialDeployerInner, require_row,
    require_voices, structural_event,
};

/// Complete horizontal statement deployment for one row.
#[derive(Clone)]
pub struct HorizontalStatementSpec {
    /// Source row to present.
    pub row_id: RowInstanceId,
    /// Stable event id for the emitted statement.
    pub event_id: SerialEventId,
    /// Voice receiving the statement.
    pub voice: VoiceId,
    /// Human-facing structural rationale.
    pub rationale: String,
    /// Structural reading that licenses the statement.
    pub license: StructuralLicense,
}

impl HorizontalStatementSpec {
    pub(crate) fn apply(
        &self,
        rows: &BTreeMap<RowInstanceId, RowForm>,
        builder: &mut PlanBuilder,
    ) -> Result<(), SerialDeployError> {
        require_row("complete-horizontal-statement", rows, &self.row_id)?;
        builder.add_event(
            "complete-horizontal-statement",
            structural_event(
                self.event_id.clone(),
                &(0..12).map(|ordinal| ordinal as u8).collect::<Vec<_>>(),
                self.row_id.clone(),
                self.voice.clone(),
                self.rationale.clone(),
                self.license.clone(),
                EventPlacement::independent(),
            ),
        )
    }
}

/// Creates one complete horizontal row statement deployer.
pub fn complete_horizontal_statement(
    row_id: RowInstanceId,
    event_id: SerialEventId,
    voice: VoiceId,
    rationale: impl Into<String>,
    license: StructuralLicense,
) -> SerialDeployer {
    SerialDeployer {
        inner: SerialDeployerInner::Horizontal(HorizontalStatementSpec {
            row_id,
            event_id,
            voice,
            rationale: rationale.into(),
            license,
        }),
    }
}

/// Sequential motivic partition deployment over one row.
#[derive(Clone)]
pub struct MotivicPartitionSpec {
    /// Source row to partition.
    pub row_id: RowInstanceId,
    /// Validated partition to emit.
    pub partition: RowPartition,
    /// One voice per partition block.
    pub voices: Vec<VoiceId>,
    /// Stable event id prefix.
    pub event_prefix: String,
    /// Human-facing structural rationale.
    pub rationale: String,
    /// Structural reading that licenses the partition.
    pub license: StructuralLicense,
}

impl MotivicPartitionSpec {
    pub(crate) fn apply(
        &self,
        rows: &BTreeMap<RowInstanceId, RowForm>,
        builder: &mut PlanBuilder,
    ) -> Result<(), SerialDeployError> {
        require_row("motivic-partition", rows, &self.row_id)?;
        require_voices(
            "motivic-partition",
            self.partition.block_count(),
            self.voices.len(),
        )?;
        let mut previous = None::<SerialEventId>;
        for (index, (block, voice)) in self.partition.blocks().iter().zip(&self.voices).enumerate()
        {
            let event_id = SerialEventId::new(format!("{}/block-{}", self.event_prefix, index))
                .map_err(|error| SerialDeployError::Plan(error.to_string()))?;
            builder.add_event(
                "motivic-partition",
                structural_event(
                    event_id.clone(),
                    block.ordinals(),
                    self.row_id.clone(),
                    voice.clone(),
                    self.rationale.clone(),
                    self.license.clone(),
                    EventPlacement::independent(),
                ),
            )?;
            if let Some(previous_id) = previous.as_ref() {
                builder.add_precedence(previous_id, &event_id);
            }
            previous = Some(event_id);
        }
        Ok(())
    }
}

/// Creates one sequential motivic partition deployer.
pub fn motivic_partition(
    row_id: RowInstanceId,
    partition: RowPartition,
    voices: Vec<VoiceId>,
    event_prefix: impl Into<String>,
    rationale: impl Into<String>,
    license: StructuralLicense,
) -> SerialDeployer {
    SerialDeployer {
        inner: SerialDeployerInner::Motivic(MotivicPartitionSpec {
            row_id,
            partition,
            voices,
            event_prefix: event_prefix.into(),
            rationale: rationale.into(),
            license,
        }),
    }
}

/// Chordal vertical-block deployment over one row.
#[derive(Clone)]
pub struct VerticalBlocksSpec {
    /// Source row to partition.
    pub row_id: RowInstanceId,
    /// Validated partition supplying vertical blocks.
    pub partition: RowPartition,
    /// Block indices that should sound as chordal events.
    pub selected_blocks: Vec<usize>,
    /// Voice receiving the chordal material.
    pub voice: VoiceId,
    /// Stable event id prefix.
    pub event_prefix: String,
    /// Human-facing structural rationale.
    pub rationale: String,
    /// Structural reading that licenses the vertical reading.
    pub license: StructuralLicense,
}

impl VerticalBlocksSpec {
    pub(crate) fn apply(
        &self,
        rows: &BTreeMap<RowInstanceId, RowForm>,
        builder: &mut PlanBuilder,
    ) -> Result<(), SerialDeployError> {
        require_row("vertical-blocks", rows, &self.row_id)?;
        let mut previous = None::<SerialEventId>;
        for (position, block_index) in self.selected_blocks.iter().copied().enumerate() {
            let block = self
                .partition
                .blocks()
                .get(block_index)
                .expect("validated block index selected by caller");
            let event_id =
                SerialEventId::new(format!("{}/vertical-{}", self.event_prefix, position))
                    .map_err(|error| SerialDeployError::Plan(error.to_string()))?;
            builder.add_event(
                "vertical-blocks",
                structural_event(
                    event_id.clone(),
                    block.ordinals(),
                    self.row_id.clone(),
                    self.voice.clone(),
                    self.rationale.clone(),
                    self.license.clone(),
                    EventPlacement::independent(),
                ),
            )?;
            if let Some(previous_id) = previous.as_ref() {
                builder.add_precedence(previous_id, &event_id);
            }
            previous = Some(event_id);
        }
        Ok(())
    }
}

/// Creates one chordal vertical-block deployer.
pub fn verticalize_selected_blocks(spec: VerticalBlocksSpec) -> SerialDeployer {
    SerialDeployer {
        inner: SerialDeployerInner::Vertical(spec),
    }
}

/// Interlocking partition deployment over one row.
#[derive(Clone)]
pub struct InterlockingPartitionSpec {
    /// Source row to partition.
    pub row_id: RowInstanceId,
    /// Primary partition to emit.
    pub partition: RowPartition,
    /// Counter-partition used as the interlocking witness.
    pub counter_partition: RowPartition,
    /// One voice per emitted block.
    pub voices: Vec<VoiceId>,
    /// Stable event id prefix.
    pub event_prefix: String,
    /// Human-facing structural rationale.
    pub rationale: String,
    /// Structural reading that licenses the interlocking reading.
    pub license: StructuralLicense,
}

impl InterlockingPartitionSpec {
    pub(crate) fn apply(
        &self,
        rows: &BTreeMap<RowInstanceId, RowForm>,
        builder: &mut PlanBuilder,
    ) -> Result<(), SerialDeployError> {
        require_row("interlocking-partition", rows, &self.row_id)?;
        require_voices(
            "interlocking-partition",
            self.partition.block_count(),
            self.voices.len(),
        )?;
        let report = analyze_interlocking_partitions(&self.partition, &self.counter_partition);
        if !report.is_interlocking {
            return Err(SerialDeployError::NotInterlocking {
                deployer: "interlocking-partition".to_owned(),
            });
        }
        let mut previous = None::<SerialEventId>;
        for (index, (block, voice)) in self.partition.blocks().iter().zip(&self.voices).enumerate()
        {
            let event_id = SerialEventId::new(format!("{}/exchange-{}", self.event_prefix, index))
                .map_err(|error| SerialDeployError::Plan(error.to_string()))?;
            builder.add_event(
                "interlocking-partition",
                structural_event(
                    event_id.clone(),
                    block.ordinals(),
                    self.row_id.clone(),
                    voice.clone(),
                    format!("{}; interlocking witness", self.rationale),
                    self.license.clone(),
                    EventPlacement::independent(),
                ),
            )?;
            if let Some(previous_id) = previous.as_ref() {
                builder.add_precedence(previous_id, &event_id);
            }
            previous = Some(event_id);
        }
        Ok(())
    }
}

/// Creates one interlocking partition deployer.
pub fn interlocking_partition(spec: InterlockingPartitionSpec) -> SerialDeployer {
    SerialDeployer {
        inner: SerialDeployerInner::Interlocking(spec),
    }
}

/// Melody/accompaniment distribution deployment over one row.
#[derive(Clone)]
pub struct MelodyAccompanimentSpec {
    /// Source row to partition.
    pub row_id: RowInstanceId,
    /// Validated partition supplying blocks.
    pub partition: RowPartition,
    /// Lead melody voice.
    pub melody_voice: VoiceId,
    /// Accompaniment voice.
    pub accompaniment_voice: VoiceId,
    /// Stable event id prefix.
    pub event_prefix: String,
    /// Human-facing structural rationale.
    pub rationale: String,
    /// Structural reading that licenses the split.
    pub license: StructuralLicense,
}

impl MelodyAccompanimentSpec {
    pub(crate) fn apply(
        &self,
        rows: &BTreeMap<RowInstanceId, RowForm>,
        builder: &mut PlanBuilder,
    ) -> Result<(), SerialDeployError> {
        require_row("melody-accompaniment", rows, &self.row_id)?;
        let mut previous = None::<SerialEventId>;
        for (index, block) in self.partition.blocks().iter().enumerate() {
            let melody_id = SerialEventId::new(format!("{}/melody-{}", self.event_prefix, index))
                .map_err(|error| SerialDeployError::Plan(error.to_string()))?;
            builder.add_event(
                "melody-accompaniment",
                structural_event(
                    melody_id.clone(),
                    &[block.ordinals()[0]],
                    self.row_id.clone(),
                    self.melody_voice.clone(),
                    self.rationale.clone(),
                    self.license.clone(),
                    EventPlacement::independent(),
                ),
            )?;
            if let Some(previous_id) = previous.as_ref() {
                builder.add_precedence(previous_id, &melody_id);
            }
            previous = Some(melody_id.clone());
            if block.ordinals().len() > 1 {
                let accompaniment_id =
                    SerialEventId::new(format!("{}/accompaniment-{}", self.event_prefix, index))
                        .map_err(|error| SerialDeployError::Plan(error.to_string()))?;
                builder.add_event(
                    "melody-accompaniment",
                    structural_event(
                        accompaniment_id.clone(),
                        &block.ordinals()[1..],
                        self.row_id.clone(),
                        self.accompaniment_voice.clone(),
                        format!("{}; accompaniment residue", self.rationale),
                        self.license.clone(),
                        EventPlacement::independent(),
                    ),
                )?;
                builder.add_precedence(&melody_id, &accompaniment_id);
                previous = Some(accompaniment_id);
            }
        }
        Ok(())
    }
}

/// Creates one melody/accompaniment deployer.
pub fn melody_accompaniment_distribution(spec: MelodyAccompanimentSpec) -> SerialDeployer {
    SerialDeployer {
        inner: SerialDeployerInner::MelodyAccompaniment(spec),
    }
}

/// Aggregate-rotation deployment over one row.
#[derive(Clone)]
pub struct AggregateRotationSpec {
    /// Source row to rotate.
    pub row_id: RowInstanceId,
    /// Rotation offset in row ordinals.
    pub rotation: usize,
    /// Block lengths after rotation.
    pub block_lengths: Vec<usize>,
    /// One voice per rotated block.
    pub voices: Vec<VoiceId>,
    /// Stable event id prefix.
    pub event_prefix: String,
    /// Human-facing structural rationale.
    pub rationale: String,
    /// Structural reading that licenses the rotation.
    pub license: StructuralLicense,
}

impl AggregateRotationSpec {
    pub(crate) fn apply(
        &self,
        rows: &BTreeMap<RowInstanceId, RowForm>,
        builder: &mut PlanBuilder,
    ) -> Result<(), SerialDeployError> {
        require_row("aggregate-rotation", rows, &self.row_id)?;
        require_voices(
            "aggregate-rotation",
            self.block_lengths.len(),
            self.voices.len(),
        )?;
        let total: usize = self.block_lengths.iter().sum();
        if total != 12 {
            return Err(SerialDeployError::InvalidRotationCoverage(total));
        }
        let ordinals = (0..12)
            .map(|offset| ((self.rotation + offset) % 12) as u8)
            .collect::<Vec<_>>();
        let mut start = 0usize;
        let mut previous = None::<SerialEventId>;
        for (index, (len, voice)) in self
            .block_lengths
            .iter()
            .copied()
            .zip(&self.voices)
            .enumerate()
        {
            let event_id = SerialEventId::new(format!("{}/rotation-{}", self.event_prefix, index))
                .map_err(|error| SerialDeployError::Plan(error.to_string()))?;
            builder.add_event(
                "aggregate-rotation",
                structural_event(
                    event_id.clone(),
                    &ordinals[start..start + len],
                    self.row_id.clone(),
                    voice.clone(),
                    self.rationale.clone(),
                    self.license.clone(),
                    EventPlacement::independent(),
                ),
            )?;
            if let Some(previous_id) = previous.as_ref() {
                builder.add_precedence(previous_id, &event_id);
            }
            previous = Some(event_id);
            start += len;
        }
        Ok(())
    }
}

/// Creates one aggregate-rotation deployer.
#[allow(clippy::missing_const_for_fn)]
pub fn aggregate_rotation(spec: AggregateRotationSpec) -> SerialDeployer {
    SerialDeployer {
        inner: SerialDeployerInner::AggregateRotation(spec),
    }
}

/// Simultaneous multi-form deployment aligned by contiguous blocks.
#[derive(Clone)]
pub struct SimultaneousFormsSpec {
    /// Row ids to align simultaneously.
    pub row_ids: Vec<RowInstanceId>,
    /// One voice per row form.
    pub voices: Vec<VoiceId>,
    /// Supported contiguous block size.
    pub block_size: usize,
    /// Stable event id prefix.
    pub event_prefix: String,
    /// Human-facing structural rationale.
    pub rationale: String,
    /// Structural reading that licenses the simultaneity.
    pub license: StructuralLicense,
}

impl SimultaneousFormsSpec {
    pub(crate) fn apply(
        &self,
        rows: &BTreeMap<RowInstanceId, RowForm>,
        builder: &mut PlanBuilder,
    ) -> Result<(), SerialDeployError> {
        require_voices("simultaneous-forms", self.row_ids.len(), self.voices.len())?;
        for row_id in &self.row_ids {
            require_row("simultaneous-forms", rows, row_id)?;
        }
        let source_id = self.row_ids.first().expect("at least one row");
        let source_row = rows.get(source_id).expect("validated row").row().clone();
        for partner_id in self.row_ids.iter().skip(1) {
            let partner = rows.get(partner_id).expect("validated row");
            let witness = analyze_combinatoriality_partition(
                &source_row,
                partner.operation(),
                self.block_size,
            )
            .map_err(|error| SerialDeployError::Partition(error.to_string()))?;
            let Some(partner_witness) = witness else {
                return Err(SerialDeployError::NotCombinatorial {
                    source_row_id: source_id.clone(),
                    partner_row_id: partner_id.clone(),
                    block_size: self.block_size,
                });
            };
            if partner_witness.operation != partner.operation() {
                return Err(SerialDeployError::NotCombinatorial {
                    source_row_id: source_id.clone(),
                    partner_row_id: partner_id.clone(),
                    block_size: self.block_size,
                });
            }
        }
        let block_count = 12 / self.block_size;
        let mut previous_group = Vec::<SerialEventId>::new();
        for block_index in 0..block_count {
            let group =
                SimultaneousGroupId::new(format!("{}/simul-{}", self.event_prefix, block_index))
                    .map_err(|error| SerialDeployError::Plan(error.to_string()))?;
            let mut current_group = Vec::new();
            for (row_id, voice) in self.row_ids.iter().zip(&self.voices) {
                let event_id = SerialEventId::new(format!(
                    "{}/form-{}/block-{}",
                    self.event_prefix,
                    row_id.as_str().replace('/', "_"),
                    block_index
                ))
                .map_err(|error| SerialDeployError::Plan(error.to_string()))?;
                let start = block_index * self.block_size;
                let ordinals = (start..start + self.block_size)
                    .map(|ordinal| ordinal as u8)
                    .collect::<Vec<_>>();
                builder.add_event(
                    "simultaneous-forms",
                    structural_event(
                        event_id.clone(),
                        &ordinals,
                        row_id.clone(),
                        voice.clone(),
                        self.rationale.clone(),
                        self.license.clone(),
                        EventPlacement::simultaneous(group.clone()),
                    ),
                )?;
                current_group.push(event_id);
            }
            for previous_id in &previous_group {
                for current_id in &current_group {
                    builder.add_precedence(previous_id, current_id);
                }
            }
            previous_group = current_group;
        }
        Ok(())
    }
}

/// Creates one simultaneous-forms deployer.
pub fn simultaneous_forms(spec: SimultaneousFormsSpec) -> SerialDeployer {
    SerialDeployer {
        inner: SerialDeployerInner::SimultaneousForms(spec),
    }
}
