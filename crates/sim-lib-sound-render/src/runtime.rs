use sim_kernel::{
    AbiVersion, Cx, Export, ExportKind, ExportRecord, ExportState, Lib, LibManifest, LibTarget,
    Linker, Result, RuntimeId, Symbol, Version,
};

const SOUND_RENDER_LIB_ID: &str = "sound-render";
const RENDER_EXPORT_KIND: &str = "Renderer";

/// Host-registered lib exporting the PCM renderer card.
pub struct SoundRenderLib;

impl Lib for SoundRenderLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(SOUND_RENDER_LIB_ID),
            version: Version(env!("CARGO_PKG_VERSION").to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: vec![Export::Value {
                symbol: Symbol::qualified("sound", "PcmRenderer"),
            }],
        }
    }

    fn load(&self, cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        linker.value(
            Symbol::qualified("sound", "PcmRenderer"),
            renderer_value(cx)?,
        )?;
        Ok(())
    }
}

/// Installs the sound-render lib into `cx`, registering the PCM renderer
/// export record (idempotent).
pub fn install_sound_render_lib(cx: &mut Cx) -> Result<()> {
    let lib = Symbol::new(SOUND_RENDER_LIB_ID);
    if !sim_lib_core::install_once(cx, &SoundRenderLib)? {
        return Ok(());
    }
    cx.registry_mut().append_export_record(
        &lib,
        ExportRecord {
            kind: ExportKind::named(RENDER_EXPORT_KIND),
            symbol: Symbol::qualified("sound", "PcmRenderer"),
            state: ExportState::Resolved {
                id: RuntimeId::Value,
            },
        },
    )?;
    Ok(())
}

fn renderer_value(cx: &mut sim_kernel::LoadCx) -> Result<sim_kernel::Value> {
    cx.factory().table(vec![
        (
            Symbol::new("symbol"),
            cx.factory()
                .symbol(Symbol::qualified("sound", "PcmRenderer"))?,
        ),
        (
            Symbol::new("layer"),
            cx.factory().string("sound".to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("operation".to_owned())?,
        ),
        (
            Symbol::new("shape"),
            cx.factory()
                .symbol(Symbol::qualified("sound", "PcmRenderer"))?,
        ),
        (
            Symbol::new("dependencies"),
            cx.factory().list(vec![
                cx.factory().string("sound-core".to_owned())?,
                cx.factory().string("sound-bridge".to_owned())?,
            ])?,
        ),
        (Symbol::new("lossless"), cx.factory().bool(false)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
    ])
}
