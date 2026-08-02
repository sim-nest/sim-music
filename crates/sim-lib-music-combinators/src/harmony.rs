use sim_lib_discrete_search::{SearchControl, SearchInterrupt};
use sim_lib_music_core::{Channel, Chord, Progression, Time};
use sim_lib_pitch_chord::{
    ChordTemplate, CoreHarmonyMetricResolver, HarmonizationRequest, HarmonizationRun,
    HarmonizationStrategy, HarmonyError, HarmonyEvaluation, HarmonyEvaluationContext,
    HarmonyMetric, HarmonyMetricObservation, HarmonyMetricResolver, HarmonyRenderProfile,
    HarmonyRuleSet, evaluate_harmony, plan_harmony,
};
use sim_lib_pitch_dissonance::{
    ContextualPitch, ContextualSonanceOptions, ContextualSonanceRegistry, PitchDissonanceRegistry,
};
use sim_lib_pitch_namer::LabelContext;

/// Soft-metric resolver composing chord-local metrics with current sonance registries.
pub struct DeclarativeHarmonyResolver {
    core: CoreHarmonyMetricResolver,
    pitch: PitchDissonanceRegistry,
    contextual: ContextualSonanceRegistry,
    label_context: LabelContext,
    contextual_options: ContextualSonanceOptions,
}

impl DeclarativeHarmonyResolver {
    /// Builds a resolver with every built-in pitch and contextual sonance model.
    pub fn new() -> Self {
        Self {
            core: CoreHarmonyMetricResolver,
            pitch: PitchDissonanceRegistry::new_with_builtins(),
            contextual: ContextualSonanceRegistry::new_with_builtins(),
            label_context: LabelContext::default(),
            contextual_options: ContextualSonanceOptions::standard(),
        }
    }

    /// Sets root/key context used by pitch-dissonance models.
    pub fn with_label_context(mut self, context: LabelContext) -> Self {
        self.label_context = context;
        self
    }

    /// Sets duplicate, normalization, merge, voice, ratio, and window policy.
    pub fn with_contextual_options(mut self, options: ContextualSonanceOptions) -> Self {
        self.contextual_options = options;
        self
    }
}

impl Default for DeclarativeHarmonyResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl HarmonyMetricResolver for DeclarativeHarmonyResolver {
    fn evaluate(
        &self,
        metric: &HarmonyMetric,
        context: HarmonyEvaluationContext<'_>,
    ) -> Result<HarmonyMetricObservation, HarmonyError> {
        match metric {
            HarmonyMetric::PitchDissonance { model } => {
                let Some(chord) = context.progression.last() else {
                    return Ok(no_current_chord());
                };
                let score = self
                    .pitch
                    .analyze_all(chord.pitch_set()?, &self.label_context)
                    .into_iter()
                    .find(|score| score.model == model)
                    .ok_or_else(|| HarmonyError::UnknownMetricModel(model.clone()))?;
                let mut facts = vec![
                    format!("model={}", score.model),
                    format!("normalization={}", score.sonance.evidence.normalization),
                    format!("aggregation={}", score.sonance.evidence.aggregation),
                    format!("dialect={}", score.sonance.evidence.dialect),
                ];
                facts.extend(score.sonance.evidence.provenance);
                Ok(HarmonyMetricObservation {
                    value: score.score,
                    facts,
                })
            }
            HarmonyMetric::ContextualSonance { model } => {
                if !self.contextual.list().contains(&model.as_str()) {
                    return Err(HarmonyError::UnknownMetricModel(model.clone()));
                }
                let [.., from, to] = context.progression else {
                    return Ok(no_current_chord());
                };
                let from = contextual_pitches(from)?;
                let to = contextual_pitches(to)?;
                let report = self.contextual.compare_named(
                    &[model.as_str()],
                    &from,
                    &to,
                    self.contextual_options,
                );
                let component = report
                    .components
                    .first()
                    .ok_or_else(|| HarmonyError::UnknownMetricModel(model.clone()))?;
                let mut facts = vec![
                    format!("model={}", component.model),
                    format!("normalization={}", component.sonance.evidence.normalization),
                    format!("aggregation={}", component.sonance.evidence.aggregation),
                    format!("dialect={}", component.sonance.evidence.dialect),
                    format!("from-events={}", report.from.ids.len()),
                    format!("to-events={}", report.to.ids.len()),
                ];
                facts.extend(component.sonance.evidence.provenance.clone());
                Ok(HarmonyMetricObservation {
                    value: component.score,
                    facts,
                })
            }
            _ => self.core.evaluate(metric, context),
        }
    }
}

/// Evaluates a declarative rule set using chord, ratio, and sonance owners.
pub fn evaluate_declarative_harmony(
    rules: &HarmonyRuleSet,
    context: HarmonyEvaluationContext<'_>,
    resolver: &DeclarativeHarmonyResolver,
) -> Result<HarmonyEvaluation, HarmonyError> {
    evaluate_harmony(rules, context, resolver)
}

/// Harmonizes one declarative request with the installed musical metric registries.
///
/// Local and global strategies share the same palette and rule evaluation.
/// Search bounds, failed-rule evidence, heuristic declarations, and optimality
/// certificates remain available in the returned receipt.
pub fn harmonize(
    request: &HarmonizationRequest,
    strategy: HarmonizationStrategy,
    control: SearchControl,
    interrupt: &dyn SearchInterrupt,
) -> Result<HarmonizationRun, HarmonyError> {
    plan_harmony(
        request,
        strategy,
        control,
        interrupt,
        &DeclarativeHarmonyResolver::new(),
    )
}

/// A rendered canonical progression plus retained export settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarmonyRenderPlan {
    /// Canonical progression built from exact chord-template pitches and durations.
    pub progression: Progression,
    /// Data-only export profile retained for MIDI/notation adapters.
    pub profile: HarmonyRenderProfile,
}

/// Converts canonical music chords to exact declarative chord templates.
pub fn chord_templates_from_progression(progression: &Progression) -> Vec<ChordTemplate> {
    progression
        .chords
        .iter()
        .enumerate()
        .map(|(index, chord)| {
            ChordTemplate::from_pitches(format!("progression/chord/{index}"), chord.pitches.clone())
        })
        .collect()
}

/// Renders templates with exact durations while retaining the complete export profile.
pub fn render_harmony_progression(
    templates: &[ChordTemplate],
    durations: &[Time],
    profile: &HarmonyRenderProfile,
) -> Result<HarmonyRenderPlan, HarmonyError> {
    profile.validate()?;
    if templates.len() != durations.len() {
        return Err(HarmonyError::InvalidField {
            field: "render.durations",
            reason: format!(
                "received {} durations for {} chords",
                durations.len(),
                templates.len()
            ),
        });
    }
    let multiplier = Time::from_integer(i64::from(profile.duration_multiplier));
    let chords = templates
        .iter()
        .zip(durations)
        .enumerate()
        .map(|(index, (template, duration))| {
            let pitches = template
                .realize()?
                .notes
                .into_iter()
                .map(|pitch| pitch.transpose(profile.chord_transpose))
                .collect();
            Chord::new(
                *duration * multiplier,
                template.id.clone(),
                pitches,
                100,
                Channel::new((index % 16) as u8).map_err(|error| HarmonyError::InvalidField {
                    field: "render.channel",
                    reason: error.to_string(),
                })?,
            )
            .map_err(|error| HarmonyError::InvalidField {
                field: "render.chord",
                reason: error.to_string(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let progression =
        Progression::new(None, chords).map_err(|error| HarmonyError::InvalidField {
            field: "render.progression",
            reason: error.to_string(),
        })?;
    Ok(HarmonyRenderPlan {
        progression,
        profile: profile.clone(),
    })
}

fn contextual_pitches(template: &ChordTemplate) -> Result<Vec<ContextualPitch>, HarmonyError> {
    Ok(template
        .realize()?
        .notes
        .into_iter()
        .enumerate()
        .map(|(index, pitch)| ContextualPitch {
            id: format!("{}/event/{index}", template.id),
            voice: Some(format!("voice/{index}")),
            pitch,
            amplitude: 1.0,
        })
        .collect())
}

fn no_current_chord() -> HarmonyMetricObservation {
    HarmonyMetricObservation {
        value: 0.0,
        facts: vec!["no-current-chord".to_owned()],
    }
}

#[cfg(test)]
mod tests {
    use num_rational::Ratio;
    use sim_lib_discrete_search::{NeverInterrupt, SearchStatus};
    use sim_lib_music_core::{Pitch, PitchClass};
    use sim_lib_pitch_chord::{
        CountRange, HarmonizationRequest, HarmonizationStrategy, HarmonyConstraint, HarmonyMetric,
        HarmonyPredicate, Weighted,
    };
    use sim_lib_pitch_scale::Scale;

    use super::*;

    fn template(id: &str, midi: &[u8]) -> ChordTemplate {
        ChordTemplate::from_pitches(id, midi.iter().copied().map(Pitch::from_midi).collect())
    }

    #[test]
    fn resolver_composes_pitch_and_contextual_sonance_registries() {
        let progression = vec![template("c", &[60, 64, 67]), template("f", &[60, 65, 69])];
        let rules = HarmonyRuleSet {
            hard: Vec::new(),
            soft: vec![
                Weighted::new(
                    "pitch",
                    1.0,
                    HarmonyMetric::PitchDissonance {
                        model: "interval-vector".to_owned(),
                    },
                ),
                Weighted::new(
                    "context",
                    1.0,
                    HarmonyMetric::ContextualSonance {
                        model: "commonality".to_owned(),
                    },
                ),
            ],
        };
        let result = evaluate_declarative_harmony(
            &rules,
            HarmonyEvaluationContext::progression(&[], &progression),
            &DeclarativeHarmonyResolver::new(),
        )
        .unwrap();

        assert!(result.legal);
        assert_eq!(result.soft.len(), 2);
        assert!(
            result.soft[0]
                .facts
                .iter()
                .any(|fact| fact == "model=interval-vector")
        );
        assert!(
            result.soft[1]
                .facts
                .iter()
                .any(|fact| fact == "model=commonality")
        );
    }

    #[test]
    fn progression_adapter_preserves_register_and_render_profile() {
        let core = Progression::new(
            None,
            vec![
                Chord::new(
                    Ratio::new(1, 4),
                    "C",
                    vec![
                        Pitch::from_midi(48),
                        Pitch::from_midi(64),
                        Pitch::from_midi(79),
                    ],
                    90,
                    Channel::new(0).unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let templates = chord_templates_from_progression(&core);
        assert_eq!(
            templates[0].realize().unwrap().notes,
            core.chords[0].pitches
        );

        let profile = HarmonyRenderProfile {
            id: "render".to_owned(),
            chord_transpose: 0,
            melody_transpose: 60,
            duration_multiplier: 4,
            chord_program: 19,
            melody_program: 56,
            tempo_bpm: 60,
            time_signature: (4, 4),
        };
        let plan = render_harmony_progression(&templates, &[Ratio::new(1, 4)], &profile).unwrap();
        assert_eq!(plan.progression.chords[0].duration, Ratio::from_integer(1));
        assert_eq!(plan.progression.chords[0].pitches, core.chords[0].pitches);
        assert_eq!(plan.profile, profile);
        assert_eq!(PitchClass::C, plan.progression.chords[0].pitches[0].class);
    }

    #[test]
    fn harmonizer_composes_every_declared_musical_constraint_lane() {
        let c = template("c", &[48, 52, 55, 60]);
        let f = template("f", &[53, 57, 60, 65]);
        let g7 = template("g7", &[55, 59, 62, 65]);
        let c_mask = c.pitch_set().unwrap();
        let request = HarmonizationRequest {
            melody: vec![
                c.pitch_set().unwrap(),
                f.pitch_set().unwrap(),
                c.pitch_set().unwrap(),
            ],
            palette: sim_lib_pitch_chord::ChordPalette::explicit(
                "full-rules",
                vec![c, f, g7],
                Vec::new(),
            )
            .unwrap(),
            rules: HarmonyRuleSet {
                hard: vec![
                    HarmonyConstraint::new("melody-fit", HarmonyPredicate::MelodyInChord),
                    HarmonyConstraint::new(
                        "scale",
                        HarmonyPredicate::InsideScaleWindow {
                            scale: Scale::major(PitchClass::C),
                            length: 2,
                        },
                    ),
                    HarmonyConstraint::new(
                        "common-tone",
                        HarmonyPredicate::CommonNotes {
                            count: CountRange::new(1, 4).unwrap(),
                        },
                    ),
                    HarmonyConstraint::new(
                        "range",
                        HarmonyPredicate::PitchRange {
                            min_midi: 48,
                            max_midi: 72,
                        },
                    ),
                    HarmonyConstraint::new(
                        "repetition",
                        HarmonyPredicate::MinimumChordDistance { distance: 1 },
                    ),
                    HarmonyConstraint::new(
                        "cadence",
                        HarmonyPredicate::ChordAt {
                            position: -1,
                            chord: c_mask,
                        },
                    ),
                ],
                soft: vec![
                    Weighted::new("voice-leading", 1.0, HarmonyMetric::VoiceLeading),
                    Weighted::new(
                        "sonance",
                        -1.0,
                        HarmonyMetric::ContextualSonance {
                            model: "commonality".to_owned(),
                        },
                    ),
                    Weighted::new(
                        "pitch-dissonance",
                        1.0,
                        HarmonyMetric::PitchDissonance {
                            model: "interval-vector".to_owned(),
                        },
                    ),
                ],
            },
        };
        let run = harmonize(
            &request,
            HarmonizationStrategy::LayeredDp,
            SearchControl::default()
                .with_max_work(100_000)
                .with_max_memory_nodes(1_000),
            &NeverInterrupt,
        )
        .unwrap();

        assert_eq!(run.receipt.status, SearchStatus::Complete);
        assert!(run.receipt.optimal);
        assert_eq!(run.results[0].evaluations.len(), 3);
        assert!(
            run.results[0]
                .evaluations
                .iter()
                .flat_map(|evaluation| &evaluation.soft)
                .any(|evidence| {
                    evidence.metric_id == "sonance"
                        && evidence
                            .facts
                            .iter()
                            .any(|fact| fact == "model=commonality")
                })
        );
        assert!(
            run.receipt
                .rejections
                .iter()
                .any(|rejection| rejection.rule_id == "cadence")
        );
    }
}
