use std::sync::Arc;

use sim_codec::parse_domain_form;
use sim_kernel::{
    AbiVersion, Cx, Export, Expr, Lib, LibManifest, LibTarget, Linker, Result, ShapeRef, Symbol,
    Value, Version,
};
use sim_shape::{
    ExactExprShape, ExprKind, ExprKindShape, Shape, ShapeDoc, ShapeMatch, TableExtraPolicy,
    TableFieldSpec, TableShape, shape_value,
};

const MUSIC_SHAPES_LIB_ID: &str = "music-shapes";

type ShapeSpec = (Symbol, &'static str, Vec<&'static str>, Arc<dyn Shape>);

/// Loadable library that registers documented `Shape` values for the `music`
/// namespace.
///
/// Each registered shape carries browse/help metadata and structural checks for
/// the music form it guards.
pub struct MusicShapesLib;

impl Lib for MusicShapesLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(MUSIC_SHAPES_LIB_ID),
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

/// Loads [`MusicShapesLib`] into `cx`, returning early if it is already present.
pub fn install_music_shapes_lib(cx: &mut Cx) -> Result<()> {
    if cx
        .registry()
        .lib(&Symbol::new(MUSIC_SHAPES_LIB_ID))
        .is_some()
    {
        return Ok(());
    }
    cx.load_lib(&MusicShapesLib).map(|_| ())
}

fn shape_specs() -> Vec<ShapeSpec> {
    vec![
        (
            Symbol::qualified("music", "Time"),
            "Time",
            vec![
                "exact whole-note rational duration",
                "codec helper uses numer/denom text",
            ],
            text_shape(),
        ),
        (
            Symbol::qualified("music", "Note"),
            "Note",
            vec![
                "read-construct note atom",
                "encodes duration, pitch, velocity, channel, articulation",
            ],
            domain_form_shape(
                "Note",
                vec![
                    atom_field("dur"),
                    atom_field("pitch"),
                    atom_field("vel"),
                    atom_field("channel"),
                    atom_field("articulation"),
                ],
            ),
        ),
        (
            Symbol::qualified("music", "Rest"),
            "Rest",
            vec!["read-construct silent span", "encodes duration only"],
            domain_form_shape("Rest", vec![atom_field("dur")]),
        ),
        (
            Symbol::qualified("music", "Par"),
            "Par",
            vec![
                "parallel music-object composition",
                "children share the same onset",
            ],
            domain_form_shape("Par", vec![list_field("children")]),
        ),
        (
            Symbol::qualified("music", "Seq"),
            "Seq",
            vec![
                "sequential music-object composition",
                "children advance by cumulative duration",
            ],
            domain_form_shape("Seq", vec![list_field("children")]),
        ),
        (
            Symbol::qualified("music", "Chord"),
            "Chord",
            vec![
                "parallel note voicing with symbolic label",
                "encodes voicing pitches and channel",
            ],
            domain_form_shape(
                "Chord",
                vec![
                    atom_field("dur"),
                    string_field("symbol"),
                    list_field("pitches"),
                    atom_field("vel"),
                    atom_field("channel"),
                ],
            ),
        ),
        (
            Symbol::qualified("music", "Melody"),
            "Melody",
            vec![
                "strict monophonic note/rest stream",
                "constructor validates non-negative spans",
            ],
            domain_form_shape("Melody", vec![list_field("items")]),
        ),
        (
            Symbol::qualified("music", "Progression"),
            "Progression",
            vec![
                "sequenced chord stream",
                "optional key field is analytic context only",
            ],
            domain_form_shape(
                "Progression",
                vec![string_field("key"), list_field("chords")],
            ),
        ),
        (
            Symbol::qualified("music", "Counterpoint"),
            "Counterpoint",
            vec![
                "parallel named melody voices",
                "voice names default when omitted",
            ],
            domain_form_shape(
                "Counterpoint",
                vec![list_field("voice_names"), list_field("voices")],
            ),
        ),
        (
            Symbol::qualified("music", "PianoRoll"),
            "PianoRoll",
            vec![
                "arbitrary timed notes",
                "canonical lowering substrate for transforms",
            ],
            domain_form_shape("PianoRoll", vec![list_field("items")]),
        ),
        (
            Symbol::qualified("music", "Arranger"),
            "Arranger",
            vec![
                "places playable music objects on lanes",
                "encodes stretch, T/I/R, pitch remap, filters, seed, and trace policy",
            ],
            domain_form_shape(
                "Arranger",
                vec![list_field("lanes"), list_field("placements")],
            ),
        ),
        (
            Symbol::qualified("music", "CustomFilter"),
            "CustomFilter",
            vec![
                "shape-checked custom filter object",
                "declares capabilities, determinism, trace policy, and rule or callable body",
            ],
            domain_form_shape(
                "CustomFilter",
                vec![
                    string_field("id"),
                    form_field("input"),
                    form_field("output"),
                    list_field("caps"),
                    atom_field("determinism"),
                    atom_field("trace"),
                    form_field("body"),
                ],
            ),
        ),
        (
            Symbol::qualified("music", "RetrogradeMode"),
            "RetrogradeMode",
            vec![
                "polyphonic retrograde interpretation selector",
                "built-ins are Cutout and PinnedNoteOn",
            ],
            domain_form_shape("RetrogradeMode", vec![atom_field("value")]),
        ),
        (
            Symbol::qualified("music", "FunctionMap"),
            "FunctionMap",
            vec![
                "scale-degree remapping surface for modal transforms",
                "built-ins plus Custom tonic/mode map",
            ],
            domain_form_shape("FunctionMap", vec![atom_field("kind")]),
        ),
        (
            Symbol::qualified("music", "DiffFrame"),
            "DiffFrame",
            vec![
                "time-sliced note-state analysis frame",
                "stores sounding, started, ended, and slurred masks",
            ],
            domain_form_shape(
                "DiffFrame",
                vec![
                    atom_field("at"),
                    atom_field("sounding"),
                    atom_field("started"),
                    atom_field("ended"),
                    atom_field("slurred"),
                ],
            ),
        ),
        (
            Symbol::qualified("music", "DiffRoll"),
            "DiffRoll",
            vec![
                "analysis view derived from PianoRoll note boundaries",
                "not a replacement for PianoRoll storage",
            ],
            domain_form_shape("DiffRoll", vec![list_field("frames")]),
        ),
        (
            Symbol::qualified("music", "ChordWindowMode"),
            "ChordWindowMode",
            vec![
                "analysis selector for sounding vs starting-note windows",
                "controls chord extraction from PianoRoll and DiffRoll",
            ],
            domain_form_shape("ChordWindowMode", vec![atom_field("value")]),
        ),
        (
            Symbol::qualified("music", "ChordWindow"),
            "ChordWindow",
            vec![
                "mask-based chord analysis result over a time window",
                "stores pitch-range, pitch-class, and BitChord views",
            ],
            domain_form_shape(
                "ChordWindow",
                vec![
                    atom_field("at"),
                    atom_field("until"),
                    atom_field("mode"),
                    atom_field("range"),
                    atom_field("pitch_classes"),
                    atom_field("root"),
                ],
            ),
        ),
        (
            Symbol::qualified("music", "LabelStrategy"),
            "LabelStrategy",
            vec![
                "progression-lifter chord-label selection policy",
                "built-ins are Functional, JazzChord, and SetClass",
            ],
            domain_form_shape("LabelStrategy", vec![atom_field("value")]),
        ),
        (
            Symbol::qualified("music", "ProgressionLiftOpts"),
            "ProgressionLiftOpts",
            vec![
                "MIDI-to-progression lifter options",
                "encodes grid, min_notes, key_hint, label strategy, and window mode",
            ],
            domain_form_shape(
                "ProgressionLiftOpts",
                vec![
                    atom_field("grid"),
                    atom_field("min_notes"),
                    atom_field("key_hint"),
                    atom_field("label_strategy"),
                    atom_field("window_mode"),
                ],
            ),
        ),
        (
            Symbol::qualified("music", "VoiceAssignment"),
            "VoiceAssignment",
            vec![
                "counterpoint-lifter voice splitting policy",
                "supports channel-only, track-first, highest-first, and lowest-first modes",
            ],
            domain_form_shape("VoiceAssignment", vec![atom_field("value")]),
        ),
        (
            Symbol::qualified("music", "CounterpointLiftOpts"),
            "CounterpointLiftOpts",
            vec![
                "MIDI-to-counterpoint lifter options",
                "encodes rest closing threshold, voice cap, and assignment mode",
            ],
            domain_form_shape(
                "CounterpointLiftOpts",
                vec![
                    atom_field("min_rest_to_close"),
                    atom_field("max_voices_per_track"),
                    atom_field("voice_assignment"),
                ],
            ),
        ),
        (
            Symbol::qualified("music", "MidiTrackObj"),
            "MidiTrackObj",
            vec![
                "music-object wrapper over one MIDI event track",
                "codec stores embedded midi event shapes",
            ],
            domain_form_shape(
                "MidiTrackObj",
                vec![atom_field("channel_hint"), list_field("events")],
            ),
        ),
        (
            Symbol::qualified("music", "MidiFileObj"),
            "MidiFileObj",
            vec![
                "music-object wrapper over an SMF value",
                "codec stores embedded smf shape text",
            ],
            domain_form_shape("MidiFileObj", vec![string_field("smf")]),
        ),
        (
            Symbol::qualified("music", "Score"),
            "Score",
            vec![
                "root .music read-construct form",
                "pairs body with default tempo, meter, and key metadata",
            ],
            domain_form_shape(
                "Score",
                vec![
                    atom_field("tempo"),
                    atom_field("time_sig"),
                    string_field("key"),
                    form_field("body"),
                ],
            ),
        ),
        (
            Symbol::qualified("music", "NotationCodec"),
            "NotationCodec",
            vec![
                "host-registered conservative LilyPond import/export codec",
                "browse/help metadata names the subset and lossiness contract",
            ],
            domain_form_shape("NotationCodec", Vec::new()),
        ),
    ]
}

fn text_shape() -> Arc<dyn Shape> {
    Arc::new(ExprKindShape::new(ExprKind::String))
}

fn domain_form_shape(name: &'static str, fields: Vec<TableFieldSpec>) -> Arc<dyn Shape> {
    Arc::new(DomainFormShape::new(form_shape(name, fields)))
}

fn form_shape(name: &'static str, mut fields: Vec<TableFieldSpec>) -> Arc<dyn Shape> {
    fields.insert(
        0,
        TableFieldSpec {
            key: Symbol::new("form"),
            shape: Arc::new(ExactExprShape::new(Expr::String(name.to_owned()))),
            required: true,
        },
    );
    Arc::new(TableShape::new(fields, TableExtraPolicy::Allow))
}

fn string_field(key: &'static str) -> TableFieldSpec {
    field(key, Arc::new(ExprKindShape::new(ExprKind::String)))
}

fn atom_field(key: &'static str) -> TableFieldSpec {
    string_field(key)
}

fn form_field(key: &'static str) -> TableFieldSpec {
    field(key, Arc::new(ExprKindShape::new(ExprKind::Map)))
}

fn list_field(key: &'static str) -> TableFieldSpec {
    field(key, Arc::new(ExprKindShape::new(ExprKind::List)))
}

fn field(key: &'static str, shape: Arc<dyn Shape>) -> TableFieldSpec {
    TableFieldSpec {
        key: Symbol::new(key),
        shape,
        required: true,
    }
}

struct DomainFormShape {
    inner: Arc<dyn Shape>,
}

impl DomainFormShape {
    fn new(inner: Arc<dyn Shape>) -> Self {
        Self { inner }
    }
}

impl Shape for DomainFormShape {
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
