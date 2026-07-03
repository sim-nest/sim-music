use sim_kernel::{
    AbiVersion, Cx, Export, Lib, LibManifest, LibTarget, Linker, Result, Symbol, Version,
};

use crate::daw_prelude_operations;

const DAW_SESSION_LIB_ID: &str = "daw-session";

/// Loadable library that registers the DAW session surface with a runtime.
pub struct DawSessionLib;

impl Lib for DawSessionLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(DAW_SESSION_LIB_ID),
            version: Version(env!("CARGO_PKG_VERSION").to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: daw_session_symbols()
                .into_iter()
                .map(|symbol| Export::Value { symbol })
                .collect(),
        }
    }

    fn load(&self, cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        for symbol in daw_session_symbols() {
            linker.value(symbol.clone(), daw_session_value(cx, symbol)?)?;
        }
        Ok(())
    }
}

/// Installs [`DawSessionLib`] into the context once, idempotently.
pub fn install_daw_session_lib(cx: &mut Cx) -> Result<()> {
    sim_lib_core::install_once(cx, &DawSessionLib).map(|_| ())
}

/// Returns the namespaced value symbols this library exports.
pub fn daw_session_symbols() -> Vec<Symbol> {
    [
        "DawSession",
        "DawInstrumentInstance",
        "DawInstrumentKind",
        "DawSessionRoute",
        "DawSessionRouteKind",
        "DawTrack",
        "DawClip",
        "DawPluginChain",
        "DawSave",
        "DawLoad",
        "DawRenderOffline",
        "DawBrowse",
        "DawTopologyPackage",
    ]
    .into_iter()
    .map(|name| Symbol::qualified(DAW_SESSION_LIB_ID, name))
    .collect()
}

fn daw_session_value(cx: &mut sim_kernel::LoadCx, symbol: Symbol) -> Result<sim_kernel::Value> {
    let operations = cx.factory().list(
        daw_prelude_operations()
            .into_iter()
            .map(|symbol| cx.factory().symbol(symbol))
            .collect::<Result<Vec<_>>>()?,
    )?;
    cx.factory().table(vec![
        (Symbol::new("symbol"), cx.factory().symbol(symbol)?),
        (
            Symbol::new("layer"),
            cx.factory().string(DAW_SESSION_LIB_ID.to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("daw-session-surface".to_owned())?,
        ),
        (
            Symbol::new("role"),
            cx.factory()
                .string("headless DAW session save/load/render/browse card".to_owned())?,
        ),
        (Symbol::new("lisp-operations"), operations),
    ])
}
