//! Expression projection for composed audio-analysis reports.

use sim_kernel::{Expr, NumberLiteral, Symbol};
use sim_lib_music_analysis::{HarmonicAlternative, HarmonicSequence};

use crate::{
    AudioAnalysis, BeatTracking, CepstralNormalization, DctNormalization, FrequencyScale, Mfcc,
    OnsetAlternative, OnsetPeaks, OnsetRejection, SpectralEnergy, ZeroCrossingRate,
};

pub(crate) fn audio_analysis_expr(analysis: &AudioAnalysis) -> Expr {
    map(vec![
        ("sample-rate", integer(analysis.sample_rate)),
        ("features", feature_expr(analysis)),
        (
            "onsets",
            analysis.onsets.as_ref().map_or(Expr::Nil, onset_expr),
        ),
        (
            "beats",
            analysis.beats.as_ref().map_or(Expr::Nil, beat_expr),
        ),
        (
            "zero-crossing-rate",
            analysis
                .zero_crossing_rate
                .as_ref()
                .map_or(Expr::Nil, zcr_expr),
        ),
        ("mfcc", analysis.mfcc.as_ref().map_or(Expr::Nil, mfcc_expr)),
        (
            "chroma",
            analysis.chroma.as_ref().map_or(Expr::Nil, |chroma| {
                map(vec![
                    ("tuning", Expr::String(chroma.reference.tuning.clone())),
                    ("divisions", integer(chroma.reference.divisions)),
                    (
                        "frames",
                        Expr::Vector(
                            chroma
                                .frames
                                .iter()
                                .map(|frame| {
                                    map(vec![
                                        ("sample", integer(frame.onset_sample)),
                                        (
                                            "values",
                                            Expr::Vector(
                                                frame.bins.iter().copied().map(float).collect(),
                                            ),
                                        ),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ])
            }),
        ),
        (
            "key",
            analysis.key.as_ref().map_or(Expr::Nil, harmonic_expr),
        ),
        (
            "chords",
            analysis.chords.as_ref().map_or(Expr::Nil, harmonic_expr),
        ),
        (
            "evidence",
            map(vec![
                ("work-used", integer(analysis.evidence.work_used)),
                ("work-limit", integer(analysis.evidence.work_limit)),
                ("result-limit", integer(analysis.evidence.result_limit)),
                ("seed", integer(analysis.evidence.seed)),
            ]),
        ),
    ])
}

fn feature_expr(analysis: &AudioAnalysis) -> Expr {
    let selection = &analysis.selection;
    let mut features = Vec::new();
    for (selected, name) in [
        (selection.onsets, "onsets"),
        (selection.beats, "beats"),
        (selection.zero_crossing_rate, "zero-crossing-rate"),
        (selection.mfcc, "mfcc"),
        (selection.chroma, "chroma"),
        (selection.key, "key"),
        (selection.chords, "chords"),
    ] {
        if selected {
            features.push(symbol(name));
        }
    }
    Expr::Vector(features)
}

fn onset_expr(onsets: &OnsetPeaks) -> Expr {
    map(vec![
        ("latency-samples", integer(onsets.latency_samples)),
        (
            "minimum-distance-samples",
            integer(onsets.plan.minimum_distance_samples),
        ),
        ("work-used", integer(onsets.work_used)),
        (
            "peaks",
            Expr::Vector(
                onsets
                    .peaks
                    .iter()
                    .map(|peak| {
                        map(vec![
                            ("sample", integer(peak.sample)),
                            ("available-at-sample", integer(peak.available_at_sample)),
                            ("strength", float(peak.strength)),
                            ("confidence", float(peak.confidence)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "alternatives",
            Expr::Vector(
                onsets
                    .alternatives
                    .iter()
                    .map(onset_alternative_expr)
                    .collect(),
            ),
        ),
    ])
}

fn onset_alternative_expr(alternative: &OnsetAlternative) -> Expr {
    map(vec![
        (
            "reason",
            symbol(match alternative.reason {
                OnsetRejection::BelowThreshold => "below-threshold",
                OnsetRejection::MinimumDistance => "minimum-distance",
                OnsetRejection::ResultLimit => "result-limit",
            }),
        ),
        ("sample", integer(alternative.candidate.sample)),
        ("confidence", float(alternative.candidate.confidence)),
    ])
}

fn beat_expr(beats: &BeatTracking) -> Expr {
    map(vec![
        ("tempo-policy", symbol("varying")),
        (
            "tempo-candidates",
            Expr::Vector(
                beats
                    .tempo_candidates
                    .iter()
                    .map(|candidate| {
                        map(vec![
                            ("bpm", float(candidate.bpm)),
                            ("confidence", float(candidate.confidence)),
                            ("support", integer(candidate.support)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "sequence",
            Expr::Vector(
                beats
                    .beats
                    .iter()
                    .map(|beat| {
                        map(vec![
                            ("sample", integer(beat.sample)),
                            ("confidence", float(beat.confidence)),
                            ("bpm", beat.bpm.map_or(Expr::Nil, float)),
                            (
                                "alternatives",
                                Expr::Vector(
                                    beat.alternatives
                                        .iter()
                                        .map(|candidate| {
                                            map(vec![
                                                ("bpm", float(candidate.bpm)),
                                                ("confidence", float(candidate.confidence)),
                                                (
                                                    "interval-factor",
                                                    float(candidate.interval_factor),
                                                ),
                                            ])
                                        })
                                        .collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "meter-hypotheses",
            Expr::Vector(
                beats
                    .meter_hypotheses
                    .iter()
                    .map(|meter| {
                        map(vec![
                            ("beats-per-bar", integer(meter.beats_per_bar)),
                            ("phase", integer(meter.phase)),
                            ("confidence", float(meter.confidence)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "dynamic-programming",
            beats.dynamic_programming.as_ref().map_or(Expr::Nil, |dp| {
                map(vec![
                    ("total-cost", float(dp.total_cost)),
                    ("work-used", integer(dp.receipt.work_used)),
                    ("cells", integer(dp.receipt.cells)),
                    ("edges", integer(dp.receipt.edges)),
                    (
                        "selected-indices",
                        Expr::Vector(dp.selected_indices.iter().copied().map(integer).collect()),
                    ),
                ])
            }),
        ),
    ])
}

fn zcr_expr(zcr: &ZeroCrossingRate) -> Expr {
    map(vec![
        ("sample-rate", integer(zcr.sample_rate)),
        ("work-used", integer(zcr.work_used)),
        (
            "frames",
            Expr::Vector(
                zcr.frames
                    .iter()
                    .map(|frame| {
                        map(vec![
                            ("sample", integer(frame.onset_sample)),
                            ("rate", float(frame.rate)),
                            ("reviewed-pairs", integer(frame.reviewed_pairs)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn mfcc_expr(mfcc: &Mfcc) -> Expr {
    map(vec![
        ("sample-rate", integer(mfcc.sample_rate)),
        (
            "scale",
            symbol(match mfcc.plan.filterbank.scale {
                FrequencyScale::Mel => "mel",
                FrequencyScale::Bark => "bark",
                FrequencyScale::Erb => "erb",
            }),
        ),
        (
            "energy",
            symbol(match mfcc.plan.energy {
                SpectralEnergy::Magnitude => "magnitude",
                SpectralEnergy::Power => "power",
            }),
        ),
        ("log-floor", float(mfcc.plan.log_floor)),
        (
            "dct-normalization",
            symbol(match mfcc.plan.dct_normalization {
                DctNormalization::None => "none",
                DctNormalization::Orthonormal => "orthonormal",
            }),
        ),
        ("lifter", mfcc.plan.lifter.map_or(Expr::Nil, float)),
        (
            "normalization",
            symbol(match mfcc.plan.normalization {
                CepstralNormalization::None => "none",
                CepstralNormalization::Mean => "mean",
                CepstralNormalization::MeanVariance { .. } => "mean-variance",
            }),
        ),
        ("work-used", integer(mfcc.work_used)),
        (
            "frames",
            Expr::Vector(
                mfcc.frames
                    .iter()
                    .map(|frame| {
                        map(vec![
                            ("sample", integer(frame.onset_sample)),
                            (
                                "coefficients",
                                Expr::Vector(
                                    frame.coefficients.iter().copied().map(float).collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn harmonic_expr(sequence: &HarmonicSequence) -> Expr {
    map(vec![
        (
            "strategy",
            symbol(match sequence.plan.strategy {
                sim_lib_music_analysis::HarmonicDecodeStrategy::Posterior => "posterior",
                sim_lib_music_analysis::HarmonicDecodeStrategy::Viterbi => "hmm",
            }),
        ),
        (
            "frames",
            Expr::Vector(
                sequence
                    .frames
                    .iter()
                    .map(|frame| {
                        map(vec![
                            ("sample", integer(frame.at_sample)),
                            ("label", Expr::String(frame.label.clone())),
                            ("confidence", float(frame.confidence)),
                            (
                                "alternatives",
                                Expr::Vector(
                                    frame
                                        .alternatives
                                        .iter()
                                        .map(harmonic_alternative_expr)
                                        .collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "evidence",
            map(vec![
                ("log-likelihood", float(sequence.evidence.log_likelihood)),
                (
                    "numerical-repairs",
                    integer(sequence.evidence.numerical_repairs),
                ),
                (
                    "normalized-steps",
                    integer(sequence.evidence.normalized_steps),
                ),
                ("work-used", integer(sequence.evidence.work_used)),
                (
                    "path-log-probability",
                    sequence
                        .evidence
                        .path_log_probability
                        .map_or(Expr::Nil, float),
                ),
            ]),
        ),
    ])
}

fn harmonic_alternative_expr(alternative: &HarmonicAlternative) -> Expr {
    map(vec![
        ("label", Expr::String(alternative.label.clone())),
        ("similarity", float(alternative.similarity)),
        ("posterior", float(alternative.posterior)),
    ])
}

fn map(entries: Vec<(&str, Expr)>) -> Expr {
    Expr::Map(
        entries
            .into_iter()
            .map(|(key, value)| (symbol(key), value))
            .collect(),
    )
}

fn integer(value: impl ToString) -> Expr {
    number("i64", value.to_string())
}

fn float(value: f64) -> Expr {
    number("f64", value.to_string())
}

fn number(domain: &str, canonical: String) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", domain),
        canonical,
    })
}

fn symbol(value: &str) -> Expr {
    Expr::Symbol(Symbol::new(value))
}
