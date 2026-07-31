use super::*;

#[test]
fn deterministic_pipeline_checks_intermediates_and_final_frames() {
    let (mut cx, result) = run_plan("layered-dp").unwrap();
    assert_fixture(&mut cx, &result, "layered-dp").unwrap();
}

#[test]
fn alternate_registered_harmonizer_is_selected_from_data_alone() {
    let (mut cx, result) = run_plan("recursive-exhaustive").unwrap();
    assert_fixture(&mut cx, &result, "recursive-exhaustive").unwrap();
    let trace = result
        .object()
        .as_table_impl()
        .unwrap()
        .get(&mut cx, Symbol::new("trace"))
        .unwrap()
        .object()
        .as_expr(&mut cx)
        .unwrap();
    assert!(format!("{trace:?}").contains("harmonize-recursive-exhaustive"));
}

#[test]
fn missing_optional_preview_reports_load_and_shape_contract() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_music_algorithm_plan_lib(&mut cx).unwrap();
    let preview = optional_preview_provider();
    let input = cx.factory().table(Vec::new()).unwrap();
    let control = cx
        .factory()
        .table(vec![
            (Symbol::new("work"), number_value(&cx, 1).unwrap()),
            (Symbol::new("results"), number_value(&cx, 1).unwrap()),
            (Symbol::new("seed"), number_value(&cx, 0).unwrap()),
        ])
        .unwrap();
    let stages = cx
        .factory()
        .expr(Expr::List(vec![Expr::List(vec![
            Expr::Symbol(Symbol::new("preview")),
            Expr::Symbol(Symbol::new(":target")),
            Expr::Symbol(Symbol::new("surface-session")),
        ])]))
        .unwrap();
    let plan = cx
        .registry()
        .function_by_symbol(&music_algorithm_plan_symbol())
        .unwrap()
        .clone();
    let plan_args = Args::new(vec![
        cx.factory().symbol(Symbol::new(":input")).unwrap(),
        input,
        cx.factory().symbol(Symbol::new(":stages")).unwrap(),
        stages,
        cx.factory().symbol(Symbol::new(":control")).unwrap(),
        control,
    ]);
    let missing = plan
        .object()
        .as_callable()
        .unwrap()
        .call(&mut cx, plan_args)
        .unwrap_err();
    assert!(missing.to_string().contains("stage preview is not loaded"));
    assert!(
        missing
            .to_string()
            .contains("music/algorithm-stage/preview")
    );
    let args = vec![
        cx.factory().table(Vec::new()).unwrap(),
        cx.factory().symbol(Symbol::new(":target")).unwrap(),
        cx.factory().symbol(Symbol::new("surface-session")).unwrap(),
        cx.factory().symbol(Symbol::new(":control")).unwrap(),
        cx.factory().table(Vec::new()).unwrap(),
    ];
    let kind = sim_lib_music_analysis::algorithm_stage_export_kind("preview");
    assert!(cx.registry().export_symbols().get(&kind).is_none());
    install_provider(&mut cx, &preview).unwrap();
    let record = cx
        .registry()
        .export_symbols()
        .get(&kind)
        .and_then(|records| records.get(&preview.symbol));
    assert!(record.is_some());
    let callable_value = cx
        .registry()
        .function_by_symbol(&preview.symbol)
        .unwrap()
        .clone();
    let callable = callable_value.object().as_callable().unwrap();
    let shape = callable.browse_args_shape(&mut cx).unwrap().unwrap();
    let args = cx.factory().list(args).unwrap();
    let matched = shape
        .object()
        .as_shape()
        .unwrap()
        .check_value(&mut cx, args)
        .unwrap();
    assert!(matched.accepted);
    assert!(matched.score > sim_kernel::MatchScore::reject());
}
