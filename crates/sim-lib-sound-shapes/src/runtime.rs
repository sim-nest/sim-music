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

const SOUND_SHAPES_LIB_ID: &str = "sound-shapes";

type ShapeSpec = (Symbol, &'static str, Vec<&'static str>, Arc<dyn Shape>);

/// Host-registered lib exporting the sound-shape definitions.
pub struct SoundShapesLib;

impl Lib for SoundShapesLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(SOUND_SHAPES_LIB_ID),
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

/// Installs the sound-shapes lib into `cx`, registering the sound-shape
/// definitions (idempotent).
pub fn install_sound_shapes_lib(cx: &mut Cx) -> Result<()> {
    if cx
        .registry()
        .lib(&Symbol::new(SOUND_SHAPES_LIB_ID))
        .is_some()
    {
        return Ok(());
    }
    cx.load_lib(&SoundShapesLib).map(|_| ())
}

fn shape_specs() -> Vec<ShapeSpec> {
    vec![
        (
            Symbol::qualified("sound", "Frequency"),
            "Frequency",
            vec!["positive hertz scalar"],
            domain_form_shape("Frequency", vec![atom_field("hz")]),
        ),
        (
            Symbol::qualified("sound", "Amplitude"),
            "Amplitude",
            vec!["linear gain scalar"],
            domain_form_shape("Amplitude", vec![atom_field("linear")]),
        ),
        (
            Symbol::qualified("sound", "Phase"),
            "Phase",
            vec!["radians modulo tau"],
            domain_form_shape("Phase", vec![atom_field("radians")]),
        ),
        (
            Symbol::qualified("sound", "Partial"),
            "Partial",
            vec!["frequency/amplitude/phase/tag component"],
            domain_form_shape(
                "Partial",
                vec![
                    form_field("frequency"),
                    form_field("amplitude"),
                    form_field("phase"),
                    optional_form_field("tag"),
                ],
            ),
        ),
        (
            Symbol::qualified("sound", "EnvelopeShape"),
            "EnvelopeShape",
            vec!["linear, exponential, or named custom shape"],
            domain_form_shape("EnvelopeShape", vec![atom_field("kind")]),
        ),
        (
            Symbol::qualified("sound", "Envelope"),
            "Envelope",
            vec!["attack/decay/sustain/release envelope"],
            domain_form_shape(
                "Envelope",
                vec![
                    atom_field("attack"),
                    atom_field("decay"),
                    atom_field("sustain"),
                    atom_field("release"),
                    form_field("shape"),
                ],
            ),
        ),
        (
            Symbol::qualified("sound", "Tone"),
            "Tone",
            vec!["finite set of partials plus envelope and duration"],
            domain_form_shape(
                "Tone",
                vec![
                    list_field("partials"),
                    form_field("envelope"),
                    atom_field("duration"),
                ],
            ),
        ),
        (
            Symbol::qualified("sound", "SpectrumSource"),
            "SpectrumSource",
            vec!["synthetic, tone snapshot, or pcm window provenance"],
            domain_form_shape("SpectrumSource", vec![atom_field("kind")]),
        ),
        (
            Symbol::qualified("sound", "Spectrum"),
            "Spectrum",
            vec!["frequency/amplitude bins with source metadata"],
            domain_form_shape("Spectrum", vec![list_field("bins"), form_field("source")]),
        ),
        (
            Symbol::qualified("sound", "AttackKind"),
            "AttackKind",
            vec!["soft, plucked, bowed, or struck onset category"],
            domain_form_shape("AttackKind", vec![atom_field("kind")]),
        ),
        (
            Symbol::qualified("sound", "TimbreMeta"),
            "TimbreMeta",
            vec!["brightness, roughness, attack, and category metadata"],
            domain_form_shape(
                "TimbreMeta",
                vec![
                    atom_field("brightness"),
                    atom_field("roughness"),
                    form_field("attack_kind"),
                    string_field("category"),
                ],
            ),
        ),
        (
            Symbol::qualified("sound", "Filter"),
            "Filter",
            vec!["low/high/band/notch/formant partial-domain filter"],
            domain_form_shape("Filter", vec![atom_field("kind")]),
        ),
        (
            Symbol::qualified("sound", "TimbreRecipe"),
            "TimbreRecipe",
            vec!["serialization-friendly timbre recipe tree"],
            domain_form_shape("TimbreRecipe", vec![atom_field("kind")]),
        ),
        (
            Symbol::qualified("sound", "Timbre"),
            "Timbre",
            vec!["tone-construction recipe with default envelope and filters"],
            domain_form_shape(
                "Timbre",
                vec![
                    string_field("name"),
                    form_field("recipe"),
                    form_field("envelope"),
                    form_field("meta"),
                    list_field("filters"),
                ],
            ),
        ),
        (
            Symbol::qualified("sound", "PitchClassN"),
            "PitchClassN",
            vec!["exploratory non-12 chroma class with explicit division count"],
            domain_form_shape(
                "PitchClassN",
                vec![atom_field("divisions"), atom_field("index")],
            ),
        ),
        (
            Symbol::qualified("sound", "Tuning"),
            "Tuning",
            vec!["runtime tuning plugin export kind"],
            domain_form_shape("Tuning", Vec::new()),
        ),
        (
            Symbol::qualified("sound", "TuningDescriptor"),
            "TuningDescriptor",
            vec!["lossless read-construct descriptor for built-in tunings"],
            domain_form_shape("TuningDescriptor", vec![atom_field("kind")]),
        ),
        (
            Symbol::qualified("sound", "DissonanceModel"),
            "DissonanceModel",
            vec!["runtime sound dissonance plugin export kind"],
            domain_form_shape("DissonanceModel", Vec::new()),
        ),
        (
            Symbol::qualified("sound", "DissonanceModelDescriptor"),
            "DissonanceModelDescriptor",
            vec!["lossless read-construct descriptor for spectral dissonance models"],
            domain_form_shape("DissonanceModelDescriptor", vec![atom_field("kind")]),
        ),
        (
            Symbol::qualified("sound", "BridgeOptions"),
            "BridgeOptions",
            vec!["polyphony and bend-range configuration for MIDI-to-sound bridging"],
            domain_form_shape(
                "BridgeOptions",
                vec![
                    atom_field("polyphony_limit"),
                    atom_field("bend_range_cents"),
                ],
            ),
        ),
        (
            Symbol::qualified("sound", "TimbreBank"),
            "TimbreBank",
            vec!["fallback timbre plus bank/program lookup entries"],
            domain_form_shape(
                "TimbreBank",
                vec![form_field("fallback"), list_field("entries")],
            ),
        ),
        (
            Symbol::qualified("sound", "PcmRenderer"),
            "PcmRenderer",
            vec!["runtime PCM renderer plugin surface"],
            domain_form_shape("PcmRenderer", Vec::new()),
        ),
        (
            Symbol::qualified("sound", "RendererOptions"),
            "RendererOptions",
            vec!["sample-rate and channel-count render configuration"],
            domain_form_shape(
                "RendererOptions",
                vec![atom_field("sample_rate"), atom_field("channels")],
            ),
        ),
        (
            Symbol::qualified("sound", "AudioLifter"),
            "AudioLifter",
            vec!["runtime audio-to-pitch lifter plugin surface"],
            domain_form_shape("AudioLifter", Vec::new()),
        ),
        (
            Symbol::qualified("sound", "AudioLiftOptions"),
            "AudioLiftOptions",
            vec!["window, peak, and note-tracking controls for audio lifting"],
            domain_form_shape(
                "AudioLiftOptions",
                vec![
                    atom_field("window_size"),
                    atom_field("hop_size"),
                    atom_field("max_peaks"),
                    atom_field("min_peak_ratio"),
                    atom_field("harmonic_tolerance_cents"),
                    atom_field("min_note_confidence"),
                    atom_field("min_note_windows"),
                ],
            ),
        ),
        (
            Symbol::qualified("sound", "PitchCandidate"),
            "PitchCandidate",
            vec!["per-frame lifted pitch estimate with confidence and cents error"],
            domain_form_shape(
                "PitchCandidate",
                vec![
                    atom_field("semitone"),
                    form_field("frequency"),
                    form_field("amplitude"),
                    atom_field("confidence"),
                    atom_field("cents_error"),
                    atom_field("harmonic_count"),
                ],
            ),
        ),
        (
            Symbol::qualified("sound", "AudioLiftFrame"),
            "AudioLiftFrame",
            vec!["pcm-window spectrum snapshot with lifted pitch candidates"],
            domain_form_shape(
                "AudioLiftFrame",
                vec![
                    atom_field("index"),
                    atom_field("onset_sample"),
                    atom_field("duration_samples"),
                    form_field("spectrum"),
                    list_field("pitch_candidates"),
                    list_field("diagnostics"),
                ],
            ),
        ),
        (
            Symbol::qualified("sound", "AudioNoteCandidate"),
            "AudioNoteCandidate",
            vec!["stitched note-track estimate recovered from audio frames"],
            domain_form_shape(
                "AudioNoteCandidate",
                vec![
                    atom_field("track"),
                    atom_field("onset_sample"),
                    atom_field("duration_samples"),
                    atom_field("sample_rate"),
                    atom_field("semitone"),
                    form_field("mean_frequency"),
                    form_field("mean_amplitude"),
                    atom_field("confidence"),
                    list_field("diagnostics"),
                ],
            ),
        ),
    ]
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

fn optional_form_field(key: &'static str) -> TableFieldSpec {
    let mut spec = form_field(key);
    spec.required = false;
    spec
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
