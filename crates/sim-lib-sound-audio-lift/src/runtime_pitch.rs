//! Loadable `sound/lift/pitch-track` callable and request decoding.

use std::{any::Any, collections::BTreeMap, sync::Arc};

use sim_kernel::{
    Args, Callable, ClassRef, Cx, Error, Expr, Object, ObjectCompat, RawArgs, Result, ShapeRef,
    Symbol, Value,
};
use sim_lib_sound_tuning::EqualTemperament;
use sim_shape::{AnyShape, ListShape, shape_value};

use crate::{
    PitchFramePolicy, PitchFrameTail, PitchInterpolation, PitchRange, PitchTrackControl,
    PitchTrackMethod, PitchTrackPlan, YinPolicy, pitch_track,
    runtime_pitch_report::pitch_track_report_expr,
};

/// Symbol of the Lisp-facing monophonic pitch tracker.
pub fn sound_lift_pitch_track_symbol() -> Symbol {
    Symbol::qualified("sound/lift", "pitch-track")
}

/// Calls `sound/lift/pitch-track` with evaluated PCM and keyword values.
pub fn call_sound_lift_pitch_track(cx: &mut Cx, args: Args) -> Result<Value> {
    let mut expressions = Vec::new();
    for value in args.into_vec() {
        expressions.push(value.object().as_expr(cx)?);
    }
    execute(cx, &expressions, false)
}

pub(crate) fn pitch_track_function_value(cx: &mut sim_kernel::LoadCx) -> Result<Value> {
    cx.factory().opaque(Arc::new(PitchTrackFunction))
}

struct PitchTrackFunction;

impl Object for PitchTrackFunction {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok("#<function sound/lift/pitch-track>".to_owned())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ObjectCompat for PitchTrackFunction {
    fn class(&self, cx: &mut Cx) -> Result<ClassRef> {
        cx.factory().class_stub(
            sim_kernel::CORE_FUNCTION_CLASS_ID,
            Symbol::qualified("core", "Function"),
        )
    }

    fn as_expr(&self, _cx: &mut Cx) -> Result<Expr> {
        Ok(Expr::Symbol(sound_lift_pitch_track_symbol()))
    }

    fn as_callable(&self) -> Option<&dyn Callable> {
        Some(self)
    }
}

impl Callable for PitchTrackFunction {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        call_sound_lift_pitch_track(cx, args)
    }

    fn call_exprs(&self, cx: &mut Cx, args: RawArgs) -> Result<Value> {
        execute(cx, args.exprs(), true)
    }

    fn browse_args_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("sound/lift/pitch-track", "args"),
            Arc::new(ListShape::variadic(
                vec![Arc::new(AnyShape)],
                Arc::new(AnyShape),
            )),
        )))
    }

    fn browse_result_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("sound/lift/pitch-track", "result"),
            Arc::new(AnyShape),
        )))
    }
}

fn execute(cx: &mut Cx, expressions: &[Expr], evaluate: bool) -> Result<Value> {
    let [pcm, options @ ..] = expressions else {
        return Err(argument_error());
    };
    if options.len() % 2 != 0 {
        return Err(argument_error());
    }
    let pcm = value_expr(cx, pcm, evaluate)?;
    let mut parsed = BTreeMap::new();
    for pair in options.chunks_exact(2) {
        let name = key_name(&pair[0])?;
        let value = value_expr(cx, &pair[1], evaluate)?;
        if parsed.insert(name.clone(), value).is_some() {
            return Err(Error::Eval(format!(
                "sound/lift/pitch-track received duplicate :{name}"
            )));
        }
    }
    reject_unknown(&parsed, &["method", "range", "frames", "yin", "control"])?;
    let (samples, sample_rate) = parse_pcm(&pcm)?;
    let mut plan = PitchTrackPlan::default();
    if let Some(method) = parsed.get("method") {
        plan.method = match symbolish(method)?.as_str() {
            "yin" => PitchTrackMethod::Yin,
            "pyin" => PitchTrackMethod::Pyin,
            other => {
                return Err(Error::Eval(format!(
                    "sound/lift/pitch-track unsupported method {other}"
                )));
            }
        };
    }
    if let Some(range) = parsed.get("range") {
        plan.range = parse_range(range)?;
    }
    if let Some(frames) = parsed.get("frames") {
        plan.frames = parse_frames(frames, plan.frames)?;
        plan.interpolation = parse_interpolation(frames, plan.interpolation)?;
    }
    if let Some(yin) = parsed.get("yin") {
        plan.yin = parse_yin(yin, plan.yin)?;
    }
    if let Some(control) = parsed.get("control") {
        plan.control = parse_control(control, plan.control)?;
    }
    let report = pitch_track(&samples, sample_rate, &EqualTemperament::default(), &plan)
        .map_err(|error| Error::Eval(error.to_string()))?;
    cx.factory().expr(pitch_track_report_expr(&report))
}

fn parse_pcm(expr: &Expr) -> Result<(Vec<f32>, u32)> {
    let fields = map_fields(expr, "PCM map with :samples and :sample-rate")?;
    reject_unknown(&fields, &["samples", "sample-rate"])?;
    let samples = fields
        .get("samples")
        .ok_or_else(|| Error::Eval("pitch-track PCM requires :samples".to_owned()))?;
    let sample_rate = fields
        .get("sample-rate")
        .ok_or_else(|| Error::Eval("pitch-track PCM requires :sample-rate".to_owned()))?;
    let samples = sequence(samples, "PCM :samples")?
        .iter()
        .map(|sample| {
            let value = f64_value(sample, "PCM sample")?;
            let narrowed = value as f32;
            if !narrowed.is_finite() {
                return Err(Error::Eval("PCM sample is outside f32 range".to_owned()));
            }
            Ok(narrowed)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((samples, u32_value(sample_rate, "PCM sample rate")?))
}

fn parse_range(expr: &Expr) -> Result<PitchRange> {
    let values = sequence(expr, "pitch range")?;
    let [min, max] = values else {
        return Err(Error::Eval(
            "pitch range must contain exactly two frequencies".to_owned(),
        ));
    };
    PitchRange::new(
        f64_value(min, "minimum pitch frequency")?,
        f64_value(max, "maximum pitch frequency")?,
    )
    .map_err(|error| Error::Eval(error.to_string()))
}

fn parse_frames(expr: &Expr, mut policy: PitchFramePolicy) -> Result<PitchFramePolicy> {
    let fields = map_fields(expr, "frame policy map")?;
    reject_unknown(&fields, &["size", "hop", "tail", "interpolation"])?;
    if let Some(value) = fields.get("size") {
        policy.size = usize_value(value, "frame size")?;
    }
    if let Some(value) = fields.get("hop") {
        policy.hop = usize_value(value, "frame hop")?;
    }
    if let Some(value) = fields.get("tail") {
        policy.tail = match symbolish(value)?.as_str() {
            "drop" => PitchFrameTail::Drop,
            "zero-pad" | "pad" => PitchFrameTail::ZeroPad,
            other => {
                return Err(Error::Eval(format!(
                    "unsupported frame tail policy {other}"
                )));
            }
        };
    }
    Ok(policy)
}

fn parse_interpolation(expr: &Expr, default: PitchInterpolation) -> Result<PitchInterpolation> {
    let fields = map_fields(expr, "frame policy map")?;
    let Some(value) = fields.get("interpolation") else {
        return Ok(default);
    };
    match symbolish(value)?.as_str() {
        "none" | "integer" => Ok(PitchInterpolation::None),
        "parabolic" => Ok(PitchInterpolation::Parabolic),
        other => Err(Error::Eval(format!(
            "unsupported pitch interpolation {other}"
        ))),
    }
}

fn parse_yin(expr: &Expr, mut policy: YinPolicy) -> Result<YinPolicy> {
    let fields = map_fields(expr, "YIN policy map")?;
    reject_unknown(
        &fields,
        &[
            "threshold",
            "thresholds",
            "voiced-probability",
            "silence-rms",
        ],
    )?;
    if let Some(value) = fields.get("threshold") {
        policy.threshold = f64_value(value, "YIN threshold")?;
    }
    if let Some(value) = fields.get("thresholds") {
        policy.pyin_thresholds = sequence(value, "pYIN thresholds")?
            .iter()
            .map(|value| f64_value(value, "pYIN threshold"))
            .collect::<Result<Vec<_>>>()?;
    }
    if let Some(value) = fields.get("voiced-probability") {
        policy.min_voiced_probability = f64_value(value, "voiced probability")?;
    }
    if let Some(value) = fields.get("silence-rms") {
        policy.silence_rms = f64_value(value, "silence RMS")?;
    }
    Ok(policy)
}

fn parse_control(expr: &Expr, mut control: PitchTrackControl) -> Result<PitchTrackControl> {
    let fields = map_fields(expr, "pitch control map")?;
    reject_unknown(&fields, &["work", "results", "seed"])?;
    if let Some(value) = fields.get("work") {
        control.max_work = u64_value(value, "work limit")?;
    }
    if let Some(value) = fields.get("results") {
        control.max_results = usize_value(value, "result limit")?;
    }
    if let Some(value) = fields.get("seed") {
        control.seed = u64_value(value, "seed")?;
    }
    Ok(control)
}

fn map_fields(expr: &Expr, expected: &'static str) -> Result<BTreeMap<String, Expr>> {
    let Expr::Map(entries) = unquote_ref(expr) else {
        return Err(Error::TypeMismatch {
            expected,
            found: "non-map",
        });
    };
    let mut fields = BTreeMap::new();
    for (key, value) in entries {
        let name = key_name(key)?;
        if fields.insert(name.clone(), value.clone()).is_some() {
            return Err(Error::Eval(format!("duplicate :{name} field")));
        }
    }
    Ok(fields)
}

fn reject_unknown(fields: &BTreeMap<String, Expr>, admitted: &[&str]) -> Result<()> {
    if let Some(name) = fields
        .keys()
        .find(|name| !admitted.contains(&name.as_str()))
    {
        return Err(Error::Eval(format!("unknown pitch-track option :{name}")));
    }
    Ok(())
}

fn sequence<'a>(expr: &'a Expr, expected: &'static str) -> Result<&'a [Expr]> {
    match unquote_ref(expr) {
        Expr::List(values) | Expr::Vector(values) => Ok(values),
        _ => Err(Error::TypeMismatch {
            expected,
            found: "non-sequence",
        }),
    }
}

fn f64_value(expr: &Expr, name: &str) -> Result<f64> {
    let text = scalar_text(expr)?;
    let value = text
        .parse::<f64>()
        .map_err(|_| Error::Eval(format!("invalid {name} {text}")))?;
    if !value.is_finite() {
        return Err(Error::Eval(format!("{name} must be finite")));
    }
    Ok(value)
}

fn usize_value(expr: &Expr, name: &str) -> Result<usize> {
    scalar_text(expr)?
        .parse()
        .map_err(|_| Error::Eval(format!("invalid {name}")))
}

fn u32_value(expr: &Expr, name: &str) -> Result<u32> {
    let value = scalar_text(expr)?
        .parse::<u32>()
        .map_err(|_| Error::Eval(format!("invalid {name}")))?;
    if value == 0 {
        return Err(Error::Eval(format!("{name} must be positive")));
    }
    Ok(value)
}

fn u64_value(expr: &Expr, name: &str) -> Result<u64> {
    scalar_text(expr)?
        .parse()
        .map_err(|_| Error::Eval(format!("invalid {name}")))
}

fn scalar_text(expr: &Expr) -> Result<String> {
    match unquote_ref(expr) {
        Expr::Number(value) => Ok(value.canonical.clone()),
        Expr::String(value) => Ok(value.clone()),
        Expr::Symbol(value) => Ok(value.name.to_string()),
        _ => Err(Error::TypeMismatch {
            expected: "number, symbol, or string",
            found: "compound expression",
        }),
    }
}

fn key_name(expr: &Expr) -> Result<String> {
    match unquote_ref(expr) {
        Expr::Symbol(symbol) => Ok(symbol
            .name
            .strip_prefix(':')
            .unwrap_or(symbol.name.as_ref())
            .to_owned()),
        Expr::String(value) => Ok(value.trim_start_matches(':').to_owned()),
        _ => Err(Error::TypeMismatch {
            expected: "keyword symbol",
            found: "non-symbol",
        }),
    }
}

fn symbolish(expr: &Expr) -> Result<String> {
    match unquote_ref(expr) {
        Expr::Symbol(value) => Ok(value.name.to_string()),
        Expr::String(value) => Ok(value.clone()),
        _ => Err(Error::TypeMismatch {
            expected: "symbol or string",
            found: "other expression",
        }),
    }
}

fn value_expr(cx: &mut Cx, expr: &Expr, evaluate: bool) -> Result<Expr> {
    if !evaluate {
        return Ok(expr.clone());
    }
    if let Expr::Block(items) = expr {
        if items.len() % 2 != 0 {
            return Err(Error::Eval(
                "pitch-track policy blocks require keyword/value pairs".to_owned(),
            ));
        }
        let mut entries = Vec::with_capacity(items.len() / 2);
        for pair in items.chunks_exact(2) {
            key_name(&pair[0])?;
            entries.push((pair[0].clone(), value_expr(cx, &pair[1], true)?));
        }
        return Ok(Expr::Map(entries));
    }
    cx.eval_expr(expr.clone())?.object().as_expr(cx)
}

fn unquote_ref(expr: &Expr) -> &Expr {
    match expr {
        Expr::Quote { expr, .. } => expr,
        other => other,
    }
}

fn argument_error() -> Error {
    Error::Eval("sound/lift/pitch-track expects PCM followed by keyword/value pairs".to_owned())
}
