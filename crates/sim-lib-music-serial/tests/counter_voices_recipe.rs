include!("../recipes/02-techniques/counter-voices/setup.rs");

#[test]
fn public_counter_voices_recipe_runs() {
    counter_voices().expect("counter voices scenario");
}
