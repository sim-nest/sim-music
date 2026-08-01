include!("../recipes/02-adaptation/modal-spine/setup.rs");

#[test]
fn public_modal_spine_recipe_runs() {
    modal_spine().expect("modal spine scenario");
}
