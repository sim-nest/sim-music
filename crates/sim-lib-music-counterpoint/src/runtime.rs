use std::sync::Arc;

use sim_kernel::{
    AbiVersion, Args, Callable, ClassRef, Cx, Error, Export, ExportKind, ExportRecord, ExportState,
    Expr, Lib, LibManifest, LibTarget, Linker, NumberLiteral, Object, ObjectCompat, RawArgs,
    Result, RuntimeId, ShapeRef, Symbol, Value, Version,
};
use sim_lib_music_core::Time;
use sim_lib_music_shapes::{decode_counterpoint, decode_melody, install_music_shapes_lib};
use sim_shape::{AnyShape, ExactExprShape, ListShape, shape_value};

use crate::runtime_generation::generate_call;
use crate::runtime_graph_expr::stretto_graph_expr;
use crate::{
    CounterpointReport, MetricEvidence, NoteEvidence, RuleSet, StrettoPolicy, StrettoTransform,
    TimeSpan, Violation, VoiceEvidence, analyze_counterpoint, stretto_graph,
};

const LIB_ID: &str = "music-counterpoint";
const EXPORT_KIND: &str = "CounterpointAnalyzer";

/// Loadable counterpoint and stretto analysis surface.
pub struct MusicCounterpointLib;

impl Lib for MusicCounterpointLib {
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
                    symbol: analyzer_symbol(),
                },
                Export::Function {
                    symbol: music_counterpoint_analyze_symbol(),
                    function_id: None,
                },
                Export::Function {
                    symbol: music_counterpoint_generate_symbol(),
                    function_id: None,
                },
                Export::Function {
                    symbol: music_stretto_graph_symbol(),
                    function_id: None,
                },
            ],
        }
    }

    fn load(&self, cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        linker.value(analyzer_symbol(), analyzer_value(cx)?)?;
        linker.function_value(
            music_counterpoint_analyze_symbol(),
            cx.factory()
                .opaque(Arc::new(CounterpointFunction::Analyze))?,
        )?;
        linker.function_value(
            music_counterpoint_generate_symbol(),
            cx.factory()
                .opaque(Arc::new(CounterpointFunction::Generate))?,
        )?;
        linker.function_value(
            music_stretto_graph_symbol(),
            cx.factory()
                .opaque(Arc::new(CounterpointFunction::Stretto))?,
        )?;
        Ok(())
    }
}

/// Installs the analysis surface and its existing music Shape owner.
pub fn install_music_counterpoint_lib(cx: &mut Cx) -> Result<()> {
    install_music_shapes_lib(cx)?;
    if !sim_lib_core::install_once(cx, &MusicCounterpointLib)? {
        return Ok(());
    }
    cx.registry_mut().append_export_record(
        &Symbol::new(LIB_ID),
        ExportRecord {
            kind: ExportKind::named(EXPORT_KIND),
            symbol: analyzer_symbol(),
            state: ExportState::Resolved {
                id: RuntimeId::Value,
            },
        },
    )?;
    Ok(())
}

/// Symbol of the Shape-described existing-counterpoint analyzer.
pub fn music_counterpoint_analyze_symbol() -> Symbol {
    Symbol::qualified("music/counterpoint", "analyze")
}

/// Symbol of the Shape-described stretto graph analyzer.
pub fn music_stretto_graph_symbol() -> Symbol {
    Symbol::qualified("music/stretto", "graph")
}

/// Symbol of bounded constraint counterpoint generation.
pub fn music_counterpoint_generate_symbol() -> Symbol {
    Symbol::qualified("music/counterpoint", "generate")
}

fn analyzer_symbol() -> Symbol {
    Symbol::qualified("music", "CounterpointAnalyzer")
}

fn analyzer_value(cx: &mut sim_kernel::LoadCx) -> Result<Value> {
    cx.factory().table(vec![
        (
            Symbol::new("symbol"),
            cx.factory().symbol(analyzer_symbol())?,
        ),
        (
            Symbol::new("layer"),
            cx.factory().string("music".to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("analysis-and-generation".to_owned())?,
        ),
        (
            Symbol::new("shape"),
            cx.factory()
                .symbol(Symbol::qualified("music", "CounterpointAnalyzer"))?,
        ),
        (
            Symbol::new("dependencies"),
            cx.factory().list(
                [
                    "music-core",
                    "music-consonance",
                    "music-transform",
                    "discrete-graph",
                    "discrete-search",
                ]
                .into_iter()
                .map(|name| cx.factory().string(name.to_owned()))
                .collect::<Result<Vec<_>>>()?,
            )?,
        ),
        (Symbol::new("lossless"), cx.factory().bool(true)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
        (
            Symbol::new("analysis-callable"),
            cx.factory().symbol(music_counterpoint_analyze_symbol())?,
        ),
        (
            Symbol::new("stretto-callable"),
            cx.factory().symbol(music_stretto_graph_symbol())?,
        ),
        (
            Symbol::new("generation-callable"),
            cx.factory().symbol(music_counterpoint_generate_symbol())?,
        ),
        (Symbol::new("generation"), cx.factory().bool(true)?),
    ])
}

enum CounterpointFunction {
    Analyze,
    Generate,
    Stretto,
}

impl Object for CounterpointFunction {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok(match self {
            Self::Analyze => "#<function music/counterpoint/analyze>",
            Self::Generate => "#<function music/counterpoint/generate>",
            Self::Stretto => "#<function music/stretto/graph>",
        }
        .to_owned())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for CounterpointFunction {
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

impl Callable for CounterpointFunction {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        let exprs = args
            .into_vec()
            .into_iter()
            .map(|value| value.object().as_expr(cx))
            .collect::<Result<Vec<_>>>()?;
        self.invoke(cx, &exprs, false)
    }

    fn call_exprs(&self, cx: &mut Cx, args: RawArgs) -> Result<Value> {
        self.invoke(cx, args.exprs(), true)
    }

    fn browse_args_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        let keyword = |name| {
            Arc::new(ExactExprShape::new(Expr::Symbol(Symbol::new(name))))
                as Arc<dyn sim_shape::Shape>
        };
        let fields = match self {
            Self::Analyze => vec![
                keyword(":counterpoint"),
                Arc::new(AnyShape),
                keyword(":rules"),
                Arc::new(AnyShape),
            ],
            Self::Generate => vec![
                Arc::new(AnyShape),
                keyword(":rules"),
                Arc::new(AnyShape),
                keyword(":voices"),
                Arc::new(AnyShape),
                keyword(":control"),
                Arc::new(AnyShape),
            ],
            Self::Stretto => vec![
                keyword(":subject"),
                Arc::new(AnyShape),
                keyword(":policy"),
                Arc::new(AnyShape),
            ],
        };
        Ok(Some(shape_value(
            match self {
                Self::Analyze => Symbol::qualified("music/counterpoint/analyze", "args"),
                Self::Generate => Symbol::qualified("music/counterpoint/generate", "args"),
                Self::Stretto => Symbol::qualified("music/stretto/graph", "args"),
            },
            Arc::new(ListShape::tuple(fields)),
        )))
    }

    fn browse_result_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            match self {
                Self::Analyze => Symbol::qualified("music/counterpoint/analyze", "result"),
                Self::Generate => Symbol::qualified("music/counterpoint/generate", "result"),
                Self::Stretto => Symbol::qualified("music/stretto/graph", "result"),
            },
            Arc::new(AnyShape),
        )))
    }
}

impl CounterpointFunction {
    fn invoke(&self, cx: &mut Cx, args: &[Expr], evaluate_values: bool) -> Result<Value> {
        match self {
            Self::Analyze => analyze_call(cx, args, evaluate_values),
            Self::Generate => generate_call(cx, args, evaluate_values),
            Self::Stretto => stretto_call(cx, args, evaluate_values),
        }
    }
}

fn analyze_call(cx: &mut Cx, args: &[Expr], evaluate_values: bool) -> Result<Value> {
    let [counterpoint_key, counterpoint, rules_key, rules] = args else {
        return Err(Error::Eval(
            "music/counterpoint/analyze expects :counterpoint STRING :rules SYMBOL".to_owned(),
        ));
    };
    expect_keyword(counterpoint_key, "counterpoint")?;
    expect_keyword(rules_key, "rules")?;
    let counterpoint = value_expr(cx, counterpoint, evaluate_values)?;
    let Expr::String(counterpoint) = unquote(counterpoint) else {
        return Err(Error::TypeMismatch {
            expected: "canonical #(Counterpoint ...) string",
            found: "non-string",
        });
    };
    let counterpoint = decode_counterpoint(&counterpoint)
        .map_err(|error| Error::Eval(format!("invalid counterpoint: {error}")))?;
    let rules = value_expr(cx, rules, evaluate_values)?;
    let rules = named_rules(&symbolish(&rules)?)?;
    cx.factory()
        .expr(counterpoint_report_expr(&analyze_counterpoint(
            &counterpoint,
            &rules,
        )))
}

fn stretto_call(cx: &mut Cx, args: &[Expr], evaluate_values: bool) -> Result<Value> {
    let [subject_key, subject, policy_key, policy] = args else {
        return Err(Error::Eval(
            "music/stretto/graph expects :subject STRING :policy MAP".to_owned(),
        ));
    };
    expect_keyword(subject_key, "subject")?;
    expect_keyword(policy_key, "policy")?;
    let subject = value_expr(cx, subject, evaluate_values)?;
    let Expr::String(subject) = unquote(subject) else {
        return Err(Error::TypeMismatch {
            expected: "canonical #(Melody ...) string",
            found: "non-string",
        });
    };
    let subject = decode_melody(&subject)
        .map_err(|error| Error::Eval(format!("invalid stretto subject: {error}")))?;
    let policy = value_expr(cx, policy, evaluate_values)?;
    let policy = parse_stretto_policy(&policy)?;
    let graph = stretto_graph(&subject, policy).map_err(|error| Error::Eval(error.to_string()))?;
    cx.factory().expr(stretto_graph_expr(&graph))
}

pub(crate) fn named_rules(name: &str) -> Result<RuleSet> {
    let pulse = Time::from_integer(1);
    match name {
        "species-one" => Ok(RuleSet::species_one(pulse)),
        "species-two" => Ok(RuleSet::species_two(pulse)),
        "species-three" => Ok(RuleSet::species_three(pulse)),
        "species-four" => Ok(RuleSet::species_four(pulse)),
        "open" => Ok(RuleSet::open()),
        other => Err(Error::Eval(format!(
            "unknown counterpoint rule set {other}"
        ))),
    }
}

fn parse_stretto_policy(expr: &Expr) -> Result<StrettoPolicy> {
    let Expr::Map(entries) = unquote_ref(expr) else {
        return Err(Error::TypeMismatch {
            expected: "stretto policy map",
            found: "non-map",
        });
    };
    let mut policy = StrettoPolicy::default();
    for (key, value) in entries {
        match keyword_name(key)?.as_str() {
            "delays" => policy.delays = scalar_list(value, parse_time)?,
            "transpositions" => {
                policy.transforms = scalar_list(value, parse_i32)?
                    .into_iter()
                    .map(StrettoTransform::original)
                    .collect();
            }
            "minimum-overlap" => policy.minimum_overlap = parse_time(value)?,
            "minimum-cluster-voices" => policy.minimum_cluster_voices = parse_usize(value)?,
            "max-entries" => policy.max_entries = parse_usize(value)?,
            "max-clusters" => policy.max_clusters = parse_usize(value)?,
            "max-chain-length" => policy.max_chain_length = parse_usize(value)?,
            "rules" => policy.compatibility_rules = named_rules(&symbolish(value)?)?,
            other => {
                return Err(Error::Eval(format!(
                    "unknown music/stretto policy :{other}"
                )));
            }
        }
    }
    Ok(policy)
}

fn counterpoint_report_expr(report: &CounterpointReport) -> Expr {
    map(vec![
        ("mode", Expr::String(report.provenance.mode.clone())),
        ("rule-set", Expr::String(report.provenance.rule_set.clone())),
        ("facts", strings(&report.provenance.facts)),
        (
            "alignment",
            Expr::Vector(
                report
                    .alignment
                    .iter()
                    .map(|window| {
                        map(vec![
                            ("span", span_expr(&window.span)),
                            (
                                "notes",
                                Expr::Vector(window.notes.iter().map(note_expr).collect()),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "motions",
            Expr::Vector(
                report
                    .motions
                    .iter()
                    .map(|motion| {
                        map(vec![
                            ("span", span_expr(&motion.span)),
                            (
                                "voices",
                                Expr::Vector(motion.voices.iter().map(voice_expr).collect()),
                            ),
                            (
                                "notes",
                                Expr::Vector(motion.notes.iter().map(note_expr).collect()),
                            ),
                            (
                                "directions",
                                Expr::Vector(vec![
                                    symbol(&format!("{:?}", motion.first).to_lowercase()),
                                    symbol(&format!("{:?}", motion.second).to_lowercase()),
                                ]),
                            ),
                            ("interval-before", integer(motion.interval_before)),
                            ("interval-after", integer(motion.interval_after)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "violations",
            Expr::Vector(report.violations.iter().map(violation_expr).collect()),
        ),
    ])
}

pub(crate) fn violation_expr(violation: &Violation) -> Expr {
    map(vec![
        ("rule", Expr::String(violation.rule.clone())),
        ("message", Expr::String(violation.message.clone())),
        ("span", span_expr(&violation.span)),
        (
            "voices",
            Expr::Vector(violation.voices.iter().map(voice_expr).collect()),
        ),
        (
            "notes",
            Expr::Vector(violation.notes.iter().map(note_expr).collect()),
        ),
        ("metric", metric_expr(&violation.metric)),
    ])
}

fn voice_expr(voice: &VoiceEvidence) -> Expr {
    map(vec![
        ("index", integer(voice.index)),
        ("id", Expr::String(voice.id.to_string())),
        ("name", Expr::String(voice.name.clone())),
    ])
}

fn note_expr(note: &NoteEvidence) -> Expr {
    map(vec![
        ("voice", voice_expr(&note.voice)),
        ("index", integer(note.index)),
        ("note-id", Expr::String(note.note_id.to_string())),
        ("event-id", Expr::String(note.event_id.to_string())),
        ("span", span_expr(&note.span)),
        (
            "pitch",
            Expr::String(format!(
                "{}{}",
                note.pitch.class.canonical_name(),
                note.pitch.octave
            )),
        ),
    ])
}

fn metric_expr(metric: &MetricEvidence) -> Expr {
    map(vec![
        ("name", Expr::String(metric.metric.clone())),
        ("observed", Expr::String(metric.observed.clone())),
        ("expected", Expr::String(metric.expected.clone())),
        ("unit", Expr::String(metric.unit.clone())),
        ("facts", strings(&metric.facts)),
    ])
}

pub(crate) fn span_expr(span: &TimeSpan) -> Expr {
    map(vec![
        ("start", time_expr(span.start)),
        ("end", time_expr(span.end)),
    ])
}

fn scalar_list<T>(expr: &Expr, parser: impl Fn(&Expr) -> Result<T>) -> Result<Vec<T>> {
    match unquote_ref(expr) {
        Expr::List(values) | Expr::Vector(values) => values.iter().map(parser).collect(),
        _ => Err(Error::TypeMismatch {
            expected: "list or vector",
            found: "non-list",
        }),
    }
}

fn parse_time(expr: &Expr) -> Result<Time> {
    let value = scalar_text(expr)?;
    let (numerator, denominator) = value.split_once('/').unwrap_or((&value, "1"));
    let numerator = numerator
        .parse::<i64>()
        .map_err(|_| Error::Eval(format!("invalid rational time {value}")))?;
    let denominator = denominator
        .parse::<i64>()
        .map_err(|_| Error::Eval(format!("invalid rational time {value}")))?;
    if denominator == 0 {
        return Err(Error::Eval("rational time denominator is zero".to_owned()));
    }
    Ok(Time::new(numerator, denominator))
}

fn parse_i32(expr: &Expr) -> Result<i32> {
    let value = scalar_text(expr)?;
    value
        .parse()
        .map_err(|_| Error::Eval(format!("invalid i32 {value}")))
}

pub(crate) fn parse_usize(expr: &Expr) -> Result<usize> {
    let value = scalar_text(expr)?;
    value
        .parse()
        .map_err(|_| Error::Eval(format!("invalid usize {value}")))
}

pub(crate) fn scalar_text(expr: &Expr) -> Result<String> {
    match unquote_ref(expr) {
        Expr::String(value) => Ok(value.clone()),
        Expr::Symbol(value) => Ok(value.name.to_string()),
        Expr::Number(value) => Ok(value.canonical.clone()),
        _ => Err(Error::TypeMismatch {
            expected: "string, symbol, or number",
            found: "compound expression",
        }),
    }
}

pub(crate) fn value_expr(cx: &mut Cx, expr: &Expr, evaluate: bool) -> Result<Expr> {
    if evaluate {
        cx.eval_expr(expr.clone())?.object().as_expr(cx)
    } else {
        Ok(expr.clone())
    }
}

pub(crate) fn expect_keyword(expr: &Expr, expected: &str) -> Result<()> {
    if keyword_name(expr)? == expected {
        Ok(())
    } else {
        Err(Error::Eval(format!("expected :{expected}")))
    }
}

pub(crate) fn keyword_name(expr: &Expr) -> Result<String> {
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

pub(crate) fn symbolish(expr: &Expr) -> Result<String> {
    match unquote_ref(expr) {
        Expr::Symbol(value) => Ok(value.name.to_string()),
        Expr::String(value) => Ok(value.clone()),
        _ => Err(Error::TypeMismatch {
            expected: "symbol or string",
            found: "other expression",
        }),
    }
}

pub(crate) fn unquote(expr: Expr) -> Expr {
    match expr {
        Expr::Quote { expr, .. } => *expr,
        other => other,
    }
}

pub(crate) fn unquote_ref(expr: &Expr) -> &Expr {
    match expr {
        Expr::Quote { expr, .. } => expr,
        other => other,
    }
}

pub(crate) fn strings(values: &[String]) -> Expr {
    Expr::Vector(values.iter().cloned().map(Expr::String).collect())
}

pub(crate) fn integer(value: impl ToString) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}

pub(crate) fn time_expr(value: Time) -> Expr {
    Expr::String(format!("{}/{}", value.numer(), value.denom()))
}

pub(crate) fn map(entries: Vec<(&str, Expr)>) -> Expr {
    Expr::Map(
        entries
            .into_iter()
            .map(|(key, value)| (symbol(key), value))
            .collect(),
    )
}

pub(crate) fn symbol(value: &str) -> Expr {
    Expr::Symbol(Symbol::new(value))
}
