use super::*;
use sim_kernel::{DefaultFactory, EagerPolicy, NumberLiteral, Shape, ShapeDoc, ShapeMatch};

struct FixtureStageLib {
    id: &'static str,
    stage: &'static str,
    function: Symbol,
    strategy: &'static str,
}

impl Lib for FixtureStageLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(self.id),
            version: Version("0.1.0".to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: vec![Export::Function {
                symbol: self.function.clone(),
                function_id: None,
            }],
        }
    }

    fn load(&self, cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        linker.function_value(
            self.function.clone(),
            cx.factory().opaque(Arc::new(FixtureStage {
                function: self.function.clone(),
                strategy: self.strategy,
            }))?,
        )?;
        Ok(())
    }
}

struct FixtureStage {
    function: Symbol,
    strategy: &'static str,
}

impl Object for FixtureStage {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok(format!("#<function {}>", self.function))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for FixtureStage {
    fn class(&self, cx: &mut Cx) -> Result<ClassRef> {
        cx.factory().class_stub(
            sim_kernel::CORE_FUNCTION_CLASS_ID,
            Symbol::qualified("core", "Function"),
        )
    }

    fn as_callable(&self) -> Option<&dyn Callable> {
        Some(self)
    }

    fn as_table(&self, cx: &mut Cx) -> Result<Value> {
        AlgorithmStageMetadata::new(
            self.function.clone(),
            Symbol::new("bounded"),
            true,
            "fixture",
        )
        .to_value(cx)
    }
}

impl Callable for FixtureStage {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        let input = args.values()[0].object().as_expr(cx)?;
        cx.factory().expr(Expr::Map(vec![
            (
                Expr::Symbol(Symbol::new("strategy")),
                Expr::Symbol(Symbol::new(self.strategy)),
            ),
            (Expr::Symbol(Symbol::new("input")), input),
        ]))
    }

    fn browse_args_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("fixture", format!("{}-args", self.strategy)),
            Arc::new(ListShape::tuple(vec![
                Arc::new(AnyShape),
                keyword_shape("strategy"),
                Arc::new(ExactExprShape::new(Expr::Symbol(Symbol::new(
                    self.strategy,
                )))),
                keyword_shape("control"),
                Arc::new(AnyShape),
            ])),
        )))
    }

    fn browse_result_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("fixture", "result"),
            Arc::new(MapShape),
        )))
    }
}

struct MapShape;

impl Shape for MapShape {
    fn check_value(&self, cx: &mut Cx, value: Value) -> Result<ShapeMatch> {
        if matches!(value.object().as_expr(cx)?, Expr::Map(_)) {
            Ok(ShapeMatch::accept(MatchScore::exact(10)))
        } else {
            Ok(ShapeMatch::reject("expected map"))
        }
    }

    fn check_expr(&self, _cx: &mut Cx, expr: &Expr) -> Result<ShapeMatch> {
        if matches!(expr, Expr::Map(_)) {
            Ok(ShapeMatch::accept(MatchScore::exact(10)))
        } else {
            Ok(ShapeMatch::reject("expected map"))
        }
    }

    fn describe(&self, _cx: &mut Cx) -> Result<ShapeDoc> {
        Ok(ShapeDoc::new("map"))
    }
}

fn number(value: u64) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("number", "integer"),
        canonical: value.to_string(),
    })
}

fn install_fixture(cx: &mut Cx, id: &'static str, strategy: &'static str) {
    let function = Symbol::qualified("fixture", strategy);
    let lib = FixtureStageLib {
        id,
        stage: "harmonize",
        function: function.clone(),
        strategy,
    };
    cx.load_lib(&lib).unwrap();
    register_algorithm_stage(cx, &Symbol::new(id), lib.stage, function).unwrap();
}

#[test]
fn data_only_strategy_selects_shape_ranked_registered_stage() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_music_algorithm_plan_lib(&mut cx).unwrap();
    install_fixture(&mut cx, "fixture-layered", "layered-dp");
    install_fixture(&mut cx, "fixture-greedy", "greedy");
    let callable = cx
        .registry()
        .function_by_symbol(&music_algorithm_plan_symbol())
        .unwrap()
        .clone();
    let control = cx
        .factory()
        .table(vec![
            (Symbol::new("work"), cx.factory().expr(number(50)).unwrap()),
            (
                Symbol::new("results"),
                cx.factory().expr(number(2)).unwrap(),
            ),
            (Symbol::new("seed"), cx.factory().expr(number(42)).unwrap()),
        ])
        .unwrap();
    let stages = cx
        .factory()
        .expr(Expr::List(vec![Expr::List(vec![
            Expr::Symbol(Symbol::new("harmonize")),
            Expr::Symbol(Symbol::new(":strategy")),
            Expr::Symbol(Symbol::new("greedy")),
        ])]))
        .unwrap();
    let input = cx.factory().string("fixture".to_owned()).unwrap();
    let call_args = Args::new(vec![
        cx.factory().symbol(Symbol::new(":input")).unwrap(),
        input,
        cx.factory().symbol(Symbol::new(":stages")).unwrap(),
        stages,
        cx.factory().symbol(Symbol::new(":control")).unwrap(),
        control,
    ]);
    let result = callable
        .object()
        .as_callable()
        .unwrap()
        .call(&mut cx, call_args)
        .unwrap();
    let result = result.object().as_table_impl().unwrap();
    let trace = result.get(&mut cx, Symbol::new("trace")).unwrap();
    assert!(trace.object().display(&mut cx).unwrap().contains('1'));
    let value = result.get(&mut cx, Symbol::new("value")).unwrap();
    let Expr::Map(fields) = value.object().as_expr(&mut cx).unwrap() else {
        panic!("map result")
    };
    assert!(fields.iter().any(|(key, value)| {
        matches!(key, Expr::Symbol(symbol) if symbol.name.as_ref() == "strategy")
            && matches!(value, Expr::Symbol(symbol) if symbol.name.as_ref() == "greedy")
    }));
}

#[test]
fn missing_stage_and_shape_mismatch_fail_without_fallback() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_music_algorithm_plan_lib(&mut cx).unwrap();
    let missing = select_stage(&mut cx, "preview", &[])
        .err()
        .expect("preview should be missing");
    assert!(missing.to_string().contains("is not loaded"));

    install_fixture(&mut cx, "fixture-layered", "layered-dp");
    let control = cx.factory().table(Vec::new()).unwrap();
    let args = vec![
        cx.factory().string("input".to_owned()).unwrap(),
        cx.factory().symbol(Symbol::new(":strategy")).unwrap(),
        cx.factory().symbol(Symbol::new("unknown")).unwrap(),
        cx.factory().symbol(Symbol::new(":control")).unwrap(),
        control,
    ];
    let mismatch = select_stage(&mut cx, "harmonize", &args)
        .err()
        .expect("unknown strategy should fail its Shape");
    assert!(mismatch.to_string().contains("argument Shape"));
    assert!(mismatch.to_string().contains("fixture/layered-dp"));
}
