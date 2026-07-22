//! Thin simdoc launcher: defers to the shared sim-tooling Card encoder.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run(args: Vec<String>) -> Result<(), String> {
    run_repo_tool(args, "simdoc")
}

pub fn run_repo_tool(args: Vec<String>, command_name: &str) -> Result<(), String> {
    let program = args.first().map(String::as_str).unwrap_or("xtask");
    if args.get(1).map(String::as_str) != Some(command_name) {
        return Err(format!("usage: {program} {command_name} [--check]"));
    }

    let root = env::current_dir().map_err(|err| format!("current dir: {err}"))?;
    let manifest = locate_sim_tooling_manifest(&root)?;
    let mut command = Command::new("cargo");
    command.args(["run", "--manifest-path"]);
    command.arg(manifest);
    command.args(["--quiet", "--", command_name]);
    if command_name == "simdoc" {
        command.arg("--repo-root");
        command.arg(&root);
    } else if !has_repo_arg(&args) {
        command.arg("--repo");
        command.arg(&root);
    }
    for arg in args.iter().skip(2) {
        command.arg(arg);
    }

    let status = command
        .status()
        .map_err(|err| format!("run shared {command_name} tool: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "shared {command_name} tool failed with status {status}"
        ))
    }
}

fn has_repo_arg(args: &[String]) -> bool {
    args.iter()
        .skip(2)
        .any(|arg| arg == "--repo" || arg.starts_with("--repo="))
}

fn locate_sim_tooling_manifest(repo_root: &Path) -> Result<PathBuf, String> {
    if let Ok(path) = env::var("SIMDOC_TOOLING_MANIFEST") {
        return Ok(PathBuf::from(path));
    }
    let sibling = repo_root
        .parent()
        .unwrap_or(repo_root)
        .join("sim-tooling")
        .join("Cargo.toml");
    if sibling.is_file() {
        return Ok(sibling);
    }
    Err("set SIMDOC_TOOLING_MANIFEST to the sim-tooling Cargo.toml".to_owned())
}
