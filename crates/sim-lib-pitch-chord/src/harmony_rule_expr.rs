use sim_kernel::Expr;

use crate::harmony_expr_support::{
    invalid, mask, parse, qualified_entry, require_tag, required, scalar, scalars, scale_from_expr,
    scale_to_expr, sequence, string, tag, tagged, text, vector,
};
use crate::{
    CountRange, HarmonyConstraint, HarmonyError, HarmonyMetric, HarmonyPredicate, HarmonyRuleSet,
    Weighted,
};

pub(crate) fn rule_set_to_expr(rules: &HarmonyRuleSet) -> Expr {
    tagged(
        "rule-set",
        vec![
            qualified_entry(
                "hard",
                vector(
                    rules
                        .hard
                        .iter()
                        .map(|rule| {
                            tagged(
                                "constraint",
                                vec![
                                    qualified_entry("id", string(&rule.id)),
                                    qualified_entry(
                                        "predicate",
                                        predicate_to_expr(&rule.predicate),
                                    ),
                                ],
                            )
                        })
                        .collect(),
                ),
            ),
            qualified_entry(
                "soft",
                vector(
                    rules
                        .soft
                        .iter()
                        .map(|metric| {
                            tagged(
                                "weighted",
                                vec![
                                    qualified_entry("id", string(&metric.id)),
                                    qualified_entry("weight", scalar(metric.weight)),
                                    qualified_entry("metric", metric_to_expr(&metric.value)),
                                ],
                            )
                        })
                        .collect(),
                ),
            ),
        ],
    )
}

pub(crate) fn rule_set_from_expr(expr: &Expr) -> Result<HarmonyRuleSet, HarmonyError> {
    require_tag(expr, "rule-set")?;
    let rules = HarmonyRuleSet {
        hard: sequence(required(expr, "hard")?, "rules.hard")?
            .iter()
            .map(|expr| {
                require_tag(expr, "constraint")?;
                Ok(HarmonyConstraint::new(
                    text(required(expr, "id")?, "constraint.id")?,
                    predicate_from_expr(required(expr, "predicate")?)?,
                ))
            })
            .collect::<Result<Vec<_>, HarmonyError>>()?,
        soft: sequence(required(expr, "soft")?, "rules.soft")?
            .iter()
            .map(|expr| {
                require_tag(expr, "weighted")?;
                Ok(Weighted::new(
                    text(required(expr, "id")?, "weighted.id")?,
                    parse(required(expr, "weight")?, "weighted.weight")?,
                    metric_from_expr(required(expr, "metric")?)?,
                ))
            })
            .collect::<Result<Vec<_>, HarmonyError>>()?,
    };
    rules.validate()?;
    Ok(rules)
}

fn predicate_to_expr(predicate: &HarmonyPredicate) -> Expr {
    match predicate {
        HarmonyPredicate::Always => tagged("always", Vec::new()),
        HarmonyPredicate::MelodyInChord => tagged("melody-in-chord", Vec::new()),
        HarmonyPredicate::ChordAt { position, chord } => tagged(
            "chord-at",
            vec![
                qualified_entry("position", scalar(position)),
                qualified_entry("chord", scalar(chord.bits())),
            ],
        ),
        HarmonyPredicate::ChordEverywhereExcept { position, chord } => tagged(
            "chord-everywhere-except",
            vec![
                qualified_entry("position", scalar(position)),
                qualified_entry("chord", scalar(chord.bits())),
            ],
        ),
        HarmonyPredicate::ChordOnlyAt { position, chord } => tagged(
            "chord-only-at",
            vec![
                qualified_entry("position", scalar(position)),
                qualified_entry("chord", scalar(chord.bits())),
            ],
        ),
        HarmonyPredicate::AtPosition { position } => tagged(
            "at-position",
            vec![qualified_entry("position", scalar(position))],
        ),
        HarmonyPredicate::DistinctPitchClasses { count } => tagged(
            "distinct-pitch-classes",
            vec![qualified_entry("count", range_to_expr(*count))],
        ),
        HarmonyPredicate::CommonNotes { count } => tagged(
            "common-notes",
            vec![qualified_entry("count", range_to_expr(*count))],
        ),
        HarmonyPredicate::CommonNotePattern { counts } => tagged(
            "common-note-pattern",
            vec![qualified_entry(
                "counts",
                vector(counts.iter().map(scalar).collect()),
            )],
        ),
        HarmonyPredicate::MinimumChordDistance { distance } => tagged(
            "minimum-chord-distance",
            vec![qualified_entry("distance", scalar(distance))],
        ),
        HarmonyPredicate::MaximumChordDistance { distance } => tagged(
            "maximum-chord-distance",
            vec![qualified_entry("distance", scalar(distance))],
        ),
        HarmonyPredicate::MinimumTypeDistance { distance } => tagged(
            "minimum-type-distance",
            vec![qualified_entry("distance", scalar(distance))],
        ),
        HarmonyPredicate::PeriodicVariation { period } => tagged(
            "periodic-variation",
            vec![qualified_entry("period", scalar(period))],
        ),
        HarmonyPredicate::PeriodicCommonality { period, count } => tagged(
            "periodic-commonality",
            vec![
                qualified_entry("period", scalar(period)),
                qualified_entry("count", range_to_expr(*count)),
            ],
        ),
        HarmonyPredicate::InsideScaleWindow { scale, length } => tagged(
            "inside-scale-window",
            vec![
                qualified_entry("scale", scale_to_expr(*scale)),
                qualified_entry("length", scalar(length)),
            ],
        ),
        HarmonyPredicate::OutsideScaleWindow { scale, length } => tagged(
            "outside-scale-window",
            vec![
                qualified_entry("scale", scale_to_expr(*scale)),
                qualified_entry("length", scalar(length)),
            ],
        ),
        HarmonyPredicate::TemplateLength => tagged("template-length", Vec::new()),
        HarmonyPredicate::TemplatesConnect => tagged("templates-connect", Vec::new()),
        HarmonyPredicate::TemplateMelodyInChord => tagged("template-melody-in-chord", Vec::new()),
        HarmonyPredicate::ObserveDepth => tagged("observe-depth", Vec::new()),
        HarmonyPredicate::All(predicates) => tagged(
            "all",
            vec![qualified_entry(
                "predicates",
                vector(predicates.iter().map(predicate_to_expr).collect()),
            )],
        ),
        HarmonyPredicate::Any(predicates) => tagged(
            "any",
            vec![qualified_entry(
                "predicates",
                vector(predicates.iter().map(predicate_to_expr).collect()),
            )],
        ),
        HarmonyPredicate::Not(predicate) => tagged(
            "not",
            vec![qualified_entry("predicate", predicate_to_expr(predicate))],
        ),
    }
}

fn predicate_from_expr(expr: &Expr) -> Result<HarmonyPredicate, HarmonyError> {
    let predicate = match tag(expr)? {
        "always" => HarmonyPredicate::Always,
        "melody-in-chord" => HarmonyPredicate::MelodyInChord,
        "chord-at" => HarmonyPredicate::ChordAt {
            position: parse(required(expr, "position")?, "predicate.position")?,
            chord: mask(required(expr, "chord")?, "predicate.chord")?,
        },
        "chord-everywhere-except" => HarmonyPredicate::ChordEverywhereExcept {
            position: parse(required(expr, "position")?, "predicate.position")?,
            chord: mask(required(expr, "chord")?, "predicate.chord")?,
        },
        "chord-only-at" => HarmonyPredicate::ChordOnlyAt {
            position: parse(required(expr, "position")?, "predicate.position")?,
            chord: mask(required(expr, "chord")?, "predicate.chord")?,
        },
        "at-position" => HarmonyPredicate::AtPosition {
            position: parse(required(expr, "position")?, "predicate.position")?,
        },
        "distinct-pitch-classes" => HarmonyPredicate::DistinctPitchClasses {
            count: range_from_expr(required(expr, "count")?)?,
        },
        "common-notes" => HarmonyPredicate::CommonNotes {
            count: range_from_expr(required(expr, "count")?)?,
        },
        "common-note-pattern" => HarmonyPredicate::CommonNotePattern {
            counts: scalars(required(expr, "counts")?, "predicate.counts")?,
        },
        "minimum-chord-distance" => HarmonyPredicate::MinimumChordDistance {
            distance: parse(required(expr, "distance")?, "predicate.distance")?,
        },
        "maximum-chord-distance" => HarmonyPredicate::MaximumChordDistance {
            distance: parse(required(expr, "distance")?, "predicate.distance")?,
        },
        "minimum-type-distance" => HarmonyPredicate::MinimumTypeDistance {
            distance: parse(required(expr, "distance")?, "predicate.distance")?,
        },
        "periodic-variation" => HarmonyPredicate::PeriodicVariation {
            period: parse(required(expr, "period")?, "predicate.period")?,
        },
        "periodic-commonality" => HarmonyPredicate::PeriodicCommonality {
            period: parse(required(expr, "period")?, "predicate.period")?,
            count: range_from_expr(required(expr, "count")?)?,
        },
        "inside-scale-window" => HarmonyPredicate::InsideScaleWindow {
            scale: scale_from_expr(required(expr, "scale")?)?,
            length: parse(required(expr, "length")?, "predicate.length")?,
        },
        "outside-scale-window" => HarmonyPredicate::OutsideScaleWindow {
            scale: scale_from_expr(required(expr, "scale")?)?,
            length: parse(required(expr, "length")?, "predicate.length")?,
        },
        "template-length" => HarmonyPredicate::TemplateLength,
        "templates-connect" => HarmonyPredicate::TemplatesConnect,
        "template-melody-in-chord" => HarmonyPredicate::TemplateMelodyInChord,
        "observe-depth" => HarmonyPredicate::ObserveDepth,
        "all" => HarmonyPredicate::All(
            sequence(required(expr, "predicates")?, "predicate.predicates")?
                .iter()
                .map(predicate_from_expr)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        "any" => HarmonyPredicate::Any(
            sequence(required(expr, "predicates")?, "predicate.predicates")?
                .iter()
                .map(predicate_from_expr)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        "not" => {
            HarmonyPredicate::Not(Box::new(predicate_from_expr(required(expr, "predicate")?)?))
        }
        other => return Err(invalid(format!("unknown harmony predicate {other}"))),
    };
    predicate.validate()?;
    Ok(predicate)
}

fn metric_to_expr(metric: &HarmonyMetric) -> Expr {
    match metric {
        HarmonyMetric::DistinctPitchClasses => tagged("metric-distinct-pitches", Vec::new()),
        HarmonyMetric::CommonNotes => tagged("metric-common-notes", Vec::new()),
        HarmonyMetric::VoiceLeading => tagged("metric-voice-leading", Vec::new()),
        HarmonyMetric::PitchDissonance { model } => tagged(
            "metric-pitch-dissonance",
            vec![qualified_entry("model", string(model))],
        ),
        HarmonyMetric::ContextualSonance { model } => tagged(
            "metric-contextual-sonance",
            vec![qualified_entry("model", string(model))],
        ),
        HarmonyMetric::RatioComplexity { exponent_milli } => tagged(
            "metric-ratio-complexity",
            vec![qualified_entry("exponent-milli", scalar(exponent_milli))],
        ),
    }
}

fn metric_from_expr(expr: &Expr) -> Result<HarmonyMetric, HarmonyError> {
    match tag(expr)? {
        "metric-distinct-pitches" => Ok(HarmonyMetric::DistinctPitchClasses),
        "metric-common-notes" => Ok(HarmonyMetric::CommonNotes),
        "metric-voice-leading" => Ok(HarmonyMetric::VoiceLeading),
        "metric-pitch-dissonance" => Ok(HarmonyMetric::PitchDissonance {
            model: text(required(expr, "model")?, "metric.model")?.to_owned(),
        }),
        "metric-contextual-sonance" => Ok(HarmonyMetric::ContextualSonance {
            model: text(required(expr, "model")?, "metric.model")?.to_owned(),
        }),
        "metric-ratio-complexity" => Ok(HarmonyMetric::RatioComplexity {
            exponent_milli: parse(required(expr, "exponent-milli")?, "metric.exponent-milli")?,
        }),
        other => Err(invalid(format!("unknown harmony metric {other}"))),
    }
}

fn range_to_expr(range: CountRange) -> Expr {
    tagged(
        "count-range",
        vec![
            qualified_entry("min", scalar(range.min)),
            qualified_entry("max", scalar(range.max)),
        ],
    )
}

fn range_from_expr(expr: &Expr) -> Result<CountRange, HarmonyError> {
    require_tag(expr, "count-range")?;
    CountRange::new(
        parse(required(expr, "min")?, "count-range.min")?,
        parse(required(expr, "max")?, "count-range.max")?,
    )
}
