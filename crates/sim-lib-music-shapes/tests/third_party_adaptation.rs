include!("../../sim-lib-music-serial/recipes/02-adaptation/third-party-adaptation/setup.rs");

#[test]
fn public_third_party_adaptation_recipe_runs() {
    third_party_adaptation().expect("third-party adaptation scenario");
}
