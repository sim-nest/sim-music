use std::sync::Arc;

use sim_kernel::{
    Args, Callable, ClassRef, Cx, Error, Expr, Linker, Object, ObjectCompat, RawArgs, Result,
    ShapeRef, Symbol, Value,
};
use sim_lib_pitch_serial::{RowLabelConvention, RowMatrix, analyze_row_class};
use sim_shape::{AnyShape, ExactExprShape, ListShape, shape_value};

use crate::{decode_tone_row, encode_tone_row};

pub(crate) fn load_serial_functions(
    cx: &mut sim_kernel::LoadCx,
    linker: &mut Linker<'_>,
) -> Result<()> {
    linker.function_value(
        serial_row_symbol(),
        cx.factory().opaque(Arc::new(SerialRowFunction))?,
    )?;
    linker.function_value(
        serial_matrix_symbol(),
        cx.factory().opaque(Arc::new(SerialMatrixFunction))?,
    )?;
    linker.function_value(
        serial_analyze_row_symbol(),
        cx.factory().opaque(Arc::new(SerialAnalyzeRowFunction))?,
    )?;
    Ok(())
}

pub(crate) fn serial_row_symbol() -> Symbol {
    Symbol::qualified("serial", "row")
}

pub(crate) fn serial_matrix_symbol() -> Symbol {
    Symbol::qualified("serial", "matrix")
}

pub(crate) fn serial_analyze_row_symbol() -> Symbol {
    Symbol::qualified("serial", "analyze-row")
}

struct SerialRowFunction;

impl Object for SerialRowFunction {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok("#<function serial/row>".to_owned())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for SerialRowFunction {
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

impl Callable for SerialRowFunction {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        let exprs = args
            .into_vec()
            .into_iter()
            .map(|value| value.object().as_expr(cx))
            .collect::<Result<Vec<_>>>()?;
        serial_row_call(cx, &exprs, false)
    }

    fn call_exprs(&self, cx: &mut Cx, args: RawArgs) -> Result<Value> {
        serial_row_call(cx, args.exprs(), true)
    }

    fn browse_args_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("serial/row", "args"),
            Arc::new(ListShape::tuple(vec![Arc::new(AnyShape)])),
        )))
    }

    fn browse_result_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("serial/row", "result"),
            Arc::new(AnyShape),
        )))
    }
}

struct SerialMatrixFunction;

impl Object for SerialMatrixFunction {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok("#<function serial/matrix>".to_owned())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for SerialMatrixFunction {
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

impl Callable for SerialMatrixFunction {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        let exprs = args
            .into_vec()
            .into_iter()
            .map(|value| value.object().as_expr(cx))
            .collect::<Result<Vec<_>>>()?;
        serial_matrix_call(cx, &exprs, false)
    }

    fn call_exprs(&self, cx: &mut Cx, args: RawArgs) -> Result<Value> {
        serial_matrix_call(cx, args.exprs(), true)
    }

    fn browse_args_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        let keyword = |name| {
            Arc::new(ExactExprShape::new(Expr::Symbol(Symbol::new(name))))
                as Arc<dyn sim_shape::Shape>
        };
        Ok(Some(shape_value(
            Symbol::qualified("serial/matrix", "args"),
            Arc::new(ListShape::tuple(vec![
                Arc::new(AnyShape),
                keyword(":labels"),
                Arc::new(AnyShape),
            ])),
        )))
    }

    fn browse_result_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("serial/matrix", "result"),
            Arc::new(AnyShape),
        )))
    }
}

struct SerialAnalyzeRowFunction;

impl Object for SerialAnalyzeRowFunction {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok("#<function serial/analyze-row>".to_owned())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for SerialAnalyzeRowFunction {
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

impl Callable for SerialAnalyzeRowFunction {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        let exprs = args
            .into_vec()
            .into_iter()
            .map(|value| value.object().as_expr(cx))
            .collect::<Result<Vec<_>>>()?;
        serial_analyze_row_call(cx, &exprs, false)
    }

    fn call_exprs(&self, cx: &mut Cx, args: RawArgs) -> Result<Value> {
        serial_analyze_row_call(cx, args.exprs(), true)
    }

    fn browse_args_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("serial/analyze-row", "args"),
            Arc::new(ListShape::tuple(vec![Arc::new(AnyShape)])),
        )))
    }

    fn browse_result_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("serial/analyze-row", "result"),
            Arc::new(AnyShape),
        )))
    }
}

fn serial_row_call(cx: &mut Cx, args: &[Expr], evaluate: bool) -> Result<Value> {
    let [classes] = args else {
        return Err(Error::Eval(
            "serial/row expects one quoted or evaluated list of twelve pitch classes".to_owned(),
        ));
    };
    let source = value_expr(cx, classes, evaluate)?;
    let classes = classes_from_expr(&source)?;
    let encoded = encode_tone_row(
        &decode_tone_row(&classes.join(","))
            .map_err(|error| Error::Eval(format!("invalid serial row: {error}")))?,
    );
    cx.factory().expr(Expr::String(encoded))
}

fn serial_matrix_call(cx: &mut Cx, args: &[Expr], evaluate: bool) -> Result<Value> {
    let (row_expr, convention) = match args {
        [row] => (row, RowLabelConvention::FirstLastPitch),
        [row, labels_key, labels] => {
            expect_keyword(labels_key, "labels")?;
            (row, parse_convention(&value_expr(cx, labels, evaluate)?)?)
        }
        _ => {
            return Err(Error::Eval(
                "serial/matrix expects ROW or ROW :labels CONVENTION".to_owned(),
            ));
        }
    };
    let row_text = row_text(&value_expr(cx, row_expr, evaluate)?)?;
    let row = decode_tone_row(&row_text)
        .map_err(|error| Error::Eval(format!("invalid serial row: {error}")))?;
    let matrix = RowMatrix::new(&row, convention).render_data();
    cx.factory().expr(matrix_expr(&matrix))
}

fn serial_analyze_row_call(cx: &mut Cx, args: &[Expr], evaluate: bool) -> Result<Value> {
    let [row_expr] = args else {
        return Err(Error::Eval(
            "serial/analyze-row expects one row value".to_owned(),
        ));
    };
    let row_text = row_text(&value_expr(cx, row_expr, evaluate)?)?;
    let row = decode_tone_row(&row_text)
        .map_err(|error| Error::Eval(format!("invalid serial row: {error}")))?;
    let report = analyze_row_class(&row);
    cx.factory().expr(report_expr(&report))
}

fn value_expr(cx: &mut Cx, expr: &Expr, evaluate: bool) -> Result<Expr> {
    if evaluate {
        cx.eval_expr(expr.clone())?.object().as_expr(cx)
    } else {
        Ok(expr.clone())
    }
}

fn classes_from_expr(expr: &Expr) -> Result<Vec<String>> {
    match expr {
        Expr::List(values) | Expr::Vector(values) => values.iter().map(numberish).collect(),
        _ => Err(Error::TypeMismatch {
            expected: "list or vector of twelve pitch classes",
            found: "non-sequence",
        }),
    }
}

fn numberish(expr: &Expr) -> Result<String> {
    match expr {
        Expr::Number(number) => Ok(number.canonical.clone()),
        Expr::String(value) => Ok(value.clone()),
        Expr::Symbol(value) => Ok(value.name.to_string()),
        _ => Err(Error::TypeMismatch {
            expected: "numeric pitch-class literal",
            found: "non-number",
        }),
    }
}

fn row_text(expr: &Expr) -> Result<String> {
    match expr {
        Expr::String(text) => Ok(text.clone()),
        _ => Err(Error::TypeMismatch {
            expected: "canonical tone-row string",
            found: "non-string",
        }),
    }
}

fn expect_keyword(expr: &Expr, expected: &str) -> Result<()> {
    match expr {
        Expr::Symbol(symbol) if symbol.name.as_ref() == format!(":{expected}") => Ok(()),
        _ => Err(Error::Eval(format!("expected :{expected} keyword"))),
    }
}

fn parse_convention(expr: &Expr) -> Result<RowLabelConvention> {
    let name = match expr {
        Expr::String(value) => value.as_str(),
        Expr::Symbol(symbol) => symbol.name.as_ref(),
        _ => {
            return Err(Error::TypeMismatch {
                expected: "row-label convention string or symbol",
                found: "non-symbol",
            });
        }
    };
    match name.trim_start_matches(':') {
        "first-last-pitch" => Ok(RowLabelConvention::FirstLastPitch),
        "operation-index" => Ok(RowLabelConvention::OperationIndex),
        other => Err(Error::Eval(format!(
            "unknown serial matrix label convention {other}"
        ))),
    }
}

fn matrix_expr(matrix: &sim_lib_pitch_serial::RowMatrixData) -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("form")),
            Expr::String("SerialMatrix".to_owned()),
        ),
        (
            Expr::Symbol(Symbol::new("source")),
            Expr::String(encode_tone_row(matrix.source())),
        ),
        (
            Expr::Symbol(Symbol::new("label-convention")),
            Expr::String(matrix.convention().as_str().to_owned()),
        ),
        (
            Expr::Symbol(Symbol::new("rows")),
            Expr::Vector(
                (0..12)
                    .map(|row| {
                        Expr::Vector(
                            (0..12)
                                .map(|column| {
                                    Expr::String(
                                        matrix
                                            .cell(
                                                sim_lib_pitch_serial::MatrixCoordinate::new(
                                                    row, column,
                                                )
                                                .expect("fixed matrix coordinate"),
                                            )
                                            .class()
                                            .value()
                                            .to_string(),
                                    )
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            ),
        ),
        (
            Expr::Symbol(Symbol::new("left-labels")),
            Expr::Vector(
                matrix
                    .edge_labels()
                    .left()
                    .iter()
                    .map(|label| Expr::String(label.to_string()))
                    .collect(),
            ),
        ),
        (
            Expr::Symbol(Symbol::new("top-labels")),
            Expr::Vector(
                matrix
                    .edge_labels()
                    .top()
                    .iter()
                    .map(|label| Expr::String(label.to_string()))
                    .collect(),
            ),
        ),
    ])
}

fn report_expr(report: &sim_lib_pitch_serial::RowClassReport) -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("form")),
            Expr::String("RowClassReport".to_owned()),
        ),
        (
            Expr::Symbol(Symbol::new("row")),
            Expr::String(encode_tone_row(&report.row)),
        ),
        (
            Expr::Symbol(Symbol::new("ordered-intervals")),
            Expr::Vector(
                report
                    .ordered_intervals
                    .intervals()
                    .iter()
                    .map(|value| Expr::String(value.to_string()))
                    .collect(),
            ),
        ),
        (
            Expr::Symbol(Symbol::new("stabilizers")),
            Expr::Vector(
                report
                    .stabilizers
                    .iter()
                    .map(|operation| Expr::String(operation.to_string()))
                    .collect(),
            ),
        ),
        (
            Expr::Symbol(Symbol::new("distinct-forms")),
            Expr::Vector(
                report
                    .distinct_forms
                    .iter()
                    .map(|row| Expr::String(encode_tone_row(row)))
                    .collect(),
            ),
        ),
        (
            Expr::Symbol(Symbol::new("alias-count")),
            Expr::String(report.aliases.len().to_string()),
        ),
        (
            Expr::Symbol(Symbol::new("derivation")),
            Expr::String(format!("{:?}", report.derivation)),
        ),
        (
            Expr::Symbol(Symbol::new("all-interval")),
            Expr::String(format!("{:?}", report.all_interval)),
        ),
        (
            Expr::Symbol(Symbol::new("combinatoriality")),
            Expr::String(format!("{:?}", report.combinatoriality)),
        ),
    ])
}
