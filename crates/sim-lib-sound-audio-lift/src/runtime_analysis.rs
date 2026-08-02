//! Loadable `sound/lift/analyze` callable and request decoding.

use std::{any::Any, collections::BTreeMap, sync::Arc};

use sim_kernel::{
    Args, Callable, ClassRef, Cx, Error, Expr, Object, ObjectCompat, RawArgs, Result, ShapeRef,
    Symbol, Value,
};
use sim_lib_music_analysis::HarmonicDecodeStrategy;
use sim_lib_sound_tuning::EqualTemperament;
use sim_shape::{AnyShape, ListShape, shape_value};

use crate::{
    AudioAnalysisControl, AudioAnalysisPlan, AudioFeatureSelection, analyze_audio,
    runtime_analysis_report::audio_analysis_expr,
};

/// Symbol of the Lisp-facing composed PCM analyzer.
pub fn sound_lift_analyze_symbol() -> Symbol {
    Symbol::qualified("sound/lift", "analyze")
}

/// Calls `sound/lift/analyze` with evaluated PCM and keyword values.
pub fn call_sound_lift_analyze(cx: &mut Cx, args: Args) -> Result<Value> {
    let mut expressions = Vec::new();
    for value in args.into_vec() {
        expressions.push(value.object().as_expr(cx)?);
    }
    execute(cx, &expressions, false)
}

pub(crate) fn analyze_function_value(cx: &mut sim_kernel::LoadCx) -> Result<Value> {
    cx.factory().opaque(Arc::new(AnalyzeFunction))
}

struct AnalyzeFunction;

impl Object for AnalyzeFunction {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok("#<function sound/lift/analyze>".to_owned())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ObjectCompat for AnalyzeFunction {
    fn class(&self, cx: &mut Cx) -> Result<ClassRef> {
        cx.factory().class_stub(
            sim_kernel::CORE_FUNCTION_CLASS_ID,
            Symbol::qualified("core", "Function"),
        )
    }

    fn as_expr(&self, _cx: &mut Cx) -> Result<Expr> {
        Ok(Expr::Symbol(sound_lift_analyze_symbol()))
    }

    fn as_callable(&self) -> Option<&dyn Callable> {
        Some(self)
    }
}

impl Callable for AnalyzeFunction {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        call_sound_lift_analyze(cx, args)
    }

    fn call_exprs(&self, cx: &mut Cx, args: RawArgs) -> Result<Value> {
        execute(cx, args.exprs(), true)
    }

    fn browse_args_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("sound/lift/analyze", "args"),
            Arc::new(ListShape::variadic(
                vec![Arc::new(AnyShape)],
                Arc::new(AnyShape),
            )),
        )))
    }

    fn browse_result_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("sound/lift/analyze", "result"),
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
                "sound/lift/analyze received duplicate :{name}"
            )));
        }
    }
    reject_unknown(&parsed, &["features", "policy", "control"])?;
    let (samples, sample_rate) = parse_pcm(&pcm)?;
    let features = parsed
        .get("features")
        .ok_or_else(|| Error::Eval("sound/lift/analyze requires :features".to_owned()))?;
    let selection = parse_features(features)?;
    let mut plan = AudioAnalysisPlan::default();
    if let Some(policy) = parsed.get("policy") {
        parse_policy(policy, &mut plan)?;
    }
    let control = parsed
        .get("control")
        .map(parse_control)
        .transpose()?
        .unwrap_or_default();
    let report = analyze_audio(
        &samples,
        sample_rate,
        &EqualTemperament::default(),
        &selection,
        &plan,
        &control,
    )
    .map_err(|error| Error::Eval(error.to_string()))?;
    cx.factory().expr(audio_analysis_expr(&report))
}

fn parse_features(expr: &Expr) -> Result<AudioFeatureSelection> {
    let mut selection = AudioFeatureSelection::default();
    for feature in sequence(expr, "feature list")? {
        match symbolish(feature)?.as_str() {
            "onsets" => selection.onsets = true,
            "beats" => selection.beats = true,
            "zcr" | "zero-crossing" | "zero-crossing-rate" => {
                selection.zero_crossing_rate = true;
            }
            "mfcc" => selection.mfcc = true,
            "chroma" => selection.chroma = true,
            "key" => selection.key = true,
            "chords" => selection.chords = true,
            other => {
                return Err(Error::Eval(format!(
                    "sound/lift/analyze unsupported feature {other}"
                )));
            }
        }
    }
    Ok(selection)
}

fn parse_policy(expr: &Expr, plan: &mut AudioAnalysisPlan) -> Result<()> {
    let fields = map_fields(expr, "analysis policy map")?;
    reject_unknown(&fields, &["tempo", "key", "chords"])?;
    if let Some(tempo) = fields.get("tempo")
        && symbolish(tempo)? != "varying"
    {
        return Err(Error::Eval(
            "sound/lift/analyze supports only the evidence-preserving varying tempo policy"
                .to_owned(),
        ));
    }
    if let Some(key) = fields.get("key") {
        plan.key.strategy = match symbolish(key)?.as_str() {
            "posterior" => HarmonicDecodeStrategy::Posterior,
            "hmm" | "viterbi" => HarmonicDecodeStrategy::Viterbi,
            other => return Err(Error::Eval(format!("unsupported key policy {other}"))),
        };
    }
    if let Some(chords) = fields.get("chords") {
        plan.chords.strategy = match symbolish(chords)?.as_str() {
            "posterior" => HarmonicDecodeStrategy::Posterior,
            "hmm" | "viterbi" => HarmonicDecodeStrategy::Viterbi,
            other => return Err(Error::Eval(format!("unsupported chord policy {other}"))),
        };
    }
    Ok(())
}

fn parse_control(expr: &Expr) -> Result<AudioAnalysisControl> {
    let fields = map_fields(expr, "analysis control map")?;
    reject_unknown(&fields, &["work", "results", "seed"])?;
    let mut control = AudioAnalysisControl::default();
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

fn parse_pcm(expr: &Expr) -> Result<(Vec<f32>, u32)> {
    let fields = map_fields(expr, "PCM map with :samples and :sample-rate")?;
    reject_unknown(&fields, &["samples", "sample-rate"])?;
    let samples = fields
        .get("samples")
        .ok_or_else(|| Error::Eval("analysis PCM requires :samples".to_owned()))?;
    let sample_rate = fields
        .get("sample-rate")
        .ok_or_else(|| Error::Eval("analysis PCM requires :sample-rate".to_owned()))?;
    let samples = sequence(samples, "PCM :samples")?
        .iter()
        .map(|sample| {
            let value = scalar_text(sample)?
                .parse::<f64>()
                .map_err(|_| Error::Eval("invalid PCM sample".to_owned()))?;
            let narrowed = value as f32;
            if !narrowed.is_finite() {
                return Err(Error::Eval("PCM sample is outside f32 range".to_owned()));
            }
            Ok(narrowed)
        })
        .collect::<Result<Vec<_>>>()?;
    let sample_rate = scalar_text(sample_rate)?
        .parse::<u32>()
        .map_err(|_| Error::Eval("invalid PCM sample rate".to_owned()))?;
    if sample_rate == 0 {
        return Err(Error::Eval("PCM sample rate must be positive".to_owned()));
    }
    Ok((samples, sample_rate))
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
        return Err(Error::Eval(format!(
            "sound/lift/analyze unknown option :{name}"
        )));
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

fn usize_value(expr: &Expr, name: &str) -> Result<usize> {
    scalar_text(expr)?
        .parse()
        .map_err(|_| Error::Eval(format!("invalid {name}")))
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
                "analysis policy blocks require keyword/value pairs".to_owned(),
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
    Error::Eval("sound/lift/analyze expects PCM followed by keyword/value pairs".to_owned())
}
