include!("../recipes/01-basics/strict-row-realization/setup.rs");

#[test]
fn public_strict_row_realization_recipe_runs() {
    strict_row_realization().expect("strict row realization scenario");
}
