use std::collections::BTreeMap;
use std::time::Duration;

use sim_lib_music_core::{Score, Staff};
use sim_lib_music_lift::MidiTimelineRealization;
use sim_lib_pitch_dissonance::{
    ContextualPitch, ContextualSonanceComponent, ContextualSonanceRegistry,
    PitchDissonanceRegistry, PitchDissonanceScore,
};
use sim_lib_pitch_set::PitchClassMask;
use sim_lib_sound_core::{Amplitude, Tone};
use sim_lib_sound_dissonance::{DissonanceRegistry, DissonanceScore};
use sim_lib_sound_tuning::Tuning;

use crate::source::{self, SourceMaterial};
use crate::windows::windows_from_notes;
use crate::{
    ConsonanceError, ConsonancePolicy, ConsonanceReport, MetricReport, SoundingNote,
    SoundingWindow, WindowSonance,
};

/// Evaluates a canonical score through exact sounding windows.
pub fn evaluate(
    score: &Score,
    policy: &ConsonancePolicy,
) -> Result<ConsonanceReport, ConsonanceError> {
    evaluate_source(source::from_score(score)?, policy)
}

/// Evaluates an identity-bearing staff without deriving replacement identities.
pub fn evaluate_staff(
    staff: &Staff,
    policy: &ConsonancePolicy,
) -> Result<ConsonanceReport, ConsonanceError> {
    evaluate_source(source::from_staff(staff)?, policy)
}

/// Evaluates a pedal- and overlap-realized MIDI timeline.
pub fn evaluate_midi_timeline(
    timeline: &MidiTimelineRealization,
    policy: &ConsonancePolicy,
) -> Result<ConsonanceReport, ConsonanceError> {
    evaluate_source(source::from_midi_timeline(timeline)?, policy)
}

fn evaluate_source(
    source: SourceMaterial,
    policy: &ConsonancePolicy,
) -> Result<ConsonanceReport, ConsonanceError> {
    let windows = windows_from_notes(&source.notes, source.duration)?;
    let mut previous = Vec::new();
    let mut reports = Vec::with_capacity(windows.len());
    for window in windows {
        reports.push(evaluate_window(&previous, window.clone(), policy)?);
        previous = window.notes;
    }
    Ok(ConsonanceReport {
        windows: reports,
        provenance: source.provenance,
    })
}

fn evaluate_window(
    previous: &[SoundingNote],
    window: SoundingWindow,
    policy: &ConsonancePolicy,
) -> Result<WindowSonance, ConsonanceError> {
    let pitch = pitch_metrics(&window.notes, policy)?;
    let acoustic = acoustic_metrics(&window.notes, policy)?;
    let contextual = contextual_metrics(previous, &window.notes, policy);
    Ok(WindowSonance {
        window,
        pitch,
        acoustic,
        ratio: contextual
            .get("ratio")
            .expect("requested built-in ratio model exists")
            .clone(),
        commonality: contextual
            .get("commonality")
            .expect("requested built-in commonality model exists")
            .clone(),
        leading: contextual
            .get("leading")
            .expect("requested built-in leading model exists")
            .clone(),
    })
}

fn pitch_metrics(
    notes: &[SoundingNote],
    policy: &ConsonancePolicy,
) -> Result<Vec<MetricReport>, ConsonanceError> {
    let pitch_classes = notes
        .iter()
        .map(|note| note.pitch.class)
        .collect::<Vec<_>>();
    let mask = PitchClassMask::from_pitch_classes(&pitch_classes);
    let available = PitchDissonanceRegistry::new_with_builtins()
        .analyze_all(mask, &policy.pitch_context)
        .into_iter()
        .map(|score| (score.model.to_owned(), score))
        .collect::<BTreeMap<_, _>>();
    policy
        .pitch_models
        .iter()
        .map(|name| {
            let score = available
                .get(name)
                .ok_or_else(|| ConsonanceError::UnknownModel {
                    domain: "pitch",
                    model: name.clone(),
                })?;
            if notes.is_empty() {
                Ok(silent_metric(name, "pitch-class-opportunity"))
            } else {
                Ok(pitch_report(score, notes.len(), mask.count_bits() as usize))
            }
        })
        .collect()
}

fn acoustic_metrics(
    notes: &[SoundingNote],
    policy: &ConsonancePolicy,
) -> Result<Vec<MetricReport>, ConsonanceError> {
    let tones = notes
        .iter()
        .map(|note| {
            let mut tone = Tone::sine(
                policy.tuning.frequency_of(note.pitch),
                Duration::from_secs(1),
            );
            tone.partials[0].amplitude = Amplitude(f64::from(note.velocity) / 127.0);
            tone
        })
        .collect::<Vec<_>>();
    let registry = DissonanceRegistry::new_with_builtins();
    policy
        .acoustic_models
        .iter()
        .map(|name| {
            let model = registry
                .get(name)
                .ok_or_else(|| ConsonanceError::UnknownModel {
                    domain: "acoustic",
                    model: name.clone(),
                })?;
            let sonance = model
                .sonance_of_chord(&tones)
                .map_err(|error| ConsonanceError::Acoustic(error.to_string()))?;
            Ok(acoustic_report(
                DissonanceScore {
                    model: name.clone(),
                    score: sonance.compatibility_score(),
                    sonance,
                },
                &tones,
            ))
        })
        .collect()
}

fn contextual_metrics(
    previous: &[SoundingNote],
    current: &[SoundingNote],
    policy: &ConsonancePolicy,
) -> BTreeMap<String, MetricReport> {
    if previous.is_empty() && current.is_empty() {
        return ["ratio", "commonality", "leading"]
            .into_iter()
            .map(|model| {
                (
                    model.to_owned(),
                    silent_metric(model, "context-transition-opportunity"),
                )
            })
            .collect();
    }
    let from = contextual_notes(previous);
    let to = contextual_notes(current);
    ContextualSonanceRegistry::new_with_builtins()
        .compare_named(
            &["ratio", "commonality", "leading"],
            &from,
            &to,
            policy.contextual,
        )
        .components
        .into_iter()
        .map(|component| {
            (
                component.model.to_owned(),
                contextual_report(component, previous.len(), current.len()),
            )
        })
        .collect()
}

fn contextual_notes(notes: &[SoundingNote]) -> Vec<ContextualPitch> {
    notes
        .iter()
        .map(|note| ContextualPitch {
            id: note.event_id.to_string(),
            voice: Some(note.voice_id.to_string()),
            pitch: note.pitch,
            amplitude: f64::from(note.velocity) / 127.0,
        })
        .collect()
}

fn pitch_report(
    score: &PitchDissonanceScore,
    event_count: usize,
    pitch_class_count: usize,
) -> MetricReport {
    MetricReport {
        model: score.model.to_owned(),
        roughness_mass: score.sonance.roughness_mass,
        normalized_density: score.sonance.normalized_density,
        harmonic_context: score.sonance.harmonic_context,
        normalization: score.sonance.evidence.normalization.to_owned(),
        aggregation: score.sonance.evidence.aggregation.to_owned(),
        evidence: score
            .sonance
            .evidence
            .provenance
            .iter()
            .cloned()
            .chain([
                format!("dialect={}", score.sonance.evidence.dialect),
                format!("event-count={event_count}"),
                format!("distinct-pitch-classes={pitch_class_count}"),
            ])
            .collect(),
    }
}

fn acoustic_report(score: DissonanceScore, tones: &[Tone]) -> MetricReport {
    let policy = score.sonance.evidence.partial_policy;
    let frequencies = tones
        .iter()
        .flat_map(|tone| tone.partials.iter())
        .map(|partial| partial.frequency.0.to_string())
        .collect::<Vec<_>>()
        .join(",");
    MetricReport {
        model: score.model,
        roughness_mass: score.sonance.roughness_mass,
        normalized_density: score.sonance.normalized_density,
        harmonic_context: score.sonance.harmonic_context,
        normalization: score.sonance.evidence.normalization.to_owned(),
        aggregation: score.sonance.evidence.aggregation.to_owned(),
        evidence: score
            .sonance
            .evidence
            .provenance
            .into_iter()
            .chain([
                format!("curve-family={}", score.sonance.evidence.curve_family),
                format!("audible-partials={}", policy.audible_partials),
                format!("inaudible-partials={}", policy.inaudible_partials),
                format!("evaluated-pairs={}", policy.evaluated_pairs),
                format!("frequencies-hz=[{frequencies}]"),
            ])
            .collect(),
    }
}

fn contextual_report(
    component: ContextualSonanceComponent,
    from_count: usize,
    to_count: usize,
) -> MetricReport {
    MetricReport {
        model: component.model.to_owned(),
        roughness_mass: component.sonance.roughness_mass,
        normalized_density: component.sonance.normalized_density,
        harmonic_context: component.sonance.harmonic_context,
        normalization: component.sonance.evidence.normalization.to_owned(),
        aggregation: component.sonance.evidence.aggregation.to_owned(),
        evidence: component
            .sonance
            .evidence
            .provenance
            .into_iter()
            .chain([
                format!("dialect={}", component.sonance.evidence.dialect),
                format!("from-events={from_count}"),
                format!("to-events={to_count}"),
            ])
            .collect(),
    }
}

fn silent_metric(model: &str, opportunity: &str) -> MetricReport {
    MetricReport {
        model: model.to_owned(),
        roughness_mass: 0.0,
        normalized_density: 0.0,
        harmonic_context: 0.0,
        normalization: "empty-window".to_owned(),
        aggregation: "identity".to_owned(),
        evidence: vec![
            "event-count=0".to_owned(),
            format!("{opportunity}=0"),
            "silence=true".to_owned(),
        ],
    }
}
