//! Inspectable serial deployment plans composed from reusable components.

mod components;
mod core;
mod schoenberg;

pub use components::{
    AggregateRotationSpec, InterlockingPartitionSpec, MelodyAccompanimentSpec,
    SimultaneousFormsSpec, VerticalBlocksSpec, complete_horizontal_statement,
    interlocking_partition, melody_accompaniment_distribution, motivic_partition,
    simultaneous_forms, verticalize_selected_blocks,
};
pub use core::{
    SerialDeployError, SerialDeployer, SerialDeployerKind, SerialDeployerParameter,
    SerialDeployerSpec, TechniquePlan, TechniquePlanBuilder, strict_aggregate,
};
pub use schoenberg::schoenberg_partitioned;
