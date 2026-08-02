use sim_lib_pitch_set::PitchClassMask;

use crate::{
    ChordTemplate, HarmonyError, HarmonyMetric, HarmonyPredicate, HarmonyRuleSet, TemplateChain,
};

/// Musical inputs visible to hard constraints and soft metrics.
#[derive(Clone, Copy, Debug)]
pub struct HarmonyEvaluationContext<'a> {
    /// Required melody pitch sets in complete phrase order.
    pub melody: &'a [PitchClassMask],
    /// Current candidate progression prefix.
    pub progression: &'a [ChordTemplate],
    /// Current cadence-template chain prefix.
    pub templates: &'a [TemplateChain],
}

impl<'a> HarmonyEvaluationContext<'a> {
    /// Builds a context for an ordinary chord-progression prefix.
    pub fn progression(melody: &'a [PitchClassMask], progression: &'a [ChordTemplate]) -> Self {
        Self {
            melody,
            progression,
            templates: &[],
        }
    }

    /// Attaches the template-chain prefix used by template-level predicates.
    pub fn with_templates(mut self, templates: &'a [TemplateChain]) -> Self {
        self.templates = templates;
        self
    }
}

/// Retained result of one named hard rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarmonyRuleEvidence {
    /// Rule id from the data file.
    pub rule_id: String,
    /// Whether this rule passed.
    pub passed: bool,
    /// Deterministic facts used by the decision.
    pub facts: Vec<String>,
}

/// Unweighted observation returned by a soft metric resolver.
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonyMetricObservation {
    /// Raw metric value.
    pub value: f64,
    /// Deterministic facts and model provenance.
    pub facts: Vec<String>,
}

/// Retained result of one named weighted soft metric.
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonyMetricEvidence {
    /// Metric id from the data file.
    pub metric_id: String,
    /// Raw metric value.
    pub value: f64,
    /// Caller-declared weight.
    pub weight: f64,
    /// Exact `value * weight` contribution.
    pub weighted_score: f64,
    /// Deterministic facts and model provenance.
    pub facts: Vec<String>,
}

/// Complete hard/soft evaluation with no legality hidden in the scalar score.
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonyEvaluation {
    /// True exactly when every hard rule passed.
    pub legal: bool,
    /// One evidence row for every hard rule, in declaration order.
    pub hard: Vec<HarmonyRuleEvidence>,
    /// One evidence row for every soft metric, in declaration order.
    pub soft: Vec<HarmonyMetricEvidence>,
    /// Sum of soft weighted contributions, independent of `legal`.
    pub score: f64,
}

/// Resolver seam for soft models owned outside the chord crate.
pub trait HarmonyMetricResolver {
    /// Evaluates one metric without changing hard legality.
    fn evaluate(
        &self,
        metric: &HarmonyMetric,
        context: HarmonyEvaluationContext<'_>,
    ) -> Result<HarmonyMetricObservation, HarmonyError>;
}

/// Evaluates every hard rule and soft metric while retaining per-rule evidence.
pub fn evaluate_harmony(
    rules: &HarmonyRuleSet,
    context: HarmonyEvaluationContext<'_>,
    resolver: &dyn HarmonyMetricResolver,
) -> Result<HarmonyEvaluation, HarmonyError> {
    rules.validate()?;
    let masks = progression_masks(context.progression)?;
    let hard = rules
        .hard
        .iter()
        .map(|rule| {
            let (passed, facts) = evaluate_predicate(&rule.predicate, context, &masks)?;
            Ok(HarmonyRuleEvidence {
                rule_id: rule.id.clone(),
                passed,
                facts,
            })
        })
        .collect::<Result<Vec<_>, HarmonyError>>()?;
    let soft = rules
        .soft
        .iter()
        .map(|weighted| {
            let observation = resolver.evaluate(&weighted.value, context)?;
            let weighted_score = observation.value * weighted.weight;
            if !observation.value.is_finite() || !weighted_score.is_finite() {
                return Err(HarmonyError::InvalidField {
                    field: "metric.result",
                    reason: format!("metric {} produced a non-finite value", weighted.id),
                });
            }
            Ok(HarmonyMetricEvidence {
                metric_id: weighted.id.clone(),
                value: observation.value,
                weight: weighted.weight,
                weighted_score,
                facts: observation.facts,
            })
        })
        .collect::<Result<Vec<_>, HarmonyError>>()?;
    Ok(HarmonyEvaluation {
        legal: hard.iter().all(|evidence| evidence.passed),
        score: soft.iter().map(|evidence| evidence.weighted_score).sum(),
        hard,
        soft,
    })
}

fn evaluate_predicate(
    predicate: &HarmonyPredicate,
    context: HarmonyEvaluationContext<'_>,
    masks: &[PitchClassMask],
) -> Result<(bool, Vec<String>), HarmonyError> {
    let current = masks.last().copied();
    let position = masks.len().checked_sub(1);
    let result = match predicate {
        HarmonyPredicate::Always => decision(true, "accept-all"),
        HarmonyPredicate::MelodyInChord => match (position, current) {
            (Some(index), Some(chord)) if index < context.melody.len() => decision(
                context.melody[index].is_subset_of(chord),
                format!(
                    "melody=0x{:03x},chord=0x{:03x}",
                    context.melody[index].bits(),
                    chord.bits()
                ),
            ),
            _ => decision(true, "empty-prefix"),
        },
        HarmonyPredicate::ChordAt {
            position: at,
            chord,
        } => {
            let target = resolve_position(*at, context.melody.len());
            decision(
                position.is_none() || position != target || current == Some(*chord),
                position_fact(position, target, current),
            )
        }
        HarmonyPredicate::ChordEverywhereExcept {
            position: at,
            chord,
        } => {
            let exempt = resolve_position(*at, context.melody.len());
            decision(
                position.is_none() || position == exempt || current == Some(*chord),
                position_fact(position, exempt, current),
            )
        }
        HarmonyPredicate::ChordOnlyAt {
            position: at,
            chord,
        } => {
            let allowed = resolve_position(*at, context.melody.len());
            decision(
                position.is_none() || position == allowed || current != Some(*chord),
                position_fact(position, allowed, current),
            )
        }
        HarmonyPredicate::AtPosition { position: at } => {
            let target = resolve_position(*at, context.melody.len());
            decision(
                position.is_some() && position == target,
                position_fact(position, target, current),
            )
        }
        HarmonyPredicate::DistinctPitchClasses { count } => {
            let found = current.map_or(0, PitchClassMask::count_bits) as usize;
            decision(
                current.is_none() || count.contains(found),
                format!("distinct={found}"),
            )
        }
        HarmonyPredicate::PitchRange { min_midi, max_midi } => {
            let notes = context
                .progression
                .last()
                .map(ChordTemplate::realize)
                .transpose()?
                .map(|chord| chord.notes)
                .unwrap_or_default();
            let midi = notes
                .iter()
                .map(|pitch| pitch.to_midi())
                .collect::<Vec<_>>();
            decision(
                midi.iter().all(|value| {
                    value.is_some_and(|value| (*min_midi..=*max_midi).contains(&value))
                }),
                format!("midi={midi:?},range={min_midi}..={max_midi}"),
            )
        }
        HarmonyPredicate::CommonNotes { count } => {
            let found = last_common_notes(masks);
            decision(
                found.is_none_or(|value| count.contains(value)),
                format!("common={found:?}"),
            )
        }
        HarmonyPredicate::CommonNotePattern { counts } => {
            let found = last_common_notes(masks);
            let expected = position.map(|index| counts[index % counts.len()]);
            decision(
                found
                    .zip(expected)
                    .is_none_or(|(found, expected)| found == expected),
                format!("common={found:?},expected={expected:?}"),
            )
        }
        HarmonyPredicate::MinimumChordDistance { distance } => {
            let repeated = prior_window(masks, *distance)
                .iter()
                .any(|known| Some(*known) == current);
            decision(!repeated, format!("window={distance},repeated={repeated}"))
        }
        HarmonyPredicate::MaximumChordDistance { distance } => {
            let prior = prior_window(masks, *distance);
            let repeated = prior.iter().any(|known| Some(*known) == current);
            decision(
                masks.len() < 2 || repeated,
                format!("window={distance},repeated={repeated}"),
            )
        }
        HarmonyPredicate::MinimumTypeDistance { distance } => {
            let normalized = current.map(PitchClassMask::normalize);
            let repeated = prior_window(masks, *distance)
                .iter()
                .any(|known| Some(known.normalize()) == normalized);
            decision(
                !repeated,
                format!("window={distance},type-repeated={repeated}"),
            )
        }
        HarmonyPredicate::PeriodicVariation { period } => {
            let matches = periodic_prior(masks, *period)
                .into_iter()
                .filter(|known| Some(**known) == current)
                .count();
            decision(matches == 0, format!("period={period},matches={matches}"))
        }
        HarmonyPredicate::PeriodicCommonality { period, count } => {
            let found = periodic_prior(masks, *period)
                .into_iter()
                .map(|known| intersection_count(*known, current.unwrap_or_default()))
                .collect::<Vec<_>>();
            decision(
                found.iter().all(|value| count.contains(*value)),
                format!("period={period},common={found:?}"),
            )
        }
        HarmonyPredicate::InsideScaleWindow { scale, length } => {
            let fits = scale_window_fits(masks, *scale, *length);
            decision(
                fits.unwrap_or(true),
                format!("length={length},fits={fits:?}"),
            )
        }
        HarmonyPredicate::OutsideScaleWindow { scale, length } => {
            let fits = scale_window_fits(masks, *scale, *length);
            decision(
                !fits.unwrap_or(false),
                format!("length={length},fits={fits:?}"),
            )
        }
        HarmonyPredicate::TemplateLength => {
            let length = flattened_length(context.templates);
            decision(
                length <= context.melody.len(),
                format!("flattened-length={length}"),
            )
        }
        HarmonyPredicate::TemplatesConnect => {
            let connects = templates_connect(context.templates)?;
            decision(
                connects,
                format!("template-count={}", context.templates.len()),
            )
        }
        HarmonyPredicate::TemplateMelodyInChord => {
            let flattened = TemplateChain::flatten_connected(context.templates);
            match flattened {
                Ok(chords) => {
                    let chord_masks = progression_masks(&chords)?;
                    let fits = chord_masks
                        .iter()
                        .zip(context.melody)
                        .all(|(chord, melody)| melody.is_subset_of(*chord));
                    decision(fits, format!("checked-chords={}", chord_masks.len()))
                }
                Err(_) => decision(false, "templates-disconnected"),
            }
        }
        HarmonyPredicate::ObserveDepth => decision(true, format!("depth={}", masks.len())),
        HarmonyPredicate::All(predicates) => compose(predicates, context, masks, true)?,
        HarmonyPredicate::Any(predicates) => compose(predicates, context, masks, false)?,
        HarmonyPredicate::Not(predicate) => {
            let (passed, facts) = evaluate_predicate(predicate, context, masks)?;
            (!passed, facts)
        }
    };
    Ok(result)
}

fn compose(
    predicates: &[HarmonyPredicate],
    context: HarmonyEvaluationContext<'_>,
    masks: &[PitchClassMask],
    all: bool,
) -> Result<(bool, Vec<String>), HarmonyError> {
    let results = predicates
        .iter()
        .map(|predicate| evaluate_predicate(predicate, context, masks))
        .collect::<Result<Vec<_>, _>>()?;
    let passed = if all {
        results.iter().all(|result| result.0)
    } else {
        results.iter().any(|result| result.0)
    };
    let facts = results
        .into_iter()
        .enumerate()
        .map(|(index, result)| format!("child-{index}={}", result.0))
        .collect();
    Ok((passed, facts))
}

pub(crate) fn progression_masks(
    chords: &[ChordTemplate],
) -> Result<Vec<PitchClassMask>, HarmonyError> {
    chords.iter().map(ChordTemplate::pitch_set).collect()
}

fn intersection_count(left: PitchClassMask, right: PitchClassMask) -> usize {
    (left.bits() & right.bits()).count_ones() as usize
}

pub(crate) fn last_common_notes(masks: &[PitchClassMask]) -> Option<usize> {
    match masks {
        [.., left, right] => Some(intersection_count(*left, *right)),
        _ => None,
    }
}

fn prior_window(masks: &[PitchClassMask], distance: usize) -> &[PitchClassMask] {
    let end = masks.len().saturating_sub(1);
    &masks[end.saturating_sub(distance)..end]
}

fn periodic_prior(masks: &[PitchClassMask], period: usize) -> Vec<&PitchClassMask> {
    let Some(current) = masks.len().checked_sub(1) else {
        return Vec::new();
    };
    (1..)
        .map(|multiple| multiple * period)
        .take_while(|offset| *offset <= current)
        .map(|offset| &masks[current - offset])
        .collect()
}

fn scale_window_fits(
    masks: &[PitchClassMask],
    scale: sim_lib_pitch_scale::Scale,
    length: usize,
) -> Option<bool> {
    if masks.len() < length {
        return None;
    }
    let window = &masks[masks.len() - length..];
    Some((0..12).any(|shift| {
        let scale_mask = scale.mask().rotate(shift);
        window.iter().all(|chord| chord.is_subset_of(scale_mask))
    }))
}

fn flattened_length(templates: &[TemplateChain]) -> usize {
    templates
        .iter()
        .map(|template| template.chords.len())
        .sum::<usize>()
        .saturating_sub(templates.len().saturating_sub(1))
}

fn templates_connect(templates: &[TemplateChain]) -> Result<bool, HarmonyError> {
    for pair in templates.windows(2) {
        if pair[0]
            .chords
            .last()
            .map(ChordTemplate::pitch_set)
            .transpose()?
            != pair[1]
                .chords
                .first()
                .map(ChordTemplate::pitch_set)
                .transpose()?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn resolve_position(position: i32, total: usize) -> Option<usize> {
    if position >= 0 {
        Some(position as usize).filter(|position| *position < total)
    } else {
        total.checked_sub(position.unsigned_abs() as usize)
    }
}

fn position_fact(
    current: Option<usize>,
    target: Option<usize>,
    chord: Option<PitchClassMask>,
) -> String {
    format!(
        "current={current:?},target={target:?},chord={:?}",
        chord.map(PitchClassMask::bits)
    )
}

pub(crate) fn observation(value: f64, fact: String) -> HarmonyMetricObservation {
    HarmonyMetricObservation {
        value,
        facts: vec![fact],
    }
}

fn decision(passed: bool, fact: impl Into<String>) -> (bool, Vec<String>) {
    (passed, vec![fact.into()])
}
