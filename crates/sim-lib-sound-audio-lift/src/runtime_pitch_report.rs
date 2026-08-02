//! Expression projection for pitch-track reports.

use sim_kernel::{Diagnostic, Expr, NumberLiteral, Severity, Symbol};

use crate::{
    AudioLiftReport, PitchFrameTail, PitchHypothesis, PitchInterpolation, PitchRejectionReason,
    PitchTrack, PitchTrackFrame, PitchTrackMethod, RejectedPitchHypothesis,
};

pub(crate) fn pitch_track_report_expr(report: &AudioLiftReport<PitchTrack>) -> Expr {
    map(vec![
        ("plan", plan_expr(&report.value)),
        ("work-used", integer(report.value.work_used)),
        (
            "frames",
            Expr::Vector(report.value.frames.iter().map(frame_expr).collect()),
        ),
        (
            "contour",
            Expr::Vector(
                report
                    .value
                    .contour
                    .iter()
                    .map(|candidate| candidate.as_ref().map_or(Expr::Nil, hypothesis_expr))
                    .collect(),
            ),
        ),
        (
            "diagnostics",
            Expr::Vector(report.diagnostics.iter().map(diagnostic_expr).collect()),
        ),
    ])
}

fn plan_expr(track: &PitchTrack) -> Expr {
    let plan = &track.plan;
    map(vec![
        (
            "method",
            symbol(match plan.method {
                PitchTrackMethod::Yin => "yin",
                PitchTrackMethod::Pyin => "pyin",
            }),
        ),
        (
            "range",
            Expr::Vector(vec![float(plan.range.min_hz), float(plan.range.max_hz)]),
        ),
        (
            "frames",
            map(vec![
                ("size", integer(plan.frames.size)),
                ("hop", integer(plan.frames.hop)),
                (
                    "tail",
                    symbol(match plan.frames.tail {
                        PitchFrameTail::Drop => "drop",
                        PitchFrameTail::ZeroPad => "zero-pad",
                    }),
                ),
                (
                    "interpolation",
                    symbol(match plan.interpolation {
                        PitchInterpolation::None => "none",
                        PitchInterpolation::Parabolic => "parabolic",
                    }),
                ),
            ]),
        ),
        (
            "yin",
            map(vec![
                ("threshold", float(plan.yin.threshold)),
                (
                    "thresholds",
                    Expr::Vector(
                        plan.yin
                            .pyin_thresholds
                            .iter()
                            .copied()
                            .map(float)
                            .collect(),
                    ),
                ),
                ("voiced-probability", float(plan.yin.min_voiced_probability)),
                ("silence-rms", float(plan.yin.silence_rms)),
            ]),
        ),
        (
            "control",
            map(vec![
                ("work", integer(plan.control.max_work)),
                ("results", integer(plan.control.max_results)),
                ("seed", integer(plan.control.seed)),
            ]),
        ),
    ])
}

fn frame_expr(frame: &PitchTrackFrame) -> Expr {
    let provenance = &frame.provenance;
    map(vec![
        (
            "provenance",
            map(vec![
                ("frame-index", integer(provenance.frame_index)),
                ("onset-sample", integer(provenance.onset_sample)),
                ("source-samples", integer(provenance.source_samples)),
                ("frame-size", integer(provenance.frame_size)),
                ("hop-size", integer(provenance.hop_size)),
                ("sample-rate", integer(provenance.sample_rate)),
                ("zero-padded", Expr::Bool(provenance.zero_padded)),
            ]),
        ),
        (
            "candidates",
            Expr::Vector(frame.candidates.iter().map(hypothesis_expr).collect()),
        ),
        (
            "rejected",
            Expr::Vector(frame.rejected.iter().map(rejected_expr).collect()),
        ),
    ])
}

fn hypothesis_expr(hypothesis: &PitchHypothesis) -> Expr {
    map(vec![
        (
            "pitch",
            map(vec![
                (
                    "name",
                    Expr::String(format!(
                        "{}{}",
                        hypothesis.pitch.class.canonical_name(),
                        hypothesis.pitch.octave
                    )),
                ),
                ("semitone", integer(hypothesis.pitch.semitone())),
                (
                    "midi",
                    hypothesis.pitch.to_midi().map_or(Expr::Nil, integer),
                ),
            ]),
        ),
        ("frequency", float(hypothesis.frequency.0)),
        ("lower-frequency", float(hypothesis.lower_frequency.0)),
        ("upper-frequency", float(hypothesis.upper_frequency.0)),
        ("lag", integer(hypothesis.lag)),
        ("interpolated-lag", float(hypothesis.interpolated_lag)),
        ("periodicity", float(hypothesis.periodicity)),
        ("voiced-probability", float(hypothesis.voiced_probability)),
        ("confidence", float(hypothesis.confidence)),
        ("cents-error", float(hypothesis.cents_error)),
    ])
}

fn rejected_expr(rejected: &RejectedPitchHypothesis) -> Expr {
    map(vec![
        (
            "reason",
            symbol(match rejected.reason {
                PitchRejectionReason::Silence => "silence",
                PitchRejectionReason::Threshold => "threshold",
                PitchRejectionReason::VoicedProbability => "voiced-probability",
                PitchRejectionReason::ResultLimit => "result-limit",
            }),
        ),
        (
            "hypothesis",
            rejected
                .hypothesis
                .as_ref()
                .map_or(Expr::Nil, hypothesis_expr),
        ),
    ])
}

fn diagnostic_expr(diagnostic: &Diagnostic) -> Expr {
    map(vec![
        (
            "severity",
            symbol(match diagnostic.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "info",
                Severity::Note => "note",
            }),
        ),
        ("message", Expr::String(diagnostic.message.clone())),
        (
            "code",
            diagnostic
                .code
                .as_ref()
                .map_or(Expr::Nil, |code| Expr::Symbol(code.clone())),
        ),
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
