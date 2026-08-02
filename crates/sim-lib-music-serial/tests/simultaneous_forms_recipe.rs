include!("../recipes/02-techniques/simultaneous-forms/setup.rs");

#[test]
fn public_simultaneous_forms_recipe_runs() {
    simultaneous_forms_recipe().expect("simultaneous forms scenario");
}
