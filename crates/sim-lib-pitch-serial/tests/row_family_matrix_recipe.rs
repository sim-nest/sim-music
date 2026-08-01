include!("../recipes/01-basics/row-family-matrix/setup.rs");

#[test]
fn public_row_family_matrix_recipe_runs() {
    row_family_matrix().expect("row family matrix scenario");
}
