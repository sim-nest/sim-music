use std::sync::Arc;

use sim_kernel::{
    AbiVersion, Args, Callable, ClassRef, Cx, Error, Export, ExportKind, ExportRecord, ExportState,
    Expr, Lib, LibManifest, LibTarget, Linker, NumberLiteral, Object, ObjectCompat, RawArgs,
    Result, RuntimeId, ShapeRef, Symbol, Value, Version,
};
use sim_lib_music_shapes::{decode_score, install_music_shapes_lib};
use sim_shape::{AnyShape, ExactExprShape, ListShape, shape_value};

use crate::{
    ConsonancePolicy, ConsonanceReport, MetricReport, ProvenanceKind, SoundingNote, TimeSpan,
    WindowSonance, evaluate,
};

const LIB_ID: &str = "music-consonance";
const EXPORT_KIND: &str = "ConsonanceEvaluator";

/// Loadable exact consonance evaluator and Lisp report surface.
pub struct MusicConsonanceLib;

impl Lib for MusicConsonanceLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(LIB_ID),
            version: Version(env!("CARGO_PKG_VERSION").to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: vec![
                Export::Value {
                    symbol: evaluator_symbol(),
                },
                Export::Function {
                    symbol: music_consonance_evaluate_symbol(),
                    function_id: None,
                },
            ],
        }
    }

    fn load(&self, cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        linker.value(evaluator_symbol(), evaluator_value(cx)?)?;
        linker.function_value(
            music_consonance_evaluate_symbol(),
            cx.factory().opaque(Arc::new(ConsonanceEvaluateFunction))?,
        )?;
        Ok(())
    }
}

/// Installs the evaluator and its existing score, sonance, and tuning owners.
pub fn install_music_consonance_lib(cx: &mut Cx) -> Result<()> {
    install_music_shapes_lib(cx)?;
    sim_lib_pitch_dissonance::install_pitch_dissonance_lib(cx)?;
    sim_lib_sound_dissonance::install_sound_dissonance_lib(cx)?;
    sim_lib_sound_tuning::install_sound_tuning_lib(cx)?;
    if !sim_lib_core::install_once(cx, &MusicConsonanceLib)? {
        return Ok(());
    }
    cx.registry_mut().append_export_record(
        &Symbol::new(LIB_ID),
        ExportRecord {
            kind: ExportKind::named(EXPORT_KIND),
            symbol: evaluator_symbol(),
            state: ExportState::Resolved {
                id: RuntimeId::Value,
            },
        },
    )?;
    Ok(())
}

/// Symbol of the Shape-described Lisp consonance callable.
pub fn music_consonance_evaluate_symbol() -> Symbol {
    Symbol::qualified("music/consonance", "evaluate")
}

fn evaluator_symbol() -> Symbol {
    Symbol::qualified("music", "ConsonanceEvaluator")
}

fn evaluator_value(cx: &mut sim_kernel::LoadCx) -> Result<Value> {
    cx.factory().table(vec![
        (
            Symbol::new("symbol"),
            cx.factory().symbol(evaluator_symbol())?,
        ),
        (
            Symbol::new("layer"),
            cx.factory().string("music".to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("analysis".to_owned())?,
        ),
        (
            Symbol::new("shape"),
            cx.factory()
                .symbol(Symbol::qualified("music", "ConsonanceEvaluator"))?,
        ),
        (
            Symbol::new("dependencies"),
            cx.factory().list(
                [
                    "music-core",
                    "music-shapes",
                    "pitch-dissonance",
                    "sound-dissonance",
                    "sound-tuning",
                ]
                .into_iter()
                .map(|name| cx.factory().string(name.to_owned()))
                .collect::<Result<Vec<_>>>()?,
            )?,
        ),
        (Symbol::new("lossless"), cx.factory().bool(true)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
        (
            Symbol::new("callable"),
            cx.factory().symbol(music_consonance_evaluate_symbol())?,
        ),
        (
            Symbol::new("aggregation"),
            cx.factory().string("separate-metrics-only".to_owned())?,
        ),
    ])
}

struct ConsonanceEvaluateFunction;

impl Object for ConsonanceEvaluateFunction {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok("#<function music/consonance/evaluate>".to_owned())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for ConsonanceEvaluateFunction {
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

impl Callable for ConsonanceEvaluateFunction {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        let exprs = args
            .into_vec()
            .into_iter()
            .map(|value| value.object().as_expr(cx))
            .collect::<Result<Vec<_>>>()?;
        evaluate_call(cx, &exprs, false)
    }

    fn call_exprs(&self, cx: &mut Cx, args: RawArgs) -> Result<Value> {
        evaluate_call(cx, args.exprs(), true)
    }

    fn browse_args_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        let keyword = |name| {
            Arc::new(ExactExprShape::new(Expr::Symbol(Symbol::new(name))))
                as Arc<dyn sim_shape::Shape>
        };
        Ok(Some(shape_value(
            Symbol::qualified("music/consonance/evaluate", "args"),
            Arc::new(ListShape::tuple(vec![
                keyword(":score"),
                Arc::new(AnyShape),
                keyword(":policy"),
                Arc::new(AnyShape),
            ])),
        )))
    }

    fn browse_result_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("music/consonance/evaluate", "result"),
            Arc::new(AnyShape),
        )))
    }
}

fn evaluate_call(cx: &mut Cx, args: &[Expr], evaluate_values: bool) -> Result<Value> {
    let [score_key, score, policy_key, policy] = args else {
        return Err(Error::Eval(
            "music/consonance/evaluate expects :score SCORE :policy MAP".to_owned(),
        ));
    };
    expect_keyword(score_key, "score")?;
    expect_keyword(policy_key, "policy")?;
    let score = value_expr(cx, score, evaluate_values)?;
    let Expr::String(score) = unquote(score) else {
        return Err(Error::TypeMismatch {
            expected: "canonical #(Score ...) string",
            found: "non-string",
        });
    };
    let score = decode_score(&score)
        .map_err(|error| Error::Eval(format!("invalid consonance score: {error}")))?;
    let policy = value_expr(cx, policy, evaluate_values)?;
    let policy = parse_policy(&policy)?;
    let report = evaluate(&score, &policy).map_err(|error| Error::Eval(error.to_string()))?;
    cx.factory().expr(report_expr(&report))
}

fn parse_policy(expr: &Expr) -> Result<ConsonancePolicy> {
    let Expr::Map(entries) = unquote_ref(expr) else {
        return Err(Error::TypeMismatch {
            expected: "consonance policy map",
            found: "non-map",
        });
    };
    let mut policy = ConsonancePolicy::default();
    for (key, value) in entries {
        match keyword_name(key)?.as_str() {
            "duplicates" => {
                policy.contextual.duplicates = match symbolish(value)?.as_str() {
                    "retain" => sim_lib_pitch_dissonance::DuplicatePolicy::Retain,
                    "collapse" => sim_lib_pitch_dissonance::DuplicatePolicy::Collapse,
                    other => {
                        return Err(Error::Eval(format!(
                            "unknown consonance duplicate policy {other}"
                        )));
                    }
                };
            }
            "normalization" => {
                policy.contextual.normalization = match symbolish(value)?.as_str() {
                    "raw" => sim_lib_pitch_dissonance::SonanceNormalization::Raw,
                    "per-pair" => sim_lib_pitch_dissonance::SonanceNormalization::PerPair,
                    other => {
                        return Err(Error::Eval(format!(
                            "unknown consonance normalization {other}"
                        )));
                    }
                };
            }
            "pitch-models" => policy.pitch_models = symbol_list(value)?,
            "acoustic-models" => policy.acoustic_models = symbol_list(value)?,
            other => {
                return Err(Error::Eval(format!(
                    "unknown music/consonance policy :{other}"
                )));
            }
        }
    }
    Ok(policy)
}

fn report_expr(report: &ConsonanceReport) -> Expr {
    map(vec![
        (
            "provenance",
            map(vec![
                (
                    "kind",
                    symbol(match report.provenance.kind {
                        ProvenanceKind::Score => "score",
                        ProvenanceKind::Staff => "staff",
                        ProvenanceKind::MidiTimeline => "midi-timeline",
                    }),
                ),
                ("source", Expr::String(report.provenance.source.clone())),
                (
                    "identity-policy",
                    Expr::String(report.provenance.identity_policy.clone()),
                ),
                ("facts", strings(&report.provenance.facts)),
            ]),
        ),
        (
            "windows",
            Expr::Vector(report.windows.iter().map(window_expr).collect()),
        ),
        ("aggregate", Expr::Nil),
    ])
}

fn window_expr(report: &WindowSonance) -> Expr {
    map(vec![
        ("span", span_expr(&report.window.span)),
        (
            "notes",
            Expr::Vector(report.window.notes.iter().map(note_expr).collect()),
        ),
        ("pitch", metrics_expr(&report.pitch)),
        ("acoustic", metrics_expr(&report.acoustic)),
        ("ratio", metric_expr(&report.ratio)),
        ("commonality", metric_expr(&report.commonality)),
        ("leading", metric_expr(&report.leading)),
    ])
}

fn note_expr(note: &SoundingNote) -> Expr {
    map(vec![
        ("voice-id", Expr::String(note.voice_id.to_string())),
        ("note-id", Expr::String(note.note_id.to_string())),
        ("event-id", Expr::String(note.event_id.to_string())),
        (
            "pitch",
            Expr::String(format!(
                "{}{}",
                note.pitch.class.canonical_name(),
                note.pitch.octave
            )),
        ),
        ("onset", time_expr(note.onset)),
        ("release", time_expr(note.release)),
        ("velocity", integer(note.velocity)),
        ("channel", integer(note.channel.0)),
        (
            "articulation",
            symbol(&format!("{:?}", note.articulation).to_lowercase()),
        ),
        ("provenance", strings(&note.provenance)),
    ])
}

fn metrics_expr(metrics: &[MetricReport]) -> Expr {
    Expr::Vector(metrics.iter().map(metric_expr).collect())
}

fn metric_expr(metric: &MetricReport) -> Expr {
    map(vec![
        ("model", Expr::String(metric.model.clone())),
        ("roughness-mass", float(metric.roughness_mass)),
        ("normalized-density", float(metric.normalized_density)),
        ("harmonic-context", float(metric.harmonic_context)),
        ("normalization", Expr::String(metric.normalization.clone())),
        ("aggregation", Expr::String(metric.aggregation.clone())),
        ("evidence", strings(&metric.evidence)),
    ])
}

fn span_expr(span: &TimeSpan) -> Expr {
    map(vec![
        ("start", time_expr(span.start)),
        ("end", time_expr(span.end)),
    ])
}

fn time_expr(value: sim_lib_music_core::Time) -> Expr {
    Expr::String(format!("{}/{}", value.numer(), value.denom()))
}

fn strings(values: &[String]) -> Expr {
    Expr::Vector(values.iter().cloned().map(Expr::String).collect())
}

fn integer(value: impl ToString) -> Expr {
    number("i64", value.to_string())
}

fn float(value: f64) -> Expr {
    number("f64", value.to_string())
}

fn number(domain: &str, canonical: String) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", domain),
        canonical,
    })
}

fn map(entries: Vec<(&str, Expr)>) -> Expr {
    Expr::Map(
        entries
            .into_iter()
            .map(|(key, value)| (symbol(key), value))
            .collect(),
    )
}

fn symbol(value: &str) -> Expr {
    Expr::Symbol(Symbol::new(value))
}

fn value_expr(cx: &mut Cx, expr: &Expr, evaluate: bool) -> Result<Expr> {
    if evaluate {
        cx.eval_expr(expr.clone())?.object().as_expr(cx)
    } else {
        Ok(expr.clone())
    }
}

fn expect_keyword(expr: &Expr, expected: &str) -> Result<()> {
    if keyword_name(expr)? == expected {
        Ok(())
    } else {
        Err(Error::Eval(format!("expected :{expected}")))
    }
}

fn keyword_name(expr: &Expr) -> Result<String> {
    match unquote_ref(expr) {
        Expr::Symbol(symbol) => Ok(symbol
            .name
            .strip_prefix(':')
            .unwrap_or(symbol.name.as_ref())
            .to_owned()),
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

fn symbol_list(expr: &Expr) -> Result<Vec<String>> {
    match unquote_ref(expr) {
        Expr::List(values) | Expr::Vector(values) => values.iter().map(symbolish).collect(),
        _ => Err(Error::TypeMismatch {
            expected: "model list",
            found: "non-list",
        }),
    }
}

fn unquote(expr: Expr) -> Expr {
    match expr {
        Expr::Quote { expr, .. } => *expr,
        other => other,
    }
}

fn unquote_ref(expr: &Expr) -> &Expr {
    match expr {
        Expr::Quote { expr, .. } => expr,
        other => other,
    }
}
