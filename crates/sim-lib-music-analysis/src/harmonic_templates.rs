//! Built-in chromatic key and triad template catalogs.

use crate::HarmonicTemplate;

const NAMES: [&str; 12] = [
    "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];

/// Returns the built-in major/minor key templates in stable chromatic order.
pub fn key_templates() -> Vec<HarmonicTemplate> {
    const MAJOR: [f64; 12] = [
        6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
    ];
    const MINOR: [f64; 12] = [
        6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
    ];
    let mut templates = Vec::with_capacity(24);
    for (root, name) in NAMES.iter().enumerate() {
        templates.push(rotated_template(format!("{name} major"), &MAJOR, root));
        templates.push(rotated_template(format!("{name} minor"), &MINOR, root));
    }
    templates
}

/// Returns built-in major/minor triad templates in stable chromatic order.
pub fn chord_templates() -> Vec<HarmonicTemplate> {
    let mut templates = Vec::with_capacity(24);
    for (root, name) in NAMES.iter().enumerate() {
        templates.push(triad_template(root, name, &[0, 4, 7], "maj"));
        templates.push(triad_template(root, name, &[0, 3, 7], "min"));
    }
    templates
}

fn rotated_template(label: String, profile: &[f64; 12], root: usize) -> HarmonicTemplate {
    HarmonicTemplate {
        label,
        weights: (0..12)
            .map(|pitch_class| profile[(pitch_class + 12 - root) % 12])
            .collect(),
    }
}

fn triad_template(
    root: usize,
    root_name: &str,
    intervals: &[usize],
    quality: &str,
) -> HarmonicTemplate {
    let mut weights = vec![0.05; 12];
    for interval in intervals {
        weights[(root + interval) % 12] = 1.0;
    }
    HarmonicTemplate {
        label: format!("{root_name}:{quality}"),
        weights,
    }
}
