#![forbid(unsafe_code)]

mod music_fixtures;
mod simdoc;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("music-fixtures") => music_fixtures::run(&args[2..]),
        _ => simdoc::run(args),
    };
    if let Err(err) = result {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
