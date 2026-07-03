//! FWHT-based melody analysis.
//!
//! This adapter converts musical material into binary / signed signals and runs
//! them through the Walsh-Hadamard transform in `sim-lib-discrete-spectral`. No
//! FWHT code lives here: the discrete stack owns the primitive, music consumes
//! it. Every analysis records the window size, padding, basis, and normalization
//! it used so results are reproducible.

use sim_lib_discrete_spectral::{
    Normalization, WalshBasis, fwht_f64, pad_to_power_of_two, spectral_entropy, walsh_signature,
};

/// The number of pitch classes; pitch-class windows pad to 16 (the next power of
/// two) for the transform.
pub const PITCH_CLASSES: usize = 12;

/// A reproducible melody analysis result.
#[derive(Debug, Clone, PartialEq)]
pub struct MelodyAnalysis {
    /// The signal length before padding.
    pub window: usize,
    /// The padded (power-of-two) length actually transformed.
    pub padded: usize,
    /// The Walsh basis used.
    pub basis: WalshBasis,
    /// The normalization used for the forward transform.
    pub normalization: Normalization,
    /// The low-order Walsh signature.
    pub signature: Vec<f64>,
    /// The Walsh spectral entropy of the signal.
    pub entropy: f64,
}

/// Convert a set of present pitch classes (each `0..12`) into a 16-bit presence
/// vector (bits 12..16 are padding zeros).
pub fn pitch_class_window_to_bits(present: &[u8]) -> Vec<bool> {
    let mut bits = vec![false; 16];
    for &pc in present {
        if (pc as usize) < PITCH_CLASSES {
            bits[pc as usize] = true;
        }
    }
    bits
}

/// Convert a pitch sequence into a signed contour vector: `+1` up, `-1` down,
/// `0` repeated, for each successive step.
pub fn contour_to_signed(pitches: &[i32]) -> Vec<i64> {
    pitches
        .windows(2)
        .map(|w| match w[1].cmp(&w[0]) {
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
        })
        .collect()
}

/// Convert an onset grid (true = note onset) into a binary vector (identity, but
/// named for the pipeline).
pub fn onset_grid_to_bits(grid: &[bool]) -> Vec<bool> {
    grid.to_vec()
}

fn to_f64_signal(bits: &[bool]) -> Vec<f64> {
    bits.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect()
}

/// Analyze a binary signal: pad to a power of two, FWHT it, and record the
/// low-order signature (of size `order`) and spectral entropy.
pub fn analyze_bits(bits: &[bool], order: usize) -> MelodyAnalysis {
    let window = bits.len();
    let signal = to_f64_signal(bits);
    let (padded_signal, padded) = pad_to_power_of_two(&signal, 0.0).expect("pad");
    let coeffs = fwht_f64(&padded_signal).expect("fwht of power-of-two length");
    MelodyAnalysis {
        window,
        padded,
        basis: WalshBasis::Natural,
        normalization: Normalization::None,
        signature: walsh_signature(&coeffs.values, order),
        entropy: spectral_entropy(&coeffs.values),
    }
}

/// The Walsh spectral entropy of a pitch-class window (lower = simpler).
pub fn pitch_class_entropy(present: &[u8]) -> f64 {
    analyze_bits(&pitch_class_window_to_bits(present), 4).entropy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_class_window_fixture() {
        // C major triad pitch classes 0, 4, 7.
        let bits = pitch_class_window_to_bits(&[0, 4, 7]);
        assert_eq!(bits.len(), 16);
        assert!(bits[0] && bits[4] && bits[7]);
        assert!(!bits[1] && !bits[12]); // absent and padding
    }

    #[test]
    fn contour_fixture() {
        // Ascending then descending then repeat.
        assert_eq!(contour_to_signed(&[60, 62, 60, 60]), vec![1, -1, 0]);
    }

    #[test]
    fn walsh_signature_is_deterministic() {
        let bits = pitch_class_window_to_bits(&[0, 4, 7]);
        let a = analyze_bits(&bits, 4);
        let b = analyze_bits(&bits, 4);
        assert_eq!(a, b);
        assert_eq!(a.padded, 16);
        assert_eq!(a.signature.len(), 4);
    }

    #[test]
    fn entropy_orders_simplicity() {
        // A single pitch class -> impulse-like -> high entropy; the full
        // chromatic set -> all-ones -> concentrated spectrum -> low entropy.
        let one = pitch_class_entropy(&[0]);
        let all: Vec<u8> = (0..12).collect();
        let full = pitch_class_entropy(&all);
        assert!(full < one, "{full} vs {one}");
    }
}
