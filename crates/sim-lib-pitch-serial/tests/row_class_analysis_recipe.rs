include!("../recipes/01-basics/row-class-analysis/setup.rs");

#[test]
fn public_row_class_analysis_recipe_runs() {
    row_class_analysis().expect("row class analysis scenario");
}
