//! Runtime Shape and Lisp callable for symbolic serial validation.

use std::sync::Arc;

use sim_kernel::{
    Args, Callable, ClassRef, Cx, Error, Expr, Linker, Object, ObjectCompat, RawArgs, Result,
    ShapeRef, Symbol, Value,
};
use sim_lib_music_core::{Articulation, Channel, Time, parse_pitch};
use sim_lib_music_serial::{
    EventSound, RealizationContext, SerialEventId, StrictEventSpec, TiePolicy,
    default_realizer_registry,
};
use sim_lib_pitch_scale::{Mode, Scale};
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
    linker.function_value(
        music_serial_realize_symbol(),
        cx.factory().opaque(Arc::new(SerialRealizeFunction))?,
    )?;
    for (symbol, value) in built_in_realizer_values(cx)? {
        linker.value(symbol, value)?;
    }
    Ok(())
}

pub(super) fn music_serial_realize_symbol() -> Symbol {
    Symbol::qualified("serial", "realize")
}

pub(super) fn built_in_realizer_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("serial/realizer", "strict-chromatic"),
        Symbol::qualified("serial/realizer", "modal-degree-cycle"),
        Symbol::qualified("serial/realizer", "modal-nearest-scale-tone"),
        Symbol::qualified("serial/realizer", "modal-marked-chromatic-inflection"),
        Symbol::qualified("serial/realizer", "modal-non-pitch-spine"),
    ]
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
struct SerialRealizeFunction;

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

impl Object for SerialRealizeFunction {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok("#<function serial/realize>".to_owned())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for SerialRealizeFunction {
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

impl Callable for SerialRealizeFunction {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        let exprs = args
            .into_vec()
            .into_iter()
            .map(|value| value.object().as_expr(cx))
            .collect::<Result<Vec<_>>>()?;
        realize_call(cx, &exprs, false)
    }

    fn call_exprs(&self, cx: &mut Cx, args: RawArgs) -> Result<Value> {
        realize_call(cx, args.exprs(), true)
    }

    fn browse_args_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("serial/realize", "args"),
            Arc::new(ListShape::tuple(vec![
                Arc::new(AnyShape),
                Arc::new(AnyShape),
                Arc::new(AnyShape),
            ])),
        )))
    }

    fn browse_result_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("serial/realize", "result"),
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

fn realize_call(cx: &mut Cx, args: &[Expr], evaluate: bool) -> Result<Value> {
    let [request, with_key, realizer_id] = args else {
        return Err(Error::Eval(
            "serial/realize expects REQUEST :with REALIZER-ID".to_owned(),
        ));
    };
    expect_keyword(with_key, "with")?;
    let request = value_expr(cx, request, evaluate)?;
    let realizer_id = stringish(&value_expr(cx, realizer_id, evaluate)?)?;
    let (plan, context) = decode_realization_request(&request)?;
    let realization = default_realizer_registry()
        .realize_named(&format!("realizer/{realizer_id}"), &plan, &context)
        .map_err(|error| Error::Eval(format!("serial realization failed: {error}")))?;
    cx.factory().expr(realization_expr(&realization))
}

fn value_expr(cx: &mut Cx, expr: &Expr, evaluate: bool) -> Result<Expr> {
    if evaluate {
        cx.eval_expr(expr.clone())?.object().as_expr(cx)
    } else {
        Ok(expr.clone())
    }
}

fn expect_keyword(expr: &Expr, expected: &str) -> Result<()> {
    match expr {
        Expr::Symbol(symbol) if symbol.name.as_ref() == format!(":{expected}") => Ok(()),
        _ => Err(Error::Eval(format!("expected :{expected} keyword"))),
    }
}

fn stringish(expr: &Expr) -> Result<String> {
    match expr {
        Expr::String(value) => Ok(value.clone()),
        Expr::Symbol(value) => Ok(value.name.to_string()),
        _ => Err(Error::TypeMismatch {
            expected: "string or symbol",
            found: "non-string",
        }),
    }
}

fn decode_realization_request(
    expr: &Expr,
) -> Result<(sim_lib_music_serial::SerialPlan, RealizationContext)> {
    let Expr::Map(fields) = expr else {
        return Err(Error::TypeMismatch {
            expected: "serial realization request map",
            found: "non-map",
        });
    };
    let plan = lookup_required(fields, "plan")?;
    let context = lookup_required(fields, "context")?;
    let Expr::String(plan_text) = plan else {
        return Err(Error::TypeMismatch {
            expected: "serial plan string",
            found: "non-string",
        });
    };
    let plan = decode_serial_plan(plan_text)
        .map_err(|error| Error::Eval(format!("invalid serial plan: {error}")))?;
    let context = parse_context(context)?;
    Ok((plan, context))
}

fn parse_context(expr: &Expr) -> Result<RealizationContext> {
    let Expr::Map(fields) = expr else {
        return Err(Error::TypeMismatch {
            expected: "realization context map",
            found: "non-map",
        });
    };
    let specs = expr_items(lookup_required(fields, "specs")?, "spec sequence")?;
    let specs = specs
        .iter()
        .map(parse_spec)
        .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
    let mut context = RealizationContext::new(specs);
    if let Some(scale) = lookup_optional(fields, "scale") {
        let text = stringish(scale)?;
        if text != "none" {
            context.scale = Some(parse_scale(&text)?);
        }
    }
    Ok(context)
}

fn parse_spec(expr: &Expr) -> Result<(SerialEventId, StrictEventSpec)> {
    let Expr::Map(fields) = expr else {
        return Err(Error::TypeMismatch {
            expected: "strict event spec map",
            found: "non-map",
        });
    };
    let id = SerialEventId::new(stringish(lookup_required(fields, "id")?)?)
        .map_err(|error| Error::Eval(format!("invalid event id: {error}")))?;
    let sound = match stringish(lookup_required(fields, "sound")?)?.as_str() {
        "notes" => EventSound::Notes,
        "rest" => EventSound::Rest,
        other => return Err(Error::Eval(format!("invalid event sound {other}"))),
    };
    let register = parse_i8(lookup_required(fields, "register")?)?;
    let duration = parse_time(lookup_required(fields, "duration")?)?;
    let velocity = parse_u8(lookup_required(fields, "velocity")?)?;
    let channel = Channel::new(parse_u8(lookup_required(fields, "channel")?)?)
        .map_err(|error| Error::Eval(format!("invalid channel: {error}")))?;
    let articulation = parse_articulation(&stringish(lookup_required(fields, "articulation")?)?)?;
    let tie = match lookup_optional(fields, "tie")
        .map(stringish)
        .transpose()?
        .unwrap_or_else(|| "none".to_owned())
        .as_str()
    {
        "none" => TiePolicy::None,
        "into-next" => TiePolicy::IntoNext,
        other => return Err(Error::Eval(format!("invalid tie policy {other}"))),
    };
    let octave_displacements = match lookup_optional(fields, "octave-displacements") {
        Some(values) => expr_items(values, "octave displacement sequence")?
            .iter()
            .map(parse_i8)
            .collect::<Result<Vec<_>>>()?,
        None => Vec::new(),
    };
    Ok((
        id,
        StrictEventSpec {
            sound,
            pitch_layout: sim_lib_music_serial::StrictPitchLayout {
                register,
                octave_displacements,
            },
            duration,
            velocity,
            channel,
            articulation,
            tie,
        },
    ))
}

fn parse_articulation(value: &str) -> Result<Articulation> {
    match value {
        "Normal" => Ok(Articulation::Normal),
        "Staccato" => Ok(Articulation::Staccato),
        "Legato" => Ok(Articulation::Legato),
        "Tenuto" => Ok(Articulation::Tenuto),
        "Accent" => Ok(Articulation::Accent),
        "Marcato" => Ok(Articulation::Marcato),
        other => Err(Error::Eval(format!("invalid articulation {other}"))),
    }
}

fn parse_scale(value: &str) -> Result<Scale> {
    let (tonic, mode) = value
        .split_once(':')
        .ok_or_else(|| Error::Eval(format!("invalid scale {value}")))?;
    let tonic = parse_pitch(&format!("{tonic}4"))
        .map_err(|_| Error::Eval(format!("invalid scale {value}")))?;
    let mode = match mode {
        "major" => Mode::Major,
        "minor-natural" => Mode::MinorNatural,
        "minor-harmonic" => Mode::MinorHarmonic,
        "minor-melodic" => Mode::MinorMelodic,
        "dorian" => Mode::Dorian,
        "phrygian" => Mode::Phrygian,
        "lydian" => Mode::Lydian,
        "mixolydian" => Mode::Mixolydian,
        "aeolian" => Mode::Aeolian,
        "locrian" => Mode::Locrian,
        "whole-tone" => Mode::WholeTone,
        "diminished" => Mode::Diminished,
        "chromatic" => Mode::Chromatic,
        _ => return Err(Error::Eval(format!("invalid scale {value}"))),
    };
    Ok(Scale::new(tonic.class, mode))
}

fn expr_items<'a>(expr: &'a Expr, expected: &'static str) -> Result<&'a [Expr]> {
    match expr {
        Expr::Vector(items) | Expr::List(items) => Ok(items),
        _ => Err(Error::TypeMismatch {
            expected,
            found: "non-sequence",
        }),
    }
}

fn parse_time(expr: &Expr) -> Result<Time> {
    let text = stringish(expr)?;
    let Some((numer, denom)) = text.split_once('/') else {
        return Err(Error::Eval(format!("invalid time literal {text}")));
    };
    let numer = numer
        .parse::<i64>()
        .map_err(|_| Error::Eval(format!("invalid time literal {text}")))?;
    let denom = denom
        .parse::<i64>()
        .map_err(|_| Error::Eval(format!("invalid time literal {text}")))?;
    Ok(Time::new(numer, denom))
}

fn parse_u8(expr: &Expr) -> Result<u8> {
    stringish(expr)?
        .parse::<u8>()
        .map_err(|_| Error::Eval("expected u8 literal".to_owned()))
}

fn parse_i8(expr: &Expr) -> Result<i8> {
    stringish(expr)?
        .parse::<i8>()
        .map_err(|_| Error::Eval("expected i8 literal".to_owned()))
}

fn lookup_required<'a>(fields: &'a [(Expr, Expr)], name: &str) -> Result<&'a Expr> {
    fields
        .iter()
        .find_map(|(key, value)| match key {
            Expr::Symbol(symbol) if symbol.name.as_ref() == name => Some(value),
            _ => None,
        })
        .ok_or_else(|| Error::Eval(format!("missing {name} field")))
}

fn lookup_optional<'a>(fields: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    fields.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol) if symbol.name.as_ref() == name => Some(value),
        _ => None,
    })
}

fn realization_expr(realization: &sim_lib_music_serial::SerialRealization) -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("form")),
            Expr::String("SerialRealization".to_owned()),
        ),
        (
            Expr::Symbol(Symbol::new("realizer-id")),
            Expr::String(
                realization
                    .notes()
                    .first()
                    .map(|note| note.origin.realizer_id.as_str().to_owned())
                    .unwrap_or_else(|| "realizer/unknown".to_owned()),
            ),
        ),
        (
            Expr::Symbol(Symbol::new("notes")),
            Expr::Vector(
                realization
                    .notes()
                    .iter()
                    .map(|note| {
                        Expr::Map(vec![
                            (
                                Expr::Symbol(Symbol::new("event-id")),
                                Expr::String(note.event_id.as_str().to_owned()),
                            ),
                            (
                                Expr::Symbol(Symbol::new("voice")),
                                Expr::String(note.voice.as_str().to_owned()),
                            ),
                            (
                                Expr::Symbol(Symbol::new("pitch")),
                                Expr::String(note.note.pitch.to_midi().unwrap_or(0).to_string()),
                            ),
                            (
                                Expr::Symbol(Symbol::new("duration")),
                                Expr::String(format!(
                                    "{}/{}",
                                    note.note.duration.numer(),
                                    note.note.duration.denom()
                                )),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            Expr::Symbol(Symbol::new("ledger")),
            Expr::Vector(
                realization
                    .ledger()
                    .entries()
                    .iter()
                    .map(|entry| {
                        Expr::Map(vec![
                            (
                                Expr::Symbol(Symbol::new("rule-id")),
                                Expr::String(entry.rule_id.as_str().to_owned()),
                            ),
                            (
                                Expr::Symbol(Symbol::new("invariant-id")),
                                Expr::String(
                                    entry
                                        .invariant_id
                                        .clone()
                                        .unwrap_or_else(|| "none".to_owned()),
                                ),
                            ),
                            (
                                Expr::Symbol(Symbol::new("status")),
                                Expr::String(format!("{:?}", entry.status)),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            Expr::Symbol(Symbol::new("spine-kind")),
            Expr::String(
                realization
                    .spine_report()
                    .map(|report| format!("{:?}", report.kind))
                    .unwrap_or_else(|| "none".to_owned()),
            ),
        ),
    ])
}

fn built_in_realizer_values(cx: &mut sim_kernel::LoadCx) -> Result<Vec<(Symbol, Value)>> {
    let rows = [
        ("strict-chromatic", "realizer/strict-chromatic"),
        ("modal-degree-cycle", "realizer/modal-degree-cycle"),
        (
            "modal-nearest-scale-tone",
            "realizer/modal-nearest-scale-tone",
        ),
        (
            "modal-marked-chromatic-inflection",
            "realizer/modal-marked-chromatic-inflection",
        ),
        ("modal-non-pitch-spine", "realizer/modal-non-pitch-spine"),
    ];
    rows.into_iter()
        .map(|(symbol_name, id)| {
            let symbol = Symbol::qualified("serial/realizer", symbol_name);
            let value = cx.factory().table(vec![
                (Symbol::new("id"), cx.factory().string(id.to_owned())?),
                (
                    Symbol::new("kind"),
                    cx.factory().string("SerialRealizer".to_owned())?,
                ),
                (
                    Symbol::new("callable"),
                    cx.factory().symbol(music_serial_realize_symbol())?,
                ),
            ])?;
            Ok((symbol, value))
        })
        .collect()
}
