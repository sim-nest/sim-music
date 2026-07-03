use std::sync::Arc;

use sim_kernel::{
    AbiVersion, Cx, Export, Lib, LibManifest, LibTarget, Linker, Result, Symbol, Version,
};
use sim_shape::{AnyShape, Shape, ShapeDoc, shape_value};

const MIDI_SHAPES_LIB_ID: &str = "midi-shapes";

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

fn shape_specs() -> Vec<(Symbol, &'static str, Vec<&'static str>)> {
    vec![
        (
            Symbol::qualified("midi", "TickTime"),
            "TickTime",
            vec![
                "absolute tick-rational MIDI time",
                "reader sugar examples: 2q 3/2q #(TickTime 960 480)",
            ],
        ),
        (
            Symbol::qualified("midi", "MidiEvent"),
            "MidiEvent",
            vec![
                "absolute-time MIDI event wrapper",
                "payloads use channel/meta/sysex/raw constructor families",
            ],
        ),
        (
            Symbol::qualified("midi", "ChannelMessage"),
            "ChannelMessage",
            vec![
                "typed channel-voice message family",
                "string codec helper round-trips all channel variants",
            ],
        ),
        (
            Symbol::qualified("midi", "MetaEvent"),
            "MetaEvent",
            vec![
                "SMF meta-event family with bucket preservation",
                "tempo, time signature, key signature, and other buckets supported",
            ],
        ),
        (
            Symbol::qualified("midi", "SysExEvent"),
            "SysExEvent",
            vec![
                "raw F0/F7 sys-ex packet shapes",
                "typed MTS helpers belong in midi-sysex",
            ],
        ),
        (
            Symbol::qualified("midi", "RawBytes"),
            "RawBytes",
            vec![
                "forward-compat raw status-byte bucket",
                "used for unknown safe round-tripped payloads",
            ],
        ),
        (
            Symbol::qualified("midi", "SmfTrack"),
            "SmfTrack",
            vec![
                "SMF absolute-time track container",
                "shape crate provides string round-trips for constructor-like payloads",
            ],
        ),
        (
            Symbol::qualified("midi", "SmfFile"),
            "SmfFile",
            vec![
                "SMF file model covering formats 0, 1, and 2",
                "surface shape documents tpq plus track collection",
            ],
        ),
        (
            Symbol::qualified("midi", "MidiSourceFactory"),
            "MidiSourceFactory",
            vec![
                "host-registered plugin row for MIDI source builders",
                "used by browse/help metadata rather than stream transport itself",
            ],
        ),
        (
            Symbol::qualified("midi", "MidiSinkFactory"),
            "MidiSinkFactory",
            vec![
                "host-registered plugin row for MIDI sink builders",
                "tracks fallible sink traits without forcing a transport choice",
            ],
        ),
        (
            Symbol::qualified("midi", "TrackedMidiSourceFactory"),
            "TrackedMidiSourceFactory",
            vec![
                "plugin row for track-aware MIDI source builders",
                "reports last-track and n-tracks metadata at runtime",
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
