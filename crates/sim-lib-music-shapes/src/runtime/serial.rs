//! Runtime Shape and Lisp callable for symbolic serial validation.

use std::sync::Arc;

use sim_kernel::{
    Args, Callable, ClassRef, Cx, Error, Expr, Linker, Object, ObjectCompat, RawArgs, Result,
    ShapeRef, Symbol, Value,
};
use sim_shape::{AnyShape, ListShape, Shape, ShapeDoc, ShapeMatch, shape_value};

use super::{
    DomainFormShape, form_field, form_shape, list_field, music_serial_validate_symbol, string_field,
};
use crate::{decode_serial_plan, decode_serial_series};

pub(super) fn serial_series_shape() -> Arc<dyn Shape> {
    Arc::new(SerialSeriesShape {
        structural: DomainFormShape::new(form_shape(
            "SerialSeries",
            vec![
                string_field("alphabet_id"),
                list_field("symbols"),
                form_field("rule"),
                list_field("order"),
            ],
        )),
    })
}

pub(super) fn serial_plan_shape() -> Arc<dyn Shape> {
    Arc::new(SerialPlanShape {
        structural: DomainFormShape::new(form_shape(
            "SerialPlan",
            vec![
                list_field("rows"),
                list_field("events"),
                list_field("precedence"),
            ],
        )),
    })
}

pub(super) fn load_validate_function(
    cx: &mut sim_kernel::LoadCx,
    linker: &mut Linker<'_>,
) -> Result<()> {
    linker.function_value(
        music_serial_validate_symbol(),
        cx.factory().opaque(Arc::new(SerialValidateFunction))?,
    )?;
    Ok(())
}

struct SerialSeriesShape {
    structural: DomainFormShape,
}

struct SerialPlanShape {
    structural: DomainFormShape,
}

impl Shape for SerialSeriesShape {
    fn is_effectful(&self) -> bool {
        false
    }

    fn is_total(&self) -> bool {
        false
    }

    fn check_value(&self, cx: &mut Cx, value: Value) -> Result<ShapeMatch> {
        let expr = value.object().as_expr(cx)?;
        self.check_expr(cx, &expr)
    }

    fn check_expr(&self, cx: &mut Cx, expr: &Expr) -> Result<ShapeMatch> {
        let structural = self.structural.check_expr(cx, expr)?;
        if !structural.accepted {
            return Ok(structural);
        }
        if let Expr::String(text) = expr
            && let Err(error) = decode_serial_series(text)
        {
            return Ok(ShapeMatch::reject(format!("shape-serial-series: {error}")));
        }
        Ok(structural)
    }

    fn describe(&self, cx: &mut Cx) -> Result<ShapeDoc> {
        self.structural.describe(cx)
    }
}

impl Shape for SerialPlanShape {
    fn is_effectful(&self) -> bool {
        false
    }

    fn is_total(&self) -> bool {
        false
    }

    fn check_value(&self, cx: &mut Cx, value: Value) -> Result<ShapeMatch> {
        let expr = value.object().as_expr(cx)?;
        self.check_expr(cx, &expr)
    }

    fn check_expr(&self, cx: &mut Cx, expr: &Expr) -> Result<ShapeMatch> {
        let structural = self.structural.check_expr(cx, expr)?;
        if !structural.accepted {
            return Ok(structural);
        }
        if let Expr::String(text) = expr
            && let Err(error) = decode_serial_plan(text)
        {
            return Ok(ShapeMatch::reject(format!("shape-serial-plan: {error}")));
        }
        Ok(structural)
    }

    fn describe(&self, cx: &mut Cx) -> Result<ShapeDoc> {
        self.structural.describe(cx)
    }
}

struct SerialValidateFunction;

impl Object for SerialValidateFunction {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok("#<function music/serial/validate>".to_owned())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for SerialValidateFunction {
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

impl Callable for SerialValidateFunction {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        let exprs = args
            .into_vec()
            .into_iter()
            .map(|value| value.object().as_expr(cx))
            .collect::<Result<Vec<_>>>()?;
        validate_call(cx, &exprs, false)
    }

    fn call_exprs(&self, cx: &mut Cx, args: RawArgs) -> Result<Value> {
        validate_call(cx, args.exprs(), true)
    }

    fn browse_args_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("music/serial/validate", "args"),
            Arc::new(ListShape::tuple(vec![Arc::new(AnyShape)])),
        )))
    }

    fn browse_result_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("music/serial/validate", "result"),
            Arc::new(AnyShape),
        )))
    }
}

fn validate_call(cx: &mut Cx, args: &[Expr], evaluate: bool) -> Result<Value> {
    let [source] = args else {
        return Err(Error::Eval(
            "music/serial/validate expects one #(SerialSeries ...) string".to_owned(),
        ));
    };
    let source = if evaluate {
        cx.eval_expr(source.clone())?.object().as_expr(cx)?
    } else {
        source.clone()
    };
    let Expr::String(source) = source else {
        return Err(Error::TypeMismatch {
            expected: "#(SerialSeries ...) string",
            found: "non-string",
        });
    };
    let series = decode_serial_series(&source)
        .map_err(|error| Error::Eval(format!("invalid serial series: {error}")))?;
    let rank = series
        .permutation_rank()
        .map(|rank| rank.to_string())
        .unwrap_or_else(|_| "not-a-permutation".to_owned());
    let ledger = series.ledger();
    cx.factory().expr(Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("alphabet-id")),
            Expr::String(ledger.alphabet_id().as_str().to_owned()),
        ),
        (
            Expr::Symbol(Symbol::new("rule")),
            Expr::Symbol(Symbol::new(format!("{:?}", ledger.rule()))),
        ),
        (
            Expr::Symbol(Symbol::new("series-length")),
            Expr::String(ledger.series_len().to_string()),
        ),
        (
            Expr::Symbol(Symbol::new("permutation-rank")),
            Expr::String(rank),
        ),
        (
            Expr::Symbol(Symbol::new("omitted")),
            Expr::List(
                ledger
                    .omitted_symbols()
                    .iter()
                    .cloned()
                    .map(Expr::String)
                    .collect(),
            ),
        ),
        (
            Expr::Symbol(Symbol::new("repeated")),
            Expr::List(
                ledger
                    .repeated_symbols()
                    .iter()
                    .cloned()
                    .map(Expr::String)
                    .collect(),
            ),
        ),
    ]))
}
