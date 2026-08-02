//! Open, Shape-ranked orchestration for modular music algorithms.

use std::sync::Arc;

use sim_kernel::{
    AbiVersion, Args, Callable, ClassRef, Cx, Error, Export, ExportKind, ExportRecord, ExportState,
    Expr, Lib, LibManifest, LibTarget, Linker, MatchScore, Object, ObjectCompat, Result, RuntimeId,
    ShapeRef, Symbol, Value, Version,
};
use sim_shape::{AnyShape, ExactExprShape, ListShape, shape_value};

const LIB_ID: &str = "music-algorithm-plan";
const STAGE_NAMESPACE: &str = "music/algorithm-stage";

/// Metadata every registered music-algorithm stage exposes through
/// [`ObjectCompat::as_table`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlgorithmStageMetadata {
    /// Stable implementation identity, distinct from the selectable stage name.
    pub implementation: Symbol,
    /// Stable cost category used by planners and browsing surfaces.
    pub cost_class: Symbol,
    /// Whether equal inputs and control data produce equal outputs.
    pub deterministic: bool,
    /// Source owner or checked implementation provenance.
    pub provenance: String,
}

impl AlgorithmStageMetadata {
    /// Creates complete stage metadata.
    pub fn new(
        implementation: Symbol,
        cost_class: Symbol,
        deterministic: bool,
        provenance: impl Into<String>,
    ) -> Self {
        Self {
            implementation,
            cost_class,
            deterministic,
            provenance: provenance.into(),
        }
    }

    /// Projects the metadata to the required runtime table contract.
    pub fn to_value(&self, cx: &mut Cx) -> Result<Value> {
        cx.factory().table(vec![
            (
                Symbol::new("implementation"),
                cx.factory().symbol(self.implementation.clone())?,
            ),
            (
                Symbol::new("cost-class"),
                cx.factory().symbol(self.cost_class.clone())?,
            ),
            (
                Symbol::new("deterministic"),
                cx.factory().bool(self.deterministic)?,
            ),
            (
                Symbol::new("provenance"),
                cx.factory().string(self.provenance.clone())?,
            ),
        ])
    }
}

/// Returns the open export kind used to register implementations of `stage`.
///
/// Multiple function symbols may share a stage kind. `music/algorithm-plan`
/// ranks their argument Shapes against the data request and rejects ties.
pub fn algorithm_stage_export_kind(stage: impl Into<String>) -> ExportKind {
    ExportKind::new(Symbol::qualified(STAGE_NAMESPACE, stage.into()))
}

/// Registers a loaded function as one implementation of a selectable stage.
///
/// Registration fails unless the function is callable, declares argument and
/// result Shapes, and exposes the complete [`AlgorithmStageMetadata`] table.
pub fn register_algorithm_stage(
    cx: &mut Cx,
    lib: &Symbol,
    stage: impl Into<String>,
    function: Symbol,
) -> Result<()> {
    let stage = stage.into();
    let value = cx
        .registry()
        .function_by_symbol(&function)
        .cloned()
        .ok_or_else(|| {
            Error::Lib(format!(
                "algorithm stage {stage} missing function {function}"
            ))
        })?;
    validate_stage_function(cx, &stage, &function, &value)?;
    cx.registry_mut().append_export_record(
        lib,
        ExportRecord {
            kind: algorithm_stage_export_kind(stage),
            symbol: function,
            state: ExportState::Resolved {
                id: RuntimeId::Value,
            },
        },
    )
}

/// Symbol of the Shape-ranked foundry application callable.
pub fn music_algorithm_plan_symbol() -> Symbol {
    Symbol::qualified("music", "algorithm-plan")
}

/// Loadable runtime application that executes data-only algorithm plans.
pub struct MusicAlgorithmPlanLib;

impl Lib for MusicAlgorithmPlanLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(LIB_ID),
            version: Version(env!("CARGO_PKG_VERSION").to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: vec![Export::Function {
                symbol: music_algorithm_plan_symbol(),
                function_id: None,
            }],
        }
    }

    fn load(&self, cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        linker.function_value(
            music_algorithm_plan_symbol(),
            cx.factory().opaque(Arc::new(AlgorithmPlanFunction))?,
        )?;
        Ok(())
    }
}

/// Installs [`MusicAlgorithmPlanLib`] into `cx` once.
pub fn install_music_algorithm_plan_lib(cx: &mut Cx) -> Result<()> {
    sim_lib_core::install_once(cx, &MusicAlgorithmPlanLib).map(|_| ())
}

struct AlgorithmPlanFunction;

impl Object for AlgorithmPlanFunction {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok("#<function music/algorithm-plan>".to_owned())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for AlgorithmPlanFunction {
    fn class(&self, cx: &mut Cx) -> Result<ClassRef> {
        cx.factory().class_stub(
            sim_kernel::CORE_FUNCTION_CLASS_ID,
            Symbol::qualified("core", "Function"),
        )
    }

    fn as_callable(&self) -> Option<&dyn Callable> {
        Some(self)
    }
}

impl Callable for AlgorithmPlanFunction {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        execute_plan(cx, args.into_vec())
    }

    fn browse_args_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("music/algorithm-plan", "args"),
            Arc::new(ListShape::tuple(vec![
                keyword_shape("input"),
                Arc::new(AnyShape),
                keyword_shape("stages"),
                Arc::new(AnyShape),
                keyword_shape("control"),
                Arc::new(AnyShape),
            ])),
        )))
    }

    fn browse_result_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("music/algorithm-plan", "result"),
            Arc::new(AnyShape),
        )))
    }
}

#[derive(Clone, Debug)]
struct StageRequest {
    name: String,
    options: Vec<Expr>,
}

fn execute_plan(cx: &mut Cx, args: Vec<Value>) -> Result<Value> {
    let [input_key, input, stages_key, stages, control_key, control] = args.as_slice() else {
        return Err(Error::Eval(
            "music/algorithm-plan expects :input VALUE :stages LIST :control TABLE".to_owned(),
        ));
    };
    expect_keyword_value(cx, input_key, "input")?;
    expect_keyword_value(cx, stages_key, "stages")?;
    expect_keyword_value(cx, control_key, "control")?;
    validate_control(cx, control)?;
    let requests = parse_stage_requests(cx, stages)?;
    if requests.is_empty() {
        return Err(Error::Eval(
            "music/algorithm-plan requires at least one stage".to_owned(),
        ));
    }

    let mut current = input.clone();
    let mut intermediate_values = Vec::with_capacity(requests.len());
    let mut trace_values = Vec::with_capacity(requests.len());
    for request in requests {
        let mut stage_args = vec![current.clone()];
        stage_args.extend(
            request
                .options
                .iter()
                .cloned()
                .map(|expr| cx.factory().expr(expr))
                .collect::<Result<Vec<_>>>()?,
        );
        stage_args.push(cx.factory().symbol(Symbol::new(":control"))?);
        stage_args.push(control.clone());
        let selected = select_stage(cx, &request.name, &stage_args)?;
        let output = selected.call(cx, Args::new(stage_args))?;
        check_stage_result(cx, &selected, output.clone())?;
        trace_values.push(stage_trace(cx, &request.name, &selected)?);
        intermediate_values.push(output.clone());
        current = output;
    }

    cx.factory().table(vec![
        (
            Symbol::new("kind"),
            cx.factory()
                .symbol(Symbol::qualified("music", "AlgorithmResult"))?,
        ),
        (Symbol::new("input"), input.clone()),
        (Symbol::new("control"), control.clone()),
        (Symbol::new("trace"), cx.factory().list(trace_values)?),
        (
            Symbol::new("intermediates"),
            cx.factory().list(intermediate_values)?,
        ),
        (Symbol::new("value"), current),
    ])
}

struct SelectedStage {
    symbol: Symbol,
    callable_value: Value,
    args_score: MatchScore,
    result_shape: ShapeRef,
}

impl SelectedStage {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        self.callable_value
            .object()
            .as_callable()
            .ok_or(Error::TypeMismatch {
                expected: "callable function",
                found: "non-callable",
            })?
            .call(cx, args)
    }
}

fn select_stage(cx: &mut Cx, name: &str, args: &[Value]) -> Result<SelectedStage> {
    let symbols = cx
        .registry()
        .export_symbols()
        .get(&algorithm_stage_export_kind(name.to_owned()))
        .map(|records| records.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    if symbols.is_empty() {
        return Err(Error::Lib(format!(
            "music algorithm stage {name} is not loaded; load a library exporting {STAGE_NAMESPACE}/{name}"
        )));
    }

    let argument_list = cx.factory().list(args.to_vec())?;
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for symbol in symbols {
        let Some(value) = cx.registry().function_by_symbol(&symbol).cloned() else {
            rejected.push(format!("{symbol}: registered value is not a function"));
            continue;
        };
        let Some(callable) = value.object().as_callable() else {
            rejected.push(format!("{symbol}: function has no Callable view"));
            continue;
        };
        let Some(args_shape) = callable.browse_args_shape(cx)? else {
            rejected.push(format!("{symbol}: missing argument Shape"));
            continue;
        };
        let Some(shape) = args_shape.object().as_shape() else {
            rejected.push(format!("{symbol}: argument Shape value is invalid"));
            continue;
        };
        let matched = shape.check_value(cx, argument_list.clone())?;
        if !matched.accepted {
            let details = matched
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            rejected.push(format!("{symbol}: {details}"));
            continue;
        }
        let Some(result_shape) = callable.browse_result_shape(cx)? else {
            rejected.push(format!("{symbol}: missing result Shape"));
            continue;
        };
        accepted.push(SelectedStage {
            symbol,
            callable_value: value,
            args_score: matched.score,
            result_shape,
        });
    }
    accepted.sort_by(|left, right| {
        right
            .args_score
            .cmp(&left.args_score)
            .then_with(|| left.symbol.cmp(&right.symbol))
    });
    let Some(best) = accepted.first() else {
        return Err(Error::Eval(format!(
            "music algorithm stage {name} has no implementation whose argument Shape accepts the request: {}",
            rejected.join(" | ")
        )));
    };
    if accepted
        .get(1)
        .is_some_and(|next| next.args_score == best.args_score)
    {
        return Err(Error::Eval(format!(
            "music algorithm stage {name} is ambiguous at Shape score {} between {} and {}",
            best.args_score.value(),
            best.symbol,
            accepted[1].symbol
        )));
    }
    Ok(accepted.remove(0))
}

fn check_stage_result(cx: &mut Cx, selected: &SelectedStage, output: Value) -> Result<()> {
    let Some(shape) = selected.result_shape.object().as_shape() else {
        return Err(Error::Eval(format!(
            "music algorithm stage {} returned an invalid result Shape",
            selected.symbol
        )));
    };
    let matched = shape.check_value(cx, output)?;
    if matched.accepted {
        Ok(())
    } else {
        Err(Error::Eval(format!(
            "music algorithm stage {} result failed Shape check: {}",
            selected.symbol,
            matched
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        )))
    }
}

fn stage_trace(cx: &mut Cx, name: &str, selected: &SelectedStage) -> Result<Value> {
    let metadata = selected.callable_value.object().as_table(cx)?;
    cx.factory().table(vec![
        (
            Symbol::new("stage"),
            cx.factory().symbol(Symbol::new(name))?,
        ),
        (
            Symbol::new("implementation"),
            cx.factory().symbol(selected.symbol.clone())?,
        ),
        (
            Symbol::new("shape-score"),
            cx.factory()
                .string(selected.args_score.value().to_string())?,
        ),
        (Symbol::new("metadata"), metadata),
    ])
}

fn validate_stage_function(cx: &mut Cx, stage: &str, symbol: &Symbol, value: &Value) -> Result<()> {
    let callable = value.object().as_callable().ok_or_else(|| {
        Error::Lib(format!(
            "algorithm stage {stage} function {symbol} is not callable"
        ))
    })?;
    if callable.browse_args_shape(cx)?.is_none() {
        return Err(Error::Lib(format!(
            "algorithm stage {stage} function {symbol} is missing its argument Shape"
        )));
    }
    if callable.browse_result_shape(cx)?.is_none() {
        return Err(Error::Lib(format!(
            "algorithm stage {stage} function {symbol} is missing its result Shape"
        )));
    }
    let metadata = value.object().as_table(cx)?;
    let Some(table) = metadata.object().as_table_impl() else {
        return Err(Error::Lib(format!(
            "algorithm stage {stage} function {symbol} metadata is not a Table"
        )));
    };
    for field in [
        "implementation",
        "cost-class",
        "deterministic",
        "provenance",
    ] {
        if !table.has(cx, Symbol::new(field))? {
            return Err(Error::Lib(format!(
                "algorithm stage {stage} function {symbol} metadata is missing {field}"
            )));
        }
    }
    Ok(())
}

fn parse_stage_requests(cx: &mut Cx, value: &Value) -> Result<Vec<StageRequest>> {
    let expr = unquote(value.object().as_expr(cx)?);
    let (Expr::List(items) | Expr::Vector(items)) = expr else {
        return Err(Error::TypeMismatch {
            expected: "algorithm stage list",
            found: "non-list",
        });
    };
    items
        .into_iter()
        .map(|item| {
            let item = unquote(item);
            let (Expr::List(mut fields) | Expr::Vector(mut fields)) = item else {
                return Err(Error::TypeMismatch {
                    expected: "algorithm stage request list",
                    found: "non-list",
                });
            };
            if fields.is_empty() {
                return Err(Error::Eval(
                    "algorithm stage request cannot be empty".to_owned(),
                ));
            }
            let head = fields.remove(0);
            let Expr::Symbol(name) = unquote(head) else {
                return Err(Error::TypeMismatch {
                    expected: "algorithm stage name symbol",
                    found: "non-symbol",
                });
            };
            if !fields.len().is_multiple_of(2) {
                return Err(Error::Eval(format!(
                    "algorithm stage {} options must be key/value pairs",
                    name
                )));
            }
            Ok(StageRequest {
                name: name.name.to_string(),
                options: fields.into_iter().map(unquote).collect(),
            })
        })
        .collect()
}

fn validate_control(cx: &mut Cx, value: &Value) -> Result<()> {
    let Some(table) = value.object().as_table_impl() else {
        return Err(Error::TypeMismatch {
            expected: "algorithm control Table",
            found: "non-table",
        });
    };
    for field in ["work", "results", "seed"] {
        let field_symbol = Symbol::new(field);
        if !table.has(cx, field_symbol.clone())? {
            return Err(Error::Eval(format!(
                "music algorithm control is missing :{field}"
            )));
        }
        let value = table.get(cx, field_symbol)?;
        let text = match value.object().as_expr(cx)? {
            Expr::Number(number) => number.canonical,
            Expr::String(text) => text,
            _ => {
                return Err(Error::Eval(format!(
                    "music algorithm control :{field} must be a non-negative integer"
                )));
            }
        };
        text.parse::<u64>().map_err(|_| {
            Error::Eval(format!(
                "music algorithm control :{field} must be a non-negative integer"
            ))
        })?;
    }
    Ok(())
}

fn expect_keyword_value(cx: &mut Cx, value: &Value, expected: &str) -> Result<()> {
    let Expr::Symbol(symbol) = value.object().as_expr(cx)? else {
        return Err(Error::TypeMismatch {
            expected: "keyword symbol",
            found: "non-symbol",
        });
    };
    if symbol.name.trim_start_matches(':') == expected {
        Ok(())
    } else {
        Err(Error::Eval(format!(
            "music/algorithm-plan expected :{expected}, got {symbol}"
        )))
    }
}

fn unquote(expr: Expr) -> Expr {
    match expr {
        Expr::Quote { expr, .. } => *expr,
        other => other,
    }
}

fn keyword_shape(name: &'static str) -> Arc<dyn sim_shape::Shape> {
    Arc::new(ExactExprShape::new(Expr::Symbol(Symbol::new(format!(
        ":{name}"
    )))))
}

#[cfg(test)]
mod tests;
