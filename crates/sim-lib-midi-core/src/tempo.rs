//! Tempo conversions between microseconds-per-quarter and beats per minute.

/// The default MIDI tempo of 500_000 microseconds per quarter (120 BPM).
pub const DEFAULT_US_PER_QUARTER: u32 = 500_000;

/// Converts beats per minute to microseconds per quarter note (rounded).
pub fn bpm_to_us_per_quarter(bpm: f64) -> u32 {
    (60_000_000.0 / bpm).round() as u32
}

/// Converts microseconds per quarter note to beats per minute.
pub fn us_per_quarter_to_bpm(us_per_quarter: u32) -> f64 {
    60_000_000.0 / us_per_quarter as f64
}
