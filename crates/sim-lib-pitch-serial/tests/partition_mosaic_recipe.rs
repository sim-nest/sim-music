include!("../recipes/02-partitions/partition-mosaic/setup.rs");

#[test]
fn public_partition_mosaic_recipe_runs() {
    partition_mosaic().expect("partition mosaic scenario");
}
