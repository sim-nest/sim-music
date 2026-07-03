use std::sync::Arc;

use sim_kernel::{
    AbiVersion, Cx, Export, Lib, LibManifest, LibTarget, Linker, Result, Symbol, Version,
};
use sim_shape::{AnyShape, Shape, ShapeDoc, shape_value};

const PITCH_SHAPES_LIB_ID: &str = "pitch-shapes";

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

fn shape_specs() -> Vec<(Symbol, &'static str, Vec<&'static str>)> {
    vec![
        (
            Symbol::qualified("pitch", "Pitch"),
            "Pitch",
            vec![
                "read-construct value for canonical chroma-plus-octave pitch",
                "reader sugar examples: C4 Eb5 F#3",
            ],
        ),
        (
            Symbol::qualified("pitch", "Interval"),
            "Interval",
            vec![
                "read-construct value for named interval classes",
                "reader sugar examples: P5 m3 M7 TT",
            ],
        ),
        (
            Symbol::qualified("pitch", "PitchClassMask"),
            "PitchClassMask",
            vec![
                "low-12-bit canonical pitch-class-set storage",
                "constructor form: #(PitchClassMask bits)",
            ],
        ),
        (
            Symbol::qualified("pitch", "Scale"),
            "Scale",
            vec![
                "tonic-plus-mode read-construct shape",
                "codec helper surface uses tonic:mode strings",
            ],
        ),
        (
            Symbol::qualified("pitch", "Key"),
            "Key",
            vec![
                "key context for roman/function naming",
                "constructor shape anchors scale degree analysis",
            ],
        ),
        (
            Symbol::qualified("pitch", "Chord"),
            "Chord",
            vec![
                "rooted pitch-class collection with inversion/slash-bass support",
                "read-construct and chord-symbol helpers coexist",
            ],
        ),
        (
            Symbol::qualified("pitch", "ChordSymbol"),
            "ChordSymbol",
            vec![
                "symbolic harmony surface routed through pitch-chord parsing",
                "codec helper provides string round-trips",
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
