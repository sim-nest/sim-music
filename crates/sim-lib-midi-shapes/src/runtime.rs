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

const MIDI_SHAPES_LIB_ID: &str = "midi-shapes";

type ShapeSpec = (Symbol, &'static str, Vec<&'static str>, Arc<dyn Shape>);

/// Host-registered lib that publishes the `midi/*` shape values (TickTime,
/// MidiEvent, ChannelMessage, MetaEvent, SysExEvent, RawBytes, SmfTrack,
/// SmfFile, and the I/O factory rows) into a running runtime.
pub struct MidiShapesLib;

impl Lib for MidiShapesLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(MIDI_SHAPES_LIB_ID),
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

/// Loads [`MidiShapesLib`] into `cx`, doing nothing if it is already present.
pub fn install_midi_shapes_lib(cx: &mut Cx) -> Result<()> {
    if cx
        .registry()
        .lib(&Symbol::new(MIDI_SHAPES_LIB_ID))
        .is_some()
    {
        return Ok(());
    }
    cx.load_lib(&MidiShapesLib).map(|_| ())
}

fn shape_specs() -> Vec<ShapeSpec> {
    vec![
        (
            Symbol::qualified("midi", "TickTime"),
            "TickTime",
            vec![
                "absolute tick-rational MIDI time",
                "reader sugar examples: 2q 3/2q #(TickTime 960 480)",
            ],
            text_shape(&["TickTime"]),
        ),
        (
            Symbol::qualified("midi", "MidiEvent"),
            "MidiEvent",
            vec![
                "absolute-time MIDI event wrapper",
                "payloads use channel/meta/sysex/raw constructor families",
            ],
            constructor_shape(&["MidiEvent"]),
        ),
        (
            Symbol::qualified("midi", "ChannelMessage"),
            "ChannelMessage",
            vec![
                "typed channel-voice message family",
                "string codec helper round-trips all channel variants",
            ],
            constructor_shape(&["Channel"]),
        ),
        (
            Symbol::qualified("midi", "MetaEvent"),
            "MetaEvent",
            vec![
                "SMF meta-event family with bucket preservation",
                "tempo, time signature, key signature, and other buckets supported",
            ],
            constructor_shape(&["Meta"]),
        ),
        (
            Symbol::qualified("midi", "SysExEvent"),
            "SysExEvent",
            vec![
                "raw F0/F7 sys-ex packet shapes",
                "typed MTS helpers belong in midi-sysex",
            ],
            constructor_shape(&["SysEx"]),
        ),
        (
            Symbol::qualified("midi", "RawBytes"),
            "RawBytes",
            vec![
                "forward-compat raw status-byte bucket",
                "used for unknown safe round-tripped payloads",
            ],
            constructor_shape(&["Raw"]),
        ),
        (
            Symbol::qualified("midi", "SmfTrack"),
            "SmfTrack",
            vec![
                "SMF absolute-time track container",
                "shape crate provides string round-trips for constructor-like payloads",
            ],
            constructor_shape(&["SmfTrack"]),
        ),
        (
            Symbol::qualified("midi", "SmfFile"),
            "SmfFile",
            vec![
                "SMF file model covering formats 0, 1, and 2",
                "surface shape documents tpq plus track collection",
            ],
            constructor_shape(&["SmfFile"]),
        ),
        (
            Symbol::qualified("midi", "MidiSourceFactory"),
            "MidiSourceFactory",
            vec![
                "host-registered plugin row for MIDI source builders",
                "used by browse/help metadata rather than stream transport itself",
            ],
            constructor_shape(&["MidiSourceFactory"]),
        ),
        (
            Symbol::qualified("midi", "MidiSinkFactory"),
            "MidiSinkFactory",
            vec![
                "host-registered plugin row for MIDI sink builders",
                "tracks fallible sink traits without forcing a transport choice",
            ],
            constructor_shape(&["MidiSinkFactory"]),
        ),
        (
            Symbol::qualified("midi", "TrackedMidiSourceFactory"),
            "TrackedMidiSourceFactory",
            vec![
                "plugin row for track-aware MIDI source builders",
                "reports last-track and n-tracks metadata at runtime",
            ],
            constructor_shape(&["TrackedMidiSourceFactory"]),
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
