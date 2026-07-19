//! Repository maintenance tasks for sim-music validation and generated docs.

#![forbid(unsafe_code)]

mod check_file_sizes;
mod music_fixtures;
mod simdoc;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("check-file-sizes") => check_file_sizes::run(),
        Some("crate-catalog") => simdoc::run_repo_tool(args, "crate-catalog"),
        Some("music-fixtures") => music_fixtures::run(&args[2..]),
        _ => simdoc::run(args),
    };
    if let Err(err) = result {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
