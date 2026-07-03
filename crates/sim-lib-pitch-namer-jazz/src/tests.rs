use sim_lib_pitch_core::PitchClass;

use crate::{JazzQuality, match_jazz_symbol, parse_jazz_symbol};

#[test]
fn parser_round_trips_generated_corpus() {
    let roots = [
        PitchClass::C,
        PitchClass::CS,
        PitchClass::D,
        PitchClass::DS,
        PitchClass::E,
        PitchClass::F,
        PitchClass::FS,
        PitchClass::G,
        PitchClass::GS,
        PitchClass::A,
        PitchClass::AS,
        PitchClass::B,
    ];
    let slashes = [
        None,
        Some(PitchClass::C),
        Some(PitchClass::F),
        Some(PitchClass::B),
    ];
    let mut count = 0usize;
    for root in roots {
        for quality in JazzQuality::all() {
            for slash_bass in slashes {
                let text = match slash_bass {
                    Some(bass) => format!(
                        "{}:{}/{}",
                        root.canonical_name(),
                        quality.suffix(),
                        bass.canonical_name()
                    ),
                    None => format!("{}:{}", root.canonical_name(), quality.suffix()),
                };
                let parsed = parse_jazz_symbol(&text).expect("valid corpus entry");
                assert_eq!(parsed.to_string(), text);
                count += 1;
            }
        }
    }
    assert!(count >= 500, "expected at least 500 entries, got {count}");
}

#[test]
fn matcher_finds_dominant_seventh() {
    let symbol = parse_jazz_symbol("C:7").expect("symbol");
    let matched = match_jazz_symbol(symbol.mask(), Some(symbol.root)).expect("match");
    assert_eq!(matched.quality, JazzQuality::Dominant7);
}
