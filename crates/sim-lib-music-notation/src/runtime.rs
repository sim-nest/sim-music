use sim_kernel::{
    Cx, ExportKind, ExportRecord, ExportState, Lib, LibManifest, Linker, LoadCx, Result, RuntimeId,
    Symbol,
};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

const MUSIC_NOTATION_LIB_ID: &str = "music-notation";
const EXPORT_KIND_NAME: &str = "NotationCodec";

/// Host-registered lib exporting the LilyPond subset notation-codec card, built
/// on the shared [`SurfacePackLib`] substrate.
pub struct MusicNotationLib;

impl Lib for MusicNotationLib {
    fn manifest(&self) -> LibManifest {
        music_notation_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        music_notation_pack().load(cx, linker)
    }
}

/// Installs the music-notation lib into `cx` and records its codec export.
///
/// Idempotent: returns early if the lib is already installed.
pub fn install_music_notation_lib(cx: &mut Cx) -> Result<()> {
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
                SurfaceField::Strs(vec!["music-core".to_owned(), "pitch-core".to_owned()]),
            ),
            (Symbol::new("lossless"), SurfaceField::Bool(false)),
            (Symbol::new("capabilities"), SurfaceField::Symbols(vec![])),
            (
                Symbol::new("surface"),
                SurfaceField::Str("lilypond-subset".to_owned()),
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
