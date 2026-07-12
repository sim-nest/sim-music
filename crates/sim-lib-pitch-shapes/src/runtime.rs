use std::sync::Arc;

use sim_codec::parse_domain_form;
use sim_kernel::{
    AbiVersion, Cx, Export, Expr, Lib, LibManifest, LibTarget, Linker, Result, ShapeRef, Symbol,
    Value, Version,
};
use sim_shape::{
    ExactExprShape, ExprKind, ExprKindShape, OrShape, Shape, ShapeDoc, ShapeMatch,
    TableExtraPolicy, TableFieldSpec, TableShape, shape_value,
};

const PITCH_SHAPES_LIB_ID: &str = "pitch-shapes";

type ShapeSpec = (Symbol, &'static str, Vec<&'static str>, Arc<dyn Shape>);

/// The SIM runtime library that registers the pitch types as documented `Shape`s.
pub struct PitchShapesLib;

impl Lib for PitchShapesLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(PITCH_SHAPES_LIB_ID),
            version: Version(env!("CARGO_PKG_VERSION").to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: shape_specs()
                .into_iter()
                .map(|(symbol, _, _, _)| Export::Shape {
                    symbol,
                    shape_id: None,
                })
                .collect(),
        }
    }

    fn load(&self, _cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        for (symbol, name, details, inner) in shape_specs() {
            linker.shape_value(
                symbol.clone(),
                shape_value(symbol, Arc::new(DocumentedShape::new(name, details, inner))),
            )?;
        }
        Ok(())
    }
}

/// Installs the [`PitchShapesLib`] into `cx`; installing more than once is a no-op.
pub fn install_pitch_shapes_lib(cx: &mut Cx) -> Result<()> {
    if cx
        .registry()
        .lib(&Symbol::new(PITCH_SHAPES_LIB_ID))
        .is_some()
    {
        return Ok(());
    }
    cx.load_lib(&PitchShapesLib).map(|_| ())
}

fn shape_specs() -> Vec<ShapeSpec> {
    vec![
        (
            Symbol::qualified("pitch", "Pitch"),
            "Pitch",
            vec![
                "read-construct value for canonical chroma-plus-octave pitch",
                "reader sugar examples: C4 Eb5 F#3",
            ],
            text_shape(&[]),
        ),
        (
            Symbol::qualified("pitch", "Interval"),
            "Interval",
            vec![
                "read-construct value for named interval classes",
                "reader sugar examples: P5 m3 M7 TT",
            ],
            text_shape(&["Interval"]),
        ),
        (
            Symbol::qualified("pitch", "PitchClassMask"),
            "PitchClassMask",
            vec![
                "low-12-bit canonical pitch-class-set storage",
                "constructor form: #(PitchClassMask bits)",
            ],
            constructor_shape(&["PitchClassMask"]),
        ),
        (
            Symbol::qualified("pitch", "Scale"),
            "Scale",
            vec![
                "tonic-plus-mode read-construct shape",
                "codec helper surface uses tonic:mode strings",
            ],
            text_shape(&[]),
        ),
        (
            Symbol::qualified("pitch", "Key"),
            "Key",
            vec![
                "key context for roman/function naming",
                "constructor shape anchors scale degree analysis",
            ],
            text_shape(&[]),
        ),
        (
            Symbol::qualified("pitch", "Chord"),
            "Chord",
            vec![
                "rooted pitch-class collection with inversion/slash-bass support",
                "read-construct and chord-symbol helpers coexist",
            ],
            text_shape(&[]),
        ),
        (
            Symbol::qualified("pitch", "ChordSymbol"),
            "ChordSymbol",
            vec![
                "symbolic harmony surface routed through pitch-chord parsing",
                "codec helper provides string round-trips",
            ],
            text_shape(&[]),
        ),
    ]
}

struct DocumentedShape {
    name: &'static str,
    details: Vec<&'static str>,
    inner: Arc<dyn Shape>,
}

impl DocumentedShape {
    fn new(name: &'static str, details: Vec<&'static str>, inner: Arc<dyn Shape>) -> Self {
        Self {
            name,
            details,
            inner,
        }
    }
}

impl Shape for DocumentedShape {
    fn parents(&self, cx: &mut Cx) -> Result<Vec<ShapeRef>> {
        self.inner.parents(cx)
    }

    fn is_effectful(&self) -> bool {
        self.inner.is_effectful()
    }

    fn is_total(&self) -> bool {
        self.inner.is_total()
    }

    fn is_subshape_of(&self, cx: &mut Cx, parent: &dyn Shape) -> Result<Option<bool>> {
        self.inner.is_subshape_of(cx, parent)
    }

    fn check_value(&self, cx: &mut Cx, value: Value) -> Result<ShapeMatch> {
        self.inner.check_value(cx, value)
    }

    fn check_expr(&self, cx: &mut Cx, expr: &Expr) -> Result<ShapeMatch> {
        self.inner.check_expr(cx, expr)
    }

    fn describe(&self, _cx: &mut Cx) -> Result<ShapeDoc> {
        let mut doc = ShapeDoc::new(self.name);
        for detail in &self.details {
            doc = doc.with_detail(*detail);
        }
        Ok(doc)
    }
}

fn text_shape(constructors: &[&'static str]) -> Arc<dyn Shape> {
    Arc::new(TextSurfaceShape::new(constructors))
}

fn constructor_shape(constructors: &[&'static str]) -> Arc<dyn Shape> {
    Arc::new(DomainFormShape::new(constructors))
}

fn form_name_shape(name: &'static str) -> Arc<dyn Shape> {
    Arc::new(TableShape::new(
        vec![TableFieldSpec {
            key: Symbol::new("form"),
            shape: Arc::new(ExactExprShape::new(Expr::String(name.to_owned()))),
            required: true,
        }],
        TableExtraPolicy::Allow,
    ))
}

fn form_names_shape(names: &[&'static str]) -> Arc<dyn Shape> {
    let mut shapes = names
        .iter()
        .map(|name| form_name_shape(name))
        .collect::<Vec<_>>();
    if shapes.len() == 1 {
        shapes.remove(0)
    } else {
        Arc::new(OrShape::new(shapes))
    }
}

struct TextSurfaceShape {
    string: ExprKindShape,
    constructors: Option<Arc<dyn Shape>>,
}

impl TextSurfaceShape {
    fn new(constructors: &[&'static str]) -> Self {
        Self {
            string: ExprKindShape::new(ExprKind::String),
            constructors: (!constructors.is_empty()).then(|| form_names_shape(constructors)),
        }
    }
}

impl Shape for TextSurfaceShape {
    fn check_value(&self, cx: &mut Cx, value: Value) -> Result<ShapeMatch> {
        let expr = value.object().as_expr(cx)?;
        self.check_expr(cx, &expr)
    }

    fn check_expr(&self, cx: &mut Cx, expr: &Expr) -> Result<ShapeMatch> {
        let Expr::String(text) = expr else {
            return self.string.check_expr(cx, expr);
        };
        if !looks_like_domain_form(text) {
            return self.string.check_expr(cx, expr);
        }
        let Some(constructors) = &self.constructors else {
            return Ok(ShapeMatch::reject(
                "shape-domain-form: unexpected constructor form",
            ));
        };
        let map = match parse_domain_form(text) {
            Ok(form) => form.to_expr_map(),
            Err(error) => return Ok(ShapeMatch::reject(format!("shape-domain-form: {error:?}"))),
        };
        constructors.check_expr(cx, &map)
    }

    fn describe(&self, _cx: &mut Cx) -> Result<ShapeDoc> {
        Ok(ShapeDoc::new("text surface shape"))
    }
}

struct DomainFormShape {
    inner: Arc<dyn Shape>,
}

impl DomainFormShape {
    fn new(constructors: &[&'static str]) -> Self {
        Self {
            inner: form_names_shape(constructors),
        }
    }
}

impl Shape for DomainFormShape {
    fn is_effectful(&self) -> bool {
        self.inner.is_effectful()
    }

    fn check_value(&self, cx: &mut Cx, value: Value) -> Result<ShapeMatch> {
        let expr = value.object().as_expr(cx)?;
        self.check_expr(cx, &expr)
    }

    fn check_expr(&self, cx: &mut Cx, expr: &Expr) -> Result<ShapeMatch> {
        match expr {
            Expr::Map(_) => self.inner.check_expr(cx, expr),
            Expr::String(text) => {
                let map = match parse_domain_form(text) {
                    Ok(form) => form.to_expr_map(),
                    Err(error) => {
                        return Ok(ShapeMatch::reject(format!("shape-domain-form: {error:?}")));
                    }
                };
                self.inner.check_expr(cx, &map)
            }
            _ => Ok(ShapeMatch::reject(
                "shape-domain-form: expected constructor string or projected map",
            )),
        }
    }

    fn describe(&self, cx: &mut Cx) -> Result<ShapeDoc> {
        self.inner.describe(cx)
    }
}

fn looks_like_domain_form(text: &str) -> bool {
    text.trim_start().starts_with("#(")
}
