#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

use sim_lib_music_synth::{PS3300_RENDER_FIXTURE_MANIFEST_PATH, ps3300_render_fixture_manifest};

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let path = Path::new(PS3300_RENDER_FIXTURE_MANIFEST_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
    }
    fs::write(path, ps3300_render_fixture_manifest())
        .map_err(|err| format!("write {}: {err}", path.display()))?;
    println!("ps3300-fixtures: wrote {}", path.display());
    Ok(())
}
