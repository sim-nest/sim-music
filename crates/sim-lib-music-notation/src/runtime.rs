//! Loadable notation profile and its single import callable.

use std::sync::Arc;

use sim_kernel::{
    Args, Callable, ClassRef, Cx, Error, Export, ExportKind, ExportRecord, ExportState, Expr, Lib,
    LibManifest, Linker, LoadCx, Object, ObjectCompat, RawArgs, Result, RuntimeId, ShapeRef,
    Symbol, Value,
};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};
use sim_lib_music_shapes::{MusicScoreDescriptor, encode_score, install_music_shapes_lib};
use sim_shape::{AnyShape, ExactExprShape, ListShape, shape_value};

use crate::{
    MusicXmlLimits, NotationIdentityKind, NotationLossKind, import_musicxml_partwise_report,
};

const MUSIC_NOTATION_LIB_ID: &str = "music-notation";
const EXPORT_KIND_NAME: &str = "NotationCodec";

/// Host-registered notation profile exporting browse metadata and the
/// Shape-described `music/notation/import` callable.
pub struct MusicNotationLib;

impl Lib for MusicNotationLib {
    fn manifest(&self) -> LibManifest {
        let mut manifest = music_notation_pack().manifest();
        manifest.exports.push(Export::Function {
            symbol: notation_import_symbol(),
            function_id: None,
        });
        manifest
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        music_notation_pack().load(cx, linker)?;
        linker.function_value(
            notation_import_symbol(),
            cx.factory().opaque(Arc::new(NotationImportFunction))?,
        )?;
        Ok(())
    }
}

/// Installs music shapes and the notation profile into `cx`.
///
/// Idempotent: returns early if the lib is already installed.
pub fn install_music_notation_lib(cx: &mut Cx) -> Result<()> {
    install_music_shapes_lib(cx)?;
    if !install_once(cx, &MusicNotationLib)? {
        return Ok(());
    }
    let lib = Symbol::new(MUSIC_NOTATION_LIB_ID);
    cx.registry_mut().append_export_record(
        &lib,
        ExportRecord {
            kind: ExportKind::named(EXPORT_KIND_NAME),
            symbol: notation_symbol(),
            state: ExportState::Resolved {
                id: RuntimeId::Value,
            },
        },
    )?;
    Ok(())
}

/// Symbol of the one notation import callable.
pub fn notation_import_symbol() -> Symbol {
    Symbol::qualified("music/notation", "import")
}

fn notation_symbol() -> Symbol {
    Symbol::qualified("music", "LilyPondSubsetCodec")
}

fn notation_value_spec() -> SurfaceValueSpec {
    SurfaceValueSpec {
        symbol: notation_symbol(),
        fields: vec![
            (
                Symbol::new("symbol"),
                SurfaceField::Symbol(notation_symbol()),
            ),
            (Symbol::new("layer"), SurfaceField::Str("music".to_owned())),
            (Symbol::new("kind"), SurfaceField::Str("plugin".to_owned())),
            (
                Symbol::new("shape"),
                SurfaceField::Symbol(Symbol::qualified("music", "NotationCodec")),
            ),
            (
                Symbol::new("dependencies"),
                SurfaceField::Strs(vec![
                    "music-core".to_owned(),
                    "music-shapes".to_owned(),
                    "pitch-core".to_owned(),
                ]),
            ),
            (Symbol::new("lossless"), SurfaceField::Bool(false)),
            (Symbol::new("capabilities"), SurfaceField::Symbols(vec![])),
            (
                Symbol::new("surface"),
                SurfaceField::Str("lilypond-subset,musicxml-partwise".to_owned()),
            ),
        ],
    }
}

fn music_notation_pack() -> SurfacePackLib {
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(MUSIC_NOTATION_LIB_ID),
            values: vec![notation_value_spec()],
        },
    }
}

struct NotationImportFunction;

impl Object for NotationImportFunction {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok("#<function music/notation/import>".to_owned())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for NotationImportFunction {
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

impl Callable for NotationImportFunction {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        let exprs = args
            .into_vec()
            .into_iter()
            .map(|value| value.object().as_expr(cx))
            .collect::<Result<Vec<_>>>()?;
        import_call(cx, &exprs, false)
    }

    fn call_exprs(&self, cx: &mut Cx, args: RawArgs) -> Result<Value> {
        import_call(cx, args.exprs(), true)
    }

    fn browse_args_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        let keyword = |name| {
            Arc::new(ExactExprShape::new(Expr::Symbol(Symbol::new(name))))
                as Arc<dyn sim_shape::Shape>
        };
        Ok(Some(shape_value(
            Symbol::qualified("music/notation/import", "args"),
            Arc::new(ListShape::tuple(vec![
                keyword(":format"),
                Arc::new(AnyShape),
                keyword(":source"),
                Arc::new(AnyShape),
                keyword(":limits"),
                Arc::new(AnyShape),
            ])),
        )))
    }

    fn browse_result_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("music/notation/import", "result"),
            Arc::new(AnyShape),
        )))
    }
}

fn import_call(cx: &mut Cx, args: &[Expr], evaluate_values: bool) -> Result<Value> {
    let [format_key, format, source_key, source, limits_key, limits] = args else {
        return Err(Error::Eval(
            "music/notation/import expects :format FORMAT :source BYTES :limits MAP".to_owned(),
        ));
    };
    expect_keyword(format_key, "format")?;
    expect_keyword(source_key, "source")?;
    expect_keyword(limits_key, "limits")?;
    if symbolish(format)? != "musicxml-partwise" {
        return Err(Error::Eval(
            "music/notation/import supports only 'musicxml-partwise".to_owned(),
        ));
    }
    let source = value_expr(cx, source, evaluate_values)?;
    let source = match source {
        Expr::Bytes(bytes) => bytes,
        Expr::String(text) => text.into_bytes(),
        other => {
            return Err(Error::TypeMismatch {
                expected: "MusicXML bytes or string",
                found: expr_kind(&other),
            });
        }
    };
    let limits = value_expr(cx, limits, evaluate_values)?;
    let limits = parse_limits(&limits)?;
    let report = import_musicxml_partwise_report(&source, limits)
        .map_err(|error| Error::Eval(error.to_string()))?;
    let score_form = encode_score(&report.value).map_err(|error| Error::Eval(error.to_string()))?;
    let score = MusicScoreDescriptor::read_construct_expr_from_text(&score_form)?;
    let identities = report
        .identities
        .into_iter()
        .map(|identity| {
            map(vec![
                (
                    "kind",
                    Expr::Symbol(Symbol::new(match identity.kind {
                        NotationIdentityKind::Part => "part",
                        NotationIdentityKind::Event => "event",
                    })),
                ),
                ("path", Expr::String(identity.canonical_path)),
                ("id", Expr::String(identity.xml_id)),
            ])
        })
        .collect();
    let losses = report
        .losses
        .into_iter()
        .map(|loss| {
            map(vec![
                (
                    "kind",
                    Expr::Symbol(Symbol::new(match loss.kind {
                        NotationLossKind::Clef => "clef",
                        NotationLossKind::PartName => "part-name",
                        NotationLossKind::PitchSpelling => "pitch-spelling",
                        NotationLossKind::DefaultedTempo => "defaulted-tempo",
                        NotationLossKind::DefaultedTimeSignature => "defaulted-time-signature",
                        NotationLossKind::Velocity => "velocity",
                        NotationLossKind::Channel => "channel",
                    })),
                ),
                ("path", loss.canonical_path.map_or(Expr::Nil, Expr::String)),
                ("detail", Expr::String(loss.detail)),
            ])
        })
        .collect();
    cx.factory().expr(map(vec![
        ("format", Expr::Symbol(Symbol::new("musicxml-partwise"))),
        ("score", score),
        ("identities", Expr::Vector(identities)),
        ("losses", Expr::Vector(losses)),
    ]))
}

fn value_expr(cx: &mut Cx, expr: &Expr, evaluate: bool) -> Result<Expr> {
    if evaluate {
        cx.eval_expr(expr.clone())?.object().as_expr(cx)
    } else {
        Ok(expr.clone())
    }
}

fn parse_limits(expr: &Expr) -> Result<MusicXmlLimits> {
    let Expr::Map(entries) = unquote_ref(expr) else {
        return Err(Error::TypeMismatch {
            expected: "MusicXML limits map",
            found: expr_kind(expr),
        });
    };
    let mut limits = MusicXmlLimits::default();
    for (key, value) in entries {
        let name = keyword_name(key)?;
        let parsed = usize_expr(value)?;
        match name.as_str() {
            "bytes" => limits.bytes = parsed,
            "nodes" => limits.nodes = parsed,
            "depth" => limits.depth = parsed,
            "text" => limits.text = parsed,
            "parts" => limits.parts = parsed,
            "events" => limits.events = parsed,
            other => {
                return Err(Error::Eval(format!(
                    "unknown music/notation/import limit :{other}"
                )));
            }
        }
    }
    Ok(limits)
}

fn expect_keyword(expr: &Expr, expected: &str) -> Result<()> {
    if keyword_name(expr)? == expected {
        Ok(())
    } else {
        Err(Error::Eval(format!(
            "music/notation/import expected :{expected}"
        )))
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
            found: expr_kind(expr),
        }),
    }
}

fn symbolish(expr: &Expr) -> Result<String> {
    match unquote_ref(expr) {
        Expr::Symbol(symbol) => Ok(symbol.name.to_string()),
        Expr::String(value) => Ok(value.clone()),
        _ => Err(Error::TypeMismatch {
            expected: "format symbol",
            found: expr_kind(expr),
        }),
    }
}

fn usize_expr(expr: &Expr) -> Result<usize> {
    let text = match unquote_ref(expr) {
        Expr::Number(number) => number.canonical.as_str(),
        Expr::String(value) => value.as_str(),
        _ => {
            return Err(Error::TypeMismatch {
                expected: "non-negative integer",
                found: expr_kind(expr),
            });
        }
    };
    text.parse()
        .map_err(|_| Error::Eval(format!("invalid MusicXML limit {text:?}")))
}

fn unquote_ref(expr: &Expr) -> &Expr {
    match expr {
        Expr::Quote { expr, .. } => expr,
        other => other,
    }
}

fn map(entries: Vec<(&str, Expr)>) -> Expr {
    Expr::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Expr::Symbol(Symbol::new(key)), value))
            .collect(),
    )
}

fn expr_kind(expr: &Expr) -> &'static str {
    match expr {
        Expr::Nil => "nil",
        Expr::Bool(_) => "bool",
        Expr::Number(_) => "number",
        Expr::Symbol(_) => "symbol",
        Expr::Local(_) => "local",
        Expr::String(_) => "string",
        Expr::Bytes(_) => "bytes",
        Expr::List(_) => "list",
        Expr::Vector(_) => "vector",
        Expr::Map(_) => "map",
        Expr::Set(_) => "set",
        Expr::Call { .. } => "call",
        Expr::Infix { .. } => "infix",
        Expr::Prefix { .. } => "prefix",
        Expr::Postfix { .. } => "postfix",
        Expr::Block(_) => "block",
        Expr::Quote { .. } => "quote",
        Expr::Annotated { .. } => "annotated",
        Expr::Extension { .. } => "extension",
    }
}
