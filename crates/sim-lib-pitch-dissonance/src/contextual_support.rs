use std::collections::BTreeMap;

use sim_lib_pitch_ratio::{PitchRatio, RatioPolicy, analyze_ratio_chord};

use crate::contextual::{
    ContextualPitch, ContextualSonanceOptions, ContextualSonanceWeights, DuplicatePolicy,
    SonanceNormalization, VoiceIdentityPolicy,
};
use crate::model::merge_contributions;
use crate::{Sonance, SonanceEvidence};

pub(crate) fn contextual_sonance(
    model: &'static str,
    roughness_mass: f64,
    normalized_density: f64,
    harmonic_context: f64,
    options: ContextualSonanceOptions,
    mut provenance: Vec<String>,
) -> Sonance {
    provenance.push(format!(
        "window=before:{};after:{}",
        options.window.before, options.window.after
    ));
    provenance.push(format!("normalization={}", options.normalization.name()));
    provenance.push(format!("duplicates={}", options.duplicates.name()));
    Sonance {
        roughness_mass,
        normalized_density,
        harmonic_context,
        evidence: SonanceEvidence {
            model,
            normalization: options.normalization.name(),
            aggregation: options.merge.name(),
            dialect: "contextual",
            provenance,
        },
    }
}

pub(crate) fn weighted_score(sonance: &Sonance, weights: ContextualSonanceWeights) -> f64 {
    sonance.roughness_mass * finite_or_one(weights.roughness)
        + sonance.normalized_density * finite_or_one(weights.density)
        + sonance.harmonic_context * finite_or_one(weights.harmonic_context)
}

fn finite_or_one(value: f64) -> f64 {
    if value.is_finite() { value } else { 1.0 }
}

pub(crate) fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

pub(crate) fn apply_duplicate_policy(
    notes: &[ContextualPitch],
    policy: DuplicatePolicy,
) -> Vec<ContextualPitch> {
    match policy {
        DuplicatePolicy::Retain => notes.to_vec(),
        DuplicatePolicy::Collapse => {
            let mut seen = BTreeMap::new();
            for note in notes {
                seen.entry((note.pitch.semitone(), note.voice.clone()))
                    .or_insert_with(|| note.clone());
            }
            seen.into_values().collect()
        }
    }
}

pub(crate) fn pair_count(len: usize) -> usize {
    len.saturating_mul(len.saturating_sub(1)) / 2
}

pub(crate) fn normalize_contextual(
    mass: f64,
    opportunities: usize,
    options: ContextualSonanceOptions,
) -> f64 {
    match options.normalization {
        SonanceNormalization::Raw => mass,
        SonanceNormalization::PerPair => {
            if opportunities == 0 {
                0.0
            } else {
                mass / opportunities as f64
            }
        }
    }
}

pub(crate) fn interval_roughness(
    notes: &[ContextualPitch],
    options: ContextualSonanceOptions,
) -> f64 {
    let contributions = pairwise(notes)
        .into_iter()
        .map(|(left, right)| {
            let interval_class = ((right.pitch.semitone() - left.pitch.semitone()).rem_euclid(12)
                as u8)
                .min((left.pitch.semitone() - right.pitch.semitone()).rem_euclid(12) as u8);
            let weight = match interval_class {
                0 => 0.35,
                1 | 11 => 1.0,
                2 | 10 => 0.7,
                6 => 0.9,
                3 | 4 | 8 | 9 => 0.25,
                _ => 0.15,
            };
            weight * left.weight() * right.weight()
        })
        .collect::<Vec<_>>();
    merge_contributions(&contributions, options.merge)
}

fn pairwise(notes: &[ContextualPitch]) -> Vec<(&ContextualPitch, &ContextualPitch)> {
    let mut pairs = Vec::new();
    for (index, left) in notes.iter().enumerate() {
        for right in notes.iter().skip(index + 1) {
            pairs.push((left, right));
        }
    }
    pairs
}

pub(crate) fn pitch_multiset(notes: &[ContextualPitch]) -> BTreeMap<i32, usize> {
    let mut counts = BTreeMap::new();
    for note in notes {
        *counts.entry(note.pitch.semitone()).or_insert(0) += 1;
    }
    counts
}

pub(crate) fn voice_pairs<'a>(
    from: &'a [ContextualPitch],
    to: &'a [ContextualPitch],
    policy: VoiceIdentityPolicy,
) -> Vec<(&'a ContextualPitch, &'a ContextualPitch)> {
    match policy {
        VoiceIdentityPolicy::ByIndex => from.iter().zip(to.iter()).collect(),
        VoiceIdentityPolicy::ByVoiceThenIndex => {
            let mut used_to = vec![false; to.len()];
            let mut pairs = Vec::new();
            for left in from {
                if let Some(voice) = &left.voice
                    && let Some((index, right)) =
                        to.iter().enumerate().find(|(index, candidate)| {
                            !used_to[*index] && candidate.voice.as_ref() == Some(voice)
                        })
                {
                    used_to[index] = true;
                    pairs.push((left, right));
                    continue;
                }
                if let Some((index, right)) =
                    to.iter().enumerate().find(|(index, _)| !used_to[*index])
                {
                    used_to[index] = true;
                    pairs.push((left, right));
                }
            }
            pairs
        }
    }
}

pub(crate) fn pseudo_partial_cost(notes: &[ContextualPitch]) -> f64 {
    let Some(root) = notes.iter().map(|note| note.pitch.semitone()).min() else {
        return 0.0;
    };
    notes
        .iter()
        .map(|note| {
            let ratio = 2.0_f64.powf((note.pitch.semitone() - root) as f64 / 12.0);
            let nearest = (1..=8)
                .map(|partial| (ratio - partial as f64).abs() / partial as f64)
                .fold(f64::INFINITY, f64::min);
            nearest * note.weight()
        })
        .sum()
}

pub(crate) fn contextual_interval_vector(notes: &[ContextualPitch]) -> [u16; 12] {
    let mut bins = [0u16; 12];
    for (left, right) in pairwise(notes) {
        let directed = (right.pitch.semitone() - left.pitch.semitone()).rem_euclid(12) as usize;
        bins[directed] = bins[directed].saturating_add(1);
    }
    bins
}

pub(crate) fn ratio_cost(notes: &[ContextualPitch], policy: RatioPolicy) -> f64 {
    if notes.len() < 2 {
        return 0.0;
    }
    let Some(root) = notes.iter().map(|note| note.pitch.semitone()).min() else {
        return 0.0;
    };
    let Some(tones) = notes
        .iter()
        .map(|note| semitone_ratio(note.pitch.semitone() - root))
        .collect::<Option<Vec<_>>>()
    else {
        return f64::NAN;
    };
    analyze_ratio_chord(&tones, policy)
        .map(|report| report.cost)
        .unwrap_or(f64::NAN)
}

fn semitone_ratio(semitones: i32) -> Option<PitchRatio> {
    let octaves = semitones.div_euclid(12);
    let class = semitones.rem_euclid(12);
    let (mut numerator, mut denominator) = match class {
        0 => (1u64, 1u64),
        1 => (16, 15),
        2 => (9, 8),
        3 => (6, 5),
        4 => (5, 4),
        5 => (4, 3),
        6 => (45, 32),
        7 => (3, 2),
        8 => (8, 5),
        9 => (5, 3),
        10 => (9, 5),
        11 => (15, 8),
        _ => return None,
    };
    if octaves >= 0 {
        numerator = numerator.checked_mul(2u64.checked_pow(octaves as u32)?)?;
    } else {
        denominator = denominator.checked_mul(2u64.checked_pow((-octaves) as u32)?)?;
    }
    PitchRatio::new(numerator, denominator).ok()
}
