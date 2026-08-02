use sim_lib_pitch_serial::{BlockOrder, try_partition};

use crate::{RowInstanceId, SerialEventId, StructuralLicense, StructuralReadingId, VoiceId};

use super::components::{
    AggregateRotationSpec, InterlockingPartitionSpec, MelodyAccompanimentSpec,
    SimultaneousFormsSpec, VerticalBlocksSpec, aggregate_rotation, complete_horizontal_statement,
    interlocking_partition, melody_accompaniment_distribution, motivic_partition,
    simultaneous_forms, verticalize_selected_blocks,
};
use super::core::{SerialDeployError, TechniquePlan, strict_aggregate};

fn license(id: &str, rationale: &str) -> StructuralLicense {
    StructuralLicense::new(
        StructuralReadingId::new(id).expect("static reading id"),
        rationale,
    )
    .expect("static license")
}

fn voice(value: &str) -> VoiceId {
    VoiceId::new(value).expect("static voice")
}

/// Returns an inspectable Schoenberg-practice deployment assembled from public components.
pub fn schoenberg_partitioned() -> Result<TechniquePlan, SerialDeployError> {
    let trichords = try_partition(
        vec![vec![0, 1, 2], vec![3, 4, 5], vec![6, 7, 8], vec![9, 10, 11]],
        BlockOrder::total(),
    )
    .map_err(|error| SerialDeployError::Partition(error.to_string()))?;
    let tetrachords = try_partition(
        vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7], vec![8, 9, 10, 11]],
        BlockOrder::total(),
    )
    .map_err(|error| SerialDeployError::Partition(error.to_string()))?;
    let interlock_a = try_partition(
        vec![vec![0, 2, 4, 6], vec![1, 3, 8, 10], vec![5, 7, 9, 11]],
        BlockOrder::partially_ordered_blocks(),
    )
    .map_err(|error| SerialDeployError::Partition(error.to_string()))?;
    let interlock_b = try_partition(
        vec![vec![0, 1, 5, 9], vec![2, 3, 7, 11], vec![4, 6, 8, 10]],
        BlockOrder::partially_ordered_blocks(),
    )
    .map_err(|error| SerialDeployError::Partition(error.to_string()))?;

    TechniquePlan::builder("partitioned-row")?
        .rule(strict_aggregate())
        .deployer(complete_horizontal_statement(
            RowInstanceId::new("row/schoenberg/primary")
                .map_err(|error| SerialDeployError::Plan(error.to_string()))?,
            SerialEventId::new("event/schoenberg/statement")
                .map_err(|error| SerialDeployError::Plan(error.to_string()))?,
            voice("voice/statement"),
            "complete horizontal statement",
            license("reading/statement", "complete row statement"),
        ))
        .deployer(motivic_partition(
            RowInstanceId::new("row/schoenberg/partition")
                .map_err(|error| SerialDeployError::Plan(error.to_string()))?,
            trichords,
            vec![
                voice("voice/motive-a"),
                voice("voice/motive-b"),
                voice("voice/motive-c"),
                voice("voice/motive-d"),
            ],
            "event/schoenberg/motive",
            "trichord partition",
            license("reading/trichord", "trichord partition reading"),
        ))
        .deployer(verticalize_selected_blocks(VerticalBlocksSpec {
            row_id: RowInstanceId::new("row/schoenberg/vertical")
                .map_err(|error| SerialDeployError::Plan(error.to_string()))?,
            partition: tetrachords.clone(),
            selected_blocks: vec![0, 1, 2],
            voice: voice("voice/chords"),
            event_prefix: "event/schoenberg/chords".to_owned(),
            rationale: "tetrachordal chord blocks".to_owned(),
            license: license("reading/chords", "vertical tetrachord reading"),
        }))
        .deployer(interlocking_partition(InterlockingPartitionSpec {
            row_id: RowInstanceId::new("row/schoenberg/interlock")
                .map_err(|error| SerialDeployError::Plan(error.to_string()))?,
            partition: interlock_a,
            counter_partition: interlock_b,
            voices: vec![
                voice("voice/interlock-a"),
                voice("voice/interlock-b"),
                voice("voice/interlock-c"),
            ],
            event_prefix: "event/schoenberg/interlock".to_owned(),
            rationale: "interlocking partition exchange".to_owned(),
            license: license("reading/interlock", "interlocking partition witness"),
        }))
        .deployer(melody_accompaniment_distribution(MelodyAccompanimentSpec {
            row_id: RowInstanceId::new("row/schoenberg/melody")
                .map_err(|error| SerialDeployError::Plan(error.to_string()))?,
            partition: tetrachords.clone(),
            melody_voice: voice("voice/melody"),
            accompaniment_voice: voice("voice/accompaniment"),
            event_prefix: "event/schoenberg/melody".to_owned(),
            rationale: "melody and accompaniment split".to_owned(),
            license: license("reading/melody", "melody/accompaniment reading"),
        }))
        .deployer(aggregate_rotation(AggregateRotationSpec {
            row_id: RowInstanceId::new("row/schoenberg/rotation")
                .map_err(|error| SerialDeployError::Plan(error.to_string()))?,
            rotation: 4,
            block_lengths: vec![3, 3, 3, 3],
            voices: vec![
                voice("voice/rotation-a"),
                voice("voice/rotation-b"),
                voice("voice/rotation-c"),
                voice("voice/rotation-d"),
            ],
            event_prefix: "event/schoenberg/rotation".to_owned(),
            rationale: "rotated aggregate succession".to_owned(),
            license: license("reading/rotation", "aggregate rotation reading"),
        }))
        .deployer(simultaneous_forms(SimultaneousFormsSpec {
            row_ids: vec![
                RowInstanceId::new("row/schoenberg/primary")
                    .map_err(|error| SerialDeployError::Plan(error.to_string()))?,
                RowInstanceId::new("row/schoenberg/partner")
                    .map_err(|error| SerialDeployError::Plan(error.to_string()))?,
            ],
            voices: vec![voice("voice/form-a"), voice("voice/form-b")],
            block_size: 6,
            event_prefix: "event/schoenberg/simultaneous".to_owned(),
            rationale: "simultaneous combinatorial forms".to_owned(),
            license: license("reading/simultaneous", "simultaneous form reading"),
        }))
        .build()
}
