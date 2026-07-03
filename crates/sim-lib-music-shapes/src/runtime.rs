use std::sync::Arc;

use sim_kernel::{
    AbiVersion, Cx, Export, Lib, LibManifest, LibTarget, Linker, Result, Symbol, Version,
};
use sim_shape::{AnyShape, Shape, ShapeDoc, shape_value};

const MUSIC_SHAPES_LIB_ID: &str = "music-shapes";

/// Loadable library that registers documented `Shape` values for the `music`
/// namespace.
///
/// Each registered shape carries browse/help metadata describing the music
/// type it guards; the shapes themselves delegate matching to the total
/// `AnyShape`.
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
                .map(|(symbol, _, _)| Export::Shape {
                    symbol,
                    shape_id: None,
                })
                .collect(),
        }
    }

    fn load(&self, _cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        for (symbol, name, details) in shape_specs() {
            linker.shape_value(
                symbol.clone(),
                shape_value(symbol, Arc::new(DocumentedShape::new(name, details))),
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

fn shape_specs() -> Vec<(Symbol, &'static str, Vec<&'static str>)> {
    vec![
        (
            Symbol::qualified("music", "Time"),
            "Time",
            vec![
                "exact whole-note rational duration",
                "codec helper uses numer/denom text",
            ],
        ),
        (
            Symbol::qualified("music", "Note"),
            "Note",
            vec![
                "read-construct note atom",
                "encodes duration, pitch, velocity, channel, articulation",
            ],
        ),
        (
            Symbol::qualified("music", "Rest"),
            "Rest",
            vec!["read-construct silent span", "encodes duration only"],
        ),
        (
            Symbol::qualified("music", "Par"),
            "Par",
            vec![
                "parallel music-object composition",
                "children share the same onset",
            ],
        ),
        (
            Symbol::qualified("music", "Seq"),
            "Seq",
            vec![
                "sequential music-object composition",
                "children advance by cumulative duration",
            ],
        ),
        (
            Symbol::qualified("music", "Chord"),
            "Chord",
            vec![
                "parallel note voicing with symbolic label",
                "encodes voicing pitches and channel",
            ],
        ),
        (
            Symbol::qualified("music", "Melody"),
            "Melody",
            vec![
                "strict monophonic note/rest stream",
                "constructor validates non-negative spans",
            ],
        ),
        (
            Symbol::qualified("music", "Progression"),
            "Progression",
            vec![
                "sequenced chord stream",
                "optional key field is analytic context only",
            ],
        ),
        (
            Symbol::qualified("music", "Counterpoint"),
            "Counterpoint",
            vec![
                "parallel named melody voices",
                "voice names default when omitted",
            ],
        ),
        (
            Symbol::qualified("music", "PianoRoll"),
            "PianoRoll",
            vec![
                "arbitrary timed notes",
                "canonical lowering substrate for transforms",
            ],
        ),
        (
            Symbol::qualified("music", "Arranger"),
            "Arranger",
            vec![
                "places playable music objects on lanes",
                "encodes stretch, T/I/R, pitch remap, filters, seed, and trace policy",
            ],
        ),
        (
            Symbol::qualified("music", "CustomFilter"),
            "CustomFilter",
            vec![
                "shape-checked custom filter object",
                "declares capabilities, determinism, trace policy, and rule or callable body",
            ],
        ),
        (
            Symbol::qualified("music", "RetrogradeMode"),
            "RetrogradeMode",
            vec![
                "polyphonic retrograde interpretation selector",
                "built-ins are Cutout and PinnedNoteOn",
            ],
        ),
        (
            Symbol::qualified("music", "FunctionMap"),
            "FunctionMap",
            vec![
                "scale-degree remapping surface for modal transforms",
                "built-ins plus Custom tonic/mode map",
            ],
        ),
        (
            Symbol::qualified("music", "DiffFrame"),
            "DiffFrame",
            vec![
                "time-sliced note-state analysis frame",
                "stores sounding, started, ended, and slurred masks",
            ],
        ),
        (
            Symbol::qualified("music", "DiffRoll"),
            "DiffRoll",
            vec![
                "analysis view derived from PianoRoll note boundaries",
                "not a replacement for PianoRoll storage",
            ],
        ),
        (
            Symbol::qualified("music", "ChordWindowMode"),
            "ChordWindowMode",
            vec![
                "analysis selector for sounding vs starting-note windows",
                "controls chord extraction from PianoRoll and DiffRoll",
            ],
        ),
        (
            Symbol::qualified("music", "ChordWindow"),
            "ChordWindow",
            vec![
                "mask-based chord analysis result over a time window",
                "stores pitch-range, pitch-class, and BitChord views",
            ],
        ),
        (
            Symbol::qualified("music", "LabelStrategy"),
            "LabelStrategy",
            vec![
                "progression-lifter chord-label selection policy",
                "built-ins are Functional, JazzChord, and SetClass",
            ],
        ),
        (
            Symbol::qualified("music", "ProgressionLiftOpts"),
            "ProgressionLiftOpts",
            vec![
                "MIDI-to-progression lifter options",
                "encodes grid, min_notes, key_hint, label strategy, and window mode",
            ],
        ),
        (
            Symbol::qualified("music", "VoiceAssignment"),
            "VoiceAssignment",
            vec![
                "counterpoint-lifter voice splitting policy",
                "supports channel-only, track-first, highest-first, and lowest-first modes",
            ],
        ),
        (
            Symbol::qualified("music", "CounterpointLiftOpts"),
            "CounterpointLiftOpts",
            vec![
                "MIDI-to-counterpoint lifter options",
                "encodes rest closing threshold, voice cap, and assignment mode",
            ],
        ),
        (
            Symbol::qualified("music", "MidiTrackObj"),
            "MidiTrackObj",
            vec![
                "music-object wrapper over one MIDI event track",
                "codec stores embedded midi event shapes",
            ],
        ),
        (
            Symbol::qualified("music", "MidiFileObj"),
            "MidiFileObj",
            vec![
                "music-object wrapper over an SMF value",
                "codec stores embedded smf shape text",
            ],
        ),
        (
            Symbol::qualified("music", "Score"),
            "Score",
            vec![
                "root .music read-construct form",
                "pairs body with default tempo, meter, and key metadata",
            ],
        ),
        (
            Symbol::qualified("music", "NotationCodec"),
            "NotationCodec",
            vec![
                "host-registered conservative LilyPond import/export codec",
                "browse/help metadata names the subset and lossiness contract",
            ],
        ),
    ]
}

struct DocumentedShape {
    name: &'static str,
    details: Vec<&'static str>,
}

impl DocumentedShape {
    fn new(name: &'static str, details: Vec<&'static str>) -> Self {
        Self { name, details }
    }
}

impl Shape for DocumentedShape {
    fn is_total(&self) -> bool {
        AnyShape.is_total()
    }

    fn check_value(
        &self,
        cx: &mut sim_kernel::Cx,
        value: sim_kernel::Value,
    ) -> Result<sim_shape::ShapeMatch> {
        AnyShape.check_value(cx, value)
    }

    fn check_expr(
        &self,
        cx: &mut sim_kernel::Cx,
        expr: &sim_kernel::Expr,
    ) -> Result<sim_shape::ShapeMatch> {
        AnyShape.check_expr(cx, expr)
    }

    fn describe(&self, _cx: &mut sim_kernel::Cx) -> Result<ShapeDoc> {
        let mut doc = ShapeDoc::new(self.name);
        for detail in &self.details {
            doc = doc.with_detail(*detail);
        }
        Ok(doc)
    }
}
