include!("../recipes/03-parameters/integral-parameters/setup.rs");

#[test]
fn public_integral_parameters_recipe_runs() {
    integral_parameters().expect("integral parameters scenario");
}
