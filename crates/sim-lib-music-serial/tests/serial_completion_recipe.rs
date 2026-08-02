include!("../recipes/02-adaptation/serial-consonance-completion/setup.rs");

#[test]
fn public_serial_completion_recipe_runs() {
    serial_consonance_completion().expect("serial completion recipe");
}
