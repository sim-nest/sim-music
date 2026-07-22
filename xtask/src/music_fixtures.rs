//! Regenerates the synth render-fixture manifests.
//!
//! These maintainer codegen tools live in `xtask` so fixture refresh stays out
//! of product binary surfaces. Run with
//! `cargo run -p xtask -- music-fixtures <dx7|ps3300|system55|system700>`.

use std::fs;
use std::path::Path;

use sim_lib_music_synth::{
    DX7_RENDER_FIXTURE_MANIFEST_PATH, PS3300_RENDER_FIXTURE_MANIFEST_PATH,
    SYSTEM55_RENDER_FIXTURE_MANIFEST_PATH, SYSTEM700_RENDER_FIXTURE_MANIFEST_PATH,
    dx7_render_fixture_manifest, ps3300_render_fixture_manifest, system55_render_fixture_manifest,
    system700_render_fixture_manifest,
};

pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("dx7") => write_manifest(
            DX7_RENDER_FIXTURE_MANIFEST_PATH,
            dx7_render_fixture_manifest(),
        ),
        Some("ps3300") => write_manifest(
            PS3300_RENDER_FIXTURE_MANIFEST_PATH,
            ps3300_render_fixture_manifest(),
        ),
        Some("system55") => write_manifest(
            SYSTEM55_RENDER_FIXTURE_MANIFEST_PATH,
            system55_render_fixture_manifest(),
        ),
        Some("system700") => write_manifest(
            SYSTEM700_RENDER_FIXTURE_MANIFEST_PATH,
            system700_render_fixture_manifest(),
        ),
        Some(other) => Err(format!("unknown music-fixtures target: {other}")),
        None => Err("usage: xtask music-fixtures <dx7|ps3300|system55|system700>".to_owned()),
    }
}

fn write_manifest(path: &str, contents: String) -> Result<(), String> {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
    }
    fs::write(path, contents).map_err(|err| format!("write {}: {err}", path.display()))?;
    println!("music-fixtures: wrote {}", path.display());
    Ok(())
}
