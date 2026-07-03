#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

use sim_lib_music_synth::{
    SYSTEM55_RENDER_FIXTURE_MANIFEST_PATH, system55_render_fixture_manifest,
};

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let path = Path::new(SYSTEM55_RENDER_FIXTURE_MANIFEST_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
    }
    fs::write(path, system55_render_fixture_manifest())
        .map_err(|err| format!("write {}: {err}", path.display()))?;
    println!("system55-fixtures: wrote {}", path.display());
    Ok(())
}
