use std::fs;
use std::path::{Path, PathBuf};

const GENERAL_WARN: usize = 500;
const GENERAL_ERROR: usize = 700;
const ENTRY_WARN: usize = 150;
const ENTRY_ERROR: usize = 250;

pub(crate) fn run() -> Result<(), String> {
    let root = std::env::current_dir().map_err(|err| format!("current dir failed: {err}"))?;
    let mut files = Vec::new();
    collect_rs_files(&root.join("crates"), &mut files)?;
    collect_rs_files(&root.join("xtask"), &mut files)?;
    files.sort();

    let total = files.len();
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).map_err(|err| format!("{}: {err}", file.display()))?;
        let lines = text.lines().count();
        let threshold = threshold_for(&file);
        let rel = file
            .strip_prefix(&root)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        if lines > threshold.error {
            failures.push(format!(
                "{rel}: {lines} lines exceeds hard limit {}",
                threshold.error
            ));
        } else if lines > threshold.warn {
            warnings.push(format!(
                "{rel}: {lines} lines exceeds soft target {}",
                threshold.warn
            ));
        }
    }

    for warning in &warnings {
        eprintln!("warning: {warning}");
    }
    if !failures.is_empty() {
        for failure in &failures {
            eprintln!("error: {failure}");
        }
        return Err(format!(
            "{} Rust file-size limit(s) exceeded",
            failures.len()
        ));
    }
    println!(
        "check-file-sizes: {} Rust file(s), {} warning(s), 0 error(s)",
        total,
        warnings.len(),
    );
    Ok(())
}

fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)
        .map_err(|err| format!("{}: {err}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("{}: {err}", dir.display()))?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|name| name.to_str());
            if matches!(name, Some("target") | Some(".git")) {
                continue;
            }
            collect_rs_files(&path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn threshold_for(path: &Path) -> Threshold {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("lib.rs" | "main.rs" | "mod.rs") => Threshold {
            warn: ENTRY_WARN,
            error: ENTRY_ERROR,
        },
        _ => Threshold {
            warn: GENERAL_WARN,
            error: GENERAL_ERROR,
        },
    }
}

struct Threshold {
    warn: usize,
    error: usize,
}
