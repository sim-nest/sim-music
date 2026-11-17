// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Thin simdoc launcher.
//! The simdoc verb runs the isolated encoder. Other verbs keep the tooling
//! workspace manifest.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run(args: Vec<String>) -> Result<(), String> {
    run_repo_tool(args, "simdoc")
}

pub fn run_repo_tool(args: Vec<String>, command_name: &str) -> Result<(), String> {
    if command_name == "simdoc" {
        return run_isolated_simdoc(args);
    }
    run_workspace_tool(args, command_name)
}

fn run_isolated_simdoc(args: Vec<String>) -> Result<(), String> {
    let program = args.first().map(String::as_str).unwrap_or("xtask");
    if args.get(1).map(String::as_str) != Some("simdoc") {
        return Err(format!("usage: {program} simdoc [--check]"));
    }

    let root = env::current_dir().map_err(|err| format!("current dir: {err}"))?;
    let hint = tooling_hint_from_env();
    let mut command = match select_simdoc(&root, hint.as_deref())? {
        SimdocLaunch::Binary(binary) => {
            let mut command = Command::new(binary);
            command.arg("simdoc");
            command
        }
        SimdocLaunch::Manifest(manifest) => {
            let mut command = Command::new("cargo");
            command.args(["run", "--locked", "--offline", "--manifest-path"]);
            command.arg(manifest);
            command.args(["--quiet", "--", "simdoc"]);
            command
        }
    };
    configure_local_origin(&mut command);
    command.arg("--repo-root");
    command.arg(&root);
    for arg in args.iter().skip(2) {
        command.arg(arg);
    }
    finish(command, "isolated simdoc encoder")
}

fn run_workspace_tool(args: Vec<String>, command_name: &str) -> Result<(), String> {
    let program = args.first().map(String::as_str).unwrap_or("xtask");
    if args.get(1).map(String::as_str) != Some(command_name) {
        return Err(format!("usage: {program} {command_name} [--check]"));
    }

    let root = env::current_dir().map_err(|err| format!("current dir: {err}"))?;
    let hint = tooling_hint_from_env();
    let manifest = select_workspace_manifest(&root, hint.as_deref())?;
    let mut command = Command::new("cargo");
    command.args(["run", "--manifest-path"]);
    command.arg(manifest);
    command.args(["--quiet", "--", command_name]);
    if !has_repo_arg(&args) {
        command.arg("--repo");
        command.arg(&root);
    }
    for arg in args.iter().skip(2) {
        command.arg(arg);
    }
    finish(command, &format!("shared {command_name} tool"))
}

fn finish(mut command: Command, label: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|err| format!("run {label}: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{label} failed with status {status}"))
    }
}

fn configure_local_origin(command: &mut Command) {
    command.env("GIT_CONFIG_COUNT", "1");
    command.env("GIT_CONFIG_KEY_0", "remote.origin.url");
    command.env(
        "GIT_CONFIG_VALUE_0",
        "https://github.com/sim-nest/sim-music",
    );
}

fn has_repo_arg(args: &[String]) -> bool {
    args.iter()
        .skip(2)
        .any(|arg| arg == "--repo" || arg.starts_with("--repo="))
}

fn tooling_hint_from_env() -> Option<PathBuf> {
    env::var_os("SIMDOC_TOOLING_MANIFEST").map(PathBuf::from)
}

#[derive(Debug, PartialEq, Eq)]
enum SimdocLaunch {
    Binary(PathBuf),
    Manifest(PathBuf),
}

fn select_simdoc(repo_root: &Path, override_hint: Option<&Path>) -> Result<SimdocLaunch, String> {
    if let Some(hint) = override_hint {
        return Ok(SimdocLaunch::Manifest(isolated_manifest_from_hint(hint)?));
    }
    if let Some(binary) = locate_built_simdoc(repo_root) {
        return Ok(SimdocLaunch::Binary(binary));
    }
    Ok(SimdocLaunch::Manifest(locate_sibling_simdoc(repo_root)?))
}

fn select_workspace_manifest(
    repo_root: &Path,
    override_hint: Option<&Path>,
) -> Result<PathBuf, String> {
    if let Some(path) = override_hint {
        return Ok(path.to_path_buf());
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

fn locate_built_simdoc(repo_root: &Path) -> Option<PathBuf> {
    let binary = repo_root
        .parent()?
        .join("sim-tooling/crates/simdoc/target/debug/simdoc");
    binary.is_file().then_some(binary)
}

fn locate_sibling_simdoc(repo_root: &Path) -> Result<PathBuf, String> {
    let isolated = repo_root
        .parent()
        .unwrap_or(repo_root)
        .join("sim-tooling")
        .join("crates")
        .join("simdoc")
        .join("Cargo.toml");
    if isolated.is_file() {
        return Ok(isolated);
    }
    Err(
        "set SIMDOC_TOOLING_MANIFEST to the sim-tooling checkout or crates/simdoc/Cargo.toml"
            .to_owned(),
    )
}

fn isolated_manifest_from_hint(hint: &Path) -> Result<PathBuf, String> {
    let isolated_rel = Path::new("crates").join("simdoc").join("Cargo.toml");
    let candidates = [
        hint.to_path_buf(),
        hint.join("Cargo.toml"),
        hint.join(&isolated_rel),
        hint.parent().unwrap_or(hint).join(&isolated_rel),
    ];
    for candidate in candidates {
        if candidate.is_file() && candidate.ends_with(&isolated_rel) {
            return Ok(candidate);
        }
    }
    Err(format!(
        "SIMDOC_TOOLING_MANIFEST={} does not locate crates/simdoc/Cargo.toml",
        hint.display()
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn isolated_rel() -> PathBuf {
        Path::new("crates").join("simdoc").join("Cargo.toml")
    }

    fn scratch_root() -> PathBuf {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "sim-music-simdoc-{}-{}-{}",
            std::process::id(),
            nanos,
            seq
        ));
        fs::create_dir_all(&root).expect("scratch");
        root
    }

    fn write_tooling(tooling: &Path) -> (PathBuf, PathBuf) {
        let isolated = tooling.join(isolated_rel());
        fs::create_dir_all(isolated.parent().expect("parent")).expect("dirs");
        fs::write(&isolated, "[package]\nname = \"simdoc\"\n").expect("manifest");
        let workspace = tooling.join("Cargo.toml");
        fs::write(&workspace, "[workspace]\n").expect("workspace");
        (workspace, isolated)
    }

    #[test]
    fn tooling_hints_resolve_to_the_isolated_encoder() {
        let tooling = scratch_root();
        let (_, selected) = write_tooling(&tooling);
        assert_eq!(
            isolated_manifest_from_hint(&tooling.join("Cargo.toml")).expect("workspace hint"),
            selected
        );
        assert_eq!(
            isolated_manifest_from_hint(&selected).expect("isolated hint"),
            selected
        );
        assert_eq!(
            isolated_manifest_from_hint(&tooling).expect("checkout hint"),
            selected
        );
        fs::remove_dir_all(tooling).expect("cleanup");
    }

    #[test]
    fn missing_tooling_hint_is_an_error() {
        let root = scratch_root();
        let missing = root.join("no-such-tooling").join("Cargo.toml");
        let error = isolated_manifest_from_hint(&missing).expect_err("missing hint");
        assert!(error.contains("does not locate crates/simdoc/Cargo.toml"));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn sibling_checkout_resolves_when_the_encoder_exists() {
        let parent = scratch_root();
        let (_, manifest) = write_tooling(&parent.join("sim-tooling"));
        let repo = parent.join("sim-music");
        fs::create_dir_all(&repo).expect("repo");
        assert_eq!(locate_sibling_simdoc(&repo).expect("sibling"), manifest);
        fs::remove_dir_all(parent).expect("cleanup");
    }

    #[test]
    fn missing_sibling_checkout_is_an_error() {
        let parent = scratch_root();
        let repo = parent.join("sim-music");
        fs::create_dir_all(&repo).expect("repo");
        let error = locate_sibling_simdoc(&repo).expect_err("missing sibling");
        assert!(error.contains("set SIMDOC_TOOLING_MANIFEST"));
        let workspace_error =
            select_workspace_manifest(&repo, None).expect_err("missing workspace");
        assert!(workspace_error.contains("set SIMDOC_TOOLING_MANIFEST"));
        fs::remove_dir_all(parent).expect("cleanup");
    }

    #[test]
    fn workspace_verbs_keep_the_tooling_workspace_manifest() {
        let parent = scratch_root();
        let (workspace, isolated) = write_tooling(&parent.join("sim-tooling"));
        let repo = parent.join("sim-music");
        fs::create_dir_all(&repo).expect("repo");
        assert_eq!(
            select_workspace_manifest(&repo, None).expect("workspace"),
            workspace
        );
        assert_ne!(workspace, isolated);
        fs::remove_dir_all(parent).expect("cleanup");
    }

    #[test]
    fn built_encoder_is_preferred_when_no_override_is_set() {
        let parent = scratch_root();
        let binary = parent.join("sim-tooling/crates/simdoc/target/debug/simdoc");
        fs::create_dir_all(binary.parent().expect("parent")).expect("dirs");
        fs::write(&binary, b"").expect("binary");
        let repo = parent.join("sim-music");
        fs::create_dir_all(&repo).expect("repo");
        assert_eq!(
            select_simdoc(&repo, None).expect("built"),
            SimdocLaunch::Binary(binary)
        );
        fs::remove_dir_all(parent).expect("cleanup");
    }

    #[test]
    fn explicit_override_beats_the_sibling_binary() {
        let parent = scratch_root();
        let binary = parent.join("sim-tooling/crates/simdoc/target/debug/simdoc");
        fs::create_dir_all(binary.parent().expect("parent")).expect("dirs");
        fs::write(&binary, b"").expect("binary");
        let (override_workspace, override_isolated) = write_tooling(&parent.join("other-tooling"));
        let repo = parent.join("sim-music");
        fs::create_dir_all(&repo).expect("repo");
        assert_eq!(
            select_simdoc(&repo, Some(&override_workspace)).expect("override"),
            SimdocLaunch::Manifest(override_isolated)
        );
        assert_eq!(
            select_workspace_manifest(&repo, Some(&override_workspace))
                .expect("workspace override"),
            override_workspace
        );
        fs::remove_dir_all(parent).expect("cleanup");
    }

    #[test]
    fn missing_built_encoder_falls_through() {
        let parent = scratch_root();
        let (_, manifest) = write_tooling(&parent.join("sim-tooling"));
        let repo = parent.join("sim-music");
        fs::create_dir_all(&repo).expect("repo");
        assert!(locate_built_simdoc(&repo).is_none());
        assert_eq!(
            select_simdoc(&repo, None).expect("manifest"),
            SimdocLaunch::Manifest(manifest)
        );
        fs::remove_dir_all(parent).expect("cleanup");
    }

    #[test]
    fn local_origin_points_at_the_public_repository() {
        let mut command = Command::new("true");
        configure_local_origin(&mut command);
        let env: Vec<(String, Option<String>)> = command
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.map(|item| item.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert!(env.contains(&("GIT_CONFIG_COUNT".to_owned(), Some("1".to_owned()))));
        assert!(env.contains(&(
            "GIT_CONFIG_KEY_0".to_owned(),
            Some("remote.origin.url".to_owned())
        )));
        assert!(env.contains(&(
            "GIT_CONFIG_VALUE_0".to_owned(),
            Some("https://github.com/sim-nest/sim-music".to_owned())
        )));
    }
}
