use std::sync::Arc;

use sim_kernel::{
    AbiVersion, Cx, Export, Lib, LibManifest, LibTarget, Linker, Result, Symbol, Version,
};
use sim_shape::{AnyShape, Shape, ShapeDoc, shape_value};

const SOUND_SHAPES_LIB_ID: &str = "sound-shapes";

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

fn shape_specs() -> Vec<(Symbol, &'static str, Vec<&'static str>)> {
    vec![
        (
            Symbol::qualified("sound", "Frequency"),
            "Frequency",
            vec!["positive hertz scalar"],
        ),
        (
            Symbol::qualified("sound", "Amplitude"),
            "Amplitude",
            vec!["linear gain scalar"],
        ),
        (
            Symbol::qualified("sound", "Phase"),
            "Phase",
            vec!["radians modulo tau"],
        ),
        (
            Symbol::qualified("sound", "Partial"),
            "Partial",
            vec!["frequency/amplitude/phase triple"],
        ),
        (
            Symbol::qualified("sound", "EnvelopeShape"),
            "EnvelopeShape",
            vec!["linear, exponential, or named custom shape"],
        ),
        (
            Symbol::qualified("sound", "Envelope"),
            "Envelope",
            vec!["attack/decay/sustain/release envelope"],
        ),
        (
            Symbol::qualified("sound", "Tone"),
            "Tone",
            vec!["finite set of partials plus envelope and duration"],
        ),
        (
            Symbol::qualified("sound", "SpectrumSource"),
            "SpectrumSource",
            vec!["synthetic, tone snapshot, or pcm window provenance"],
        ),
        (
            Symbol::qualified("sound", "Spectrum"),
            "Spectrum",
            vec!["frequency/amplitude bins with source metadata"],
        ),
        (
            Symbol::qualified("sound", "AttackKind"),
            "AttackKind",
            vec!["soft, plucked, bowed, or struck onset category"],
        ),
        (
            Symbol::qualified("sound", "TimbreMeta"),
            "TimbreMeta",
            vec!["brightness, roughness, attack, and category metadata"],
        ),
        (
            Symbol::qualified("sound", "Filter"),
            "Filter",
            vec!["low/high/band/notch/formant partial-domain filter"],
        ),
        (
            Symbol::qualified("sound", "TimbreRecipe"),
            "TimbreRecipe",
            vec!["serialization-friendly timbre recipe tree"],
        ),
        (
            Symbol::qualified("sound", "Timbre"),
            "Timbre",
            vec!["tone-construction recipe with default envelope and filters"],
        ),
        (
            Symbol::qualified("sound", "PitchClassN"),
            "PitchClassN",
            vec!["exploratory non-12 chroma class with explicit division count"],
        ),
        (
            Symbol::qualified("sound", "Tuning"),
            "Tuning",
            vec!["runtime tuning plugin export kind"],
        ),
        (
            Symbol::qualified("sound", "TuningDescriptor"),
            "TuningDescriptor",
            vec!["lossless read-construct descriptor for built-in tunings"],
        ),
        (
            Symbol::qualified("sound", "DissonanceModel"),
            "DissonanceModel",
            vec!["runtime sound dissonance plugin export kind"],
        ),
        (
            Symbol::qualified("sound", "DissonanceModelDescriptor"),
            "DissonanceModelDescriptor",
            vec!["lossless read-construct descriptor for spectral dissonance models"],
        ),
        (
            Symbol::qualified("sound", "BridgeOptions"),
            "BridgeOptions",
            vec!["polyphony and bend-range configuration for MIDI-to-sound bridging"],
        ),
        (
            Symbol::qualified("sound", "TimbreBank"),
            "TimbreBank",
            vec!["fallback timbre plus bank/program lookup entries"],
        ),
        (
            Symbol::qualified("sound", "PcmRenderer"),
            "PcmRenderer",
            vec!["runtime PCM renderer plugin surface"],
        ),
        (
            Symbol::qualified("sound", "RendererOptions"),
            "RendererOptions",
            vec!["sample-rate and channel-count render configuration"],
        ),
        (
            Symbol::qualified("sound", "AudioLifter"),
            "AudioLifter",
            vec!["runtime audio-to-pitch lifter plugin surface"],
        ),
        (
            Symbol::qualified("sound", "AudioLiftOptions"),
            "AudioLiftOptions",
            vec!["window, peak, and note-tracking controls for audio lifting"],
        ),
        (
            Symbol::qualified("sound", "PitchCandidate"),
            "PitchCandidate",
            vec!["per-frame lifted pitch estimate with confidence and cents error"],
        ),
        (
            Symbol::qualified("sound", "AudioLiftFrame"),
            "AudioLiftFrame",
            vec!["pcm-window spectrum snapshot with lifted pitch candidates"],
        ),
        (
            Symbol::qualified("sound", "AudioNoteCandidate"),
            "AudioNoteCandidate",
            vec!["stitched note-track estimate recovered from audio frames"],
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
