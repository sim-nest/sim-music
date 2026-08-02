include!("../recipes/02-techniques/referential-subsets/setup.rs");

#[test]
fn public_referential_subsets_recipe_runs() {
    referential_subsets().expect("referential subsets scenario");
}
