include!("../recipes/01-basics/certified-transforms/setup.rs");

#[test]
fn public_certified_transform_recipe_runs() {
    certified_transform_scenario().expect("certified transform scenario");
}
