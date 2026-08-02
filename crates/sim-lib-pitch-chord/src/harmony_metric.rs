use sim_lib_pitch_ratio::{MeanDialect, RatioPolicy, analyze_ratio_chord_with_root};

use crate::harmony_eval::{last_common_notes, observation, progression_masks};
use crate::{
    HarmonyError, HarmonyEvaluationContext, HarmonyMetric, HarmonyMetricObservation,
    HarmonyMetricResolver, VoicingChange,
};

/// Resolver for chord-local count, voice-leading, and exact-ratio metrics.
#[derive(Clone, Copy, Debug, Default)]
pub struct CoreHarmonyMetricResolver;

impl HarmonyMetricResolver for CoreHarmonyMetricResolver {
    fn evaluate(
        &self,
        metric: &HarmonyMetric,
        context: HarmonyEvaluationContext<'_>,
    ) -> Result<HarmonyMetricObservation, HarmonyError> {
        let masks = progression_masks(context.progression)?;
        match metric {
            HarmonyMetric::DistinctPitchClasses => {
                let value = masks.last().map_or(0, |mask| mask.count_bits()) as f64;
                Ok(observation(
                    value,
                    format!("distinct-pitch-classes={value}"),
                ))
            }
            HarmonyMetric::CommonNotes => {
                let value = last_common_notes(&masks).unwrap_or(0) as f64;
                Ok(observation(value, format!("common-pitch-classes={value}")))
            }
            HarmonyMetric::VoiceLeading => {
                let value = match context.progression {
                    [.., source, target] => {
                        VoicingChange::between("metric/voice-leading", source, target, 12)?.cost
                            as f64
                    }
                    _ => 0.0,
                };
                Ok(observation(
                    value,
                    format!("certified-circular-squared-cost={value}"),
                ))
            }
            HarmonyMetric::RatioComplexity { exponent_milli } => {
                let Some(chord) = context.progression.last() else {
                    return Ok(observation(0.0, "no-current-chord".to_owned()));
                };
                if chord.ratios.is_empty() {
                    return Err(HarmonyError::InvalidField {
                        field: "ratio-complexity",
                        reason: format!("chord {} has no ratio tones", chord.id),
                    });
                }
                let exponent = f64::from(*exponent_milli) / 1_000.0;
                let report = analyze_ratio_chord_with_root(
                    &chord.ratios,
                    0,
                    RatioPolicy::default(),
                    exponent,
                    MeanDialect::Standard,
                )
                .map_err(|error| HarmonyError::InvalidField {
                    field: "ratio-complexity",
                    reason: error.to_string(),
                })?;
                Ok(HarmonyMetricObservation {
                    value: report.cost,
                    facts: vec![
                        format!("ratio-tones={}", report.covered.admitted_tones),
                        format!("ratio-exponent={exponent}"),
                        format!("ratio-matrix-entries={}", report.covered.matrix_entries),
                    ],
                })
            }
            HarmonyMetric::PitchDissonance { model }
            | HarmonyMetric::ContextualSonance { model }
            | HarmonyMetric::LearnedTransition { model } => {
                Err(HarmonyError::UnknownMetricModel(model.clone()))
            }
        }
    }
}
