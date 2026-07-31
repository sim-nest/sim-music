use std::collections::BTreeSet;

use sim_lib_music_core::{Articulation, ObjectId, Pitch, Staff, StaffNote};
use thiserror::Error;

use crate::{
    Addition, AdditionKind, ConsonanceReport, MetricReport, PatchError, TimeSpan, apply_patch,
};

/// Consonance report family addressed by a completion threshold.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MetricFamily {
    /// Set-domain pitch analysis.
    Pitch,
    /// Frequency- and amplitude-domain analysis.
    Acoustic,
    /// Exact-ratio contextual analysis.
    Ratio,
    /// Event-commonality contextual analysis.
    Commonality,
    /// Voice-leading contextual analysis.
    Leading,
}

/// Explicit upper and lower bounds for one named consonance metric.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MetricBounds {
    /// Maximum accepted unnormalized roughness mass.
    pub max_roughness_mass: Option<f64>,
    /// Maximum accepted normalized density.
    pub max_normalized_density: Option<f64>,
    /// Minimum accepted harmonic or continuity context.
    pub min_harmonic_context: Option<f64>,
    /// Maximum accepted harmonic or continuity context.
    pub max_harmonic_context: Option<f64>,
}

/// One named metric threshold over all or part of the score.
#[derive(Clone, Debug, PartialEq)]
pub struct MetricThreshold {
    /// Metric family.
    pub family: MetricFamily,
    /// Exact model name within the family.
    pub model: String,
    /// Optional half-open score span to which the threshold applies.
    pub span: Option<TimeSpan>,
    /// Component bounds checked independently in every intersecting window.
    pub bounds: MetricBounds,
}

/// A pitch range applied globally or to one named voice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchRangeConstraint {
    /// `None` applies the range to every added note.
    pub voice_id: Option<ObjectId>,
    /// Inclusive lower pitch.
    pub lowest: Pitch,
    /// Inclusive upper pitch.
    pub highest: Pitch,
}

/// Source material that completion must leave undisturbed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreservationConstraints {
    /// Source identities that must exist before search.
    ///
    /// Every source identity is retained by the additive patch contract; this
    /// list lets callers make their protected subset explicit and auditable.
    pub required_ids: Vec<ObjectId>,
    /// Spans in which completion may not introduce sounding notes.
    pub protected_spans: Vec<TimeSpan>,
}

/// Explicit style envelope for candidate subsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleConstraints {
    /// Addition kinds allowed in a result.
    pub allowed_kinds: BTreeSet<AdditionKind>,
    /// Minimum number of semantic additions in a finished patch.
    pub min_additions: usize,
    /// Maximum number of semantic additions in a patch.
    pub max_additions: Option<usize>,
    /// Maximum number of introduced notes.
    pub max_added_notes: Option<usize>,
    /// Maximum number of introduced voices.
    pub max_new_voices: Option<usize>,
    /// Maximum introduced notes sounding simultaneously.
    pub max_simultaneous_added_notes: Option<usize>,
    /// Articulations allowed on introduced notes; empty means all.
    pub allowed_articulations: Vec<Articulation>,
}

impl Default for StyleConstraints {
    fn default() -> Self {
        Self {
            allowed_kinds: [
                AdditionKind::Note,
                AdditionKind::Ornament,
                AdditionKind::Chord,
                AdditionKind::Pedal,
                AdditionKind::Doubling,
                AdditionKind::Voice,
            ]
            .into_iter()
            .collect(),
            min_additions: 0,
            max_additions: None,
            max_added_notes: None,
            max_new_voices: None,
            max_simultaneous_added_notes: None,
            allowed_articulations: Vec::new(),
        }
    }
}

/// Complete metric, preservation, range, and style policy for completion.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompletionConstraints {
    /// Required after-report thresholds.
    pub thresholds: Vec<MetricThreshold>,
    /// Immutable-source and protected-span requirements.
    pub preservation: PreservationConstraints,
    /// Global and voice-specific pitch ranges.
    pub ranges: Vec<PitchRangeConstraint>,
    /// Candidate subset style envelope.
    pub style: StyleConstraints,
}

/// Invalid or unsatisfied completion constraint.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ConstraintError {
    /// A policy value is malformed.
    #[error("invalid completion constraint: {0}")]
    Invalid(String),
    /// The patch itself was invalid.
    #[error(transparent)]
    Patch(#[from] PatchError),
}

impl CompletionConstraints {
    pub(crate) fn validate(&self, source: &Staff) -> Result<(), ConstraintError> {
        let source_ids = source.object_ids().into_iter().collect::<BTreeSet<_>>();
        if let Some(id) = self
            .preservation
            .required_ids
            .iter()
            .find(|id| !source_ids.contains(*id))
        {
            return invalid(format!("required source identity {id} does not exist"));
        }
        for range in &self.ranges {
            if range.lowest.semitone() > range.highest.semitone() {
                return invalid("pitch range lower bound exceeds its upper bound");
            }
        }
        if self
            .style
            .max_additions
            .is_some_and(|limit| limit < self.style.min_additions)
        {
            return invalid("maximum additions is below minimum additions");
        }
        for threshold in &self.thresholds {
            if threshold.model.trim().is_empty() {
                return invalid("metric threshold model cannot be empty");
            }
            for value in [
                threshold.bounds.max_roughness_mass,
                threshold.bounds.max_normalized_density,
                threshold.bounds.min_harmonic_context,
                threshold.bounds.max_harmonic_context,
            ]
            .into_iter()
            .flatten()
            {
                if !value.is_finite() {
                    return invalid("metric threshold values must be finite");
                }
            }
            if threshold
                .bounds
                .min_harmonic_context
                .zip(threshold.bounds.max_harmonic_context)
                .is_some_and(|(minimum, maximum)| minimum > maximum)
            {
                return invalid("harmonic-context minimum exceeds its maximum");
            }
        }
        Ok(())
    }

    pub(crate) fn accepts_partial(
        &self,
        source: &Staff,
        additions: &[Addition],
    ) -> Result<bool, ConstraintError> {
        if self
            .style
            .max_additions
            .is_some_and(|limit| additions.len() > limit)
            || additions
                .iter()
                .any(|addition| !self.style.allowed_kinds.contains(&addition.kind()))
        {
            return Ok(false);
        }
        let notes = addition_notes(additions);
        if self
            .style
            .max_added_notes
            .is_some_and(|limit| notes.len() > limit)
            || self
                .style
                .max_new_voices
                .is_some_and(|limit| new_voice_count(additions) > limit)
            || self
                .style
                .max_simultaneous_added_notes
                .is_some_and(|limit| maximum_simultaneous(&notes) > limit)
        {
            return Ok(false);
        }
        if !self.style.allowed_articulations.is_empty()
            && notes.iter().any(|note| {
                !self
                    .style
                    .allowed_articulations
                    .contains(&note.note.articulation)
            })
        {
            return Ok(false);
        }
        if notes.iter().any(|note| !self.note_is_in_range(note)) {
            return Ok(false);
        }
        if notes.iter().any(|note| {
            self.preservation
                .protected_spans
                .iter()
                .any(|span| overlaps_note(span, note))
        }) {
            return Ok(false);
        }
        let patch = crate::ConsonancePatch::new(source, additions.to_vec())?;
        apply_patch(source, &patch)?;
        Ok(true)
    }

    pub(crate) fn accepts_complete(
        &self,
        source: &Staff,
        additions: &[Addition],
        report: &ConsonanceReport,
    ) -> Result<bool, ConstraintError> {
        if additions.len() < self.style.min_additions || !self.accepts_partial(source, additions)? {
            return Ok(false);
        }
        Ok(self
            .thresholds
            .iter()
            .all(|threshold| threshold_accepts(threshold, report)))
    }

    fn note_is_in_range(&self, note: &StaffNote) -> bool {
        self.ranges
            .iter()
            .filter(|range| {
                range
                    .voice_id
                    .as_ref()
                    .is_none_or(|voice| voice == &note.voice_id)
            })
            .all(|range| {
                (range.lowest.semitone()..=range.highest.semitone())
                    .contains(&note.note.pitch.semitone())
            })
    }
}

fn threshold_accepts(threshold: &MetricThreshold, report: &ConsonanceReport) -> bool {
    let mut matched = false;
    for window in &report.windows {
        if threshold
            .span
            .as_ref()
            .is_some_and(|span| !spans_overlap(span, &window.window.span))
        {
            continue;
        }
        let metric = match threshold.family {
            MetricFamily::Pitch => window
                .pitch
                .iter()
                .find(|metric| metric.model == threshold.model),
            MetricFamily::Acoustic => window
                .acoustic
                .iter()
                .find(|metric| metric.model == threshold.model),
            MetricFamily::Ratio => named_metric(&window.ratio, &threshold.model),
            MetricFamily::Commonality => named_metric(&window.commonality, &threshold.model),
            MetricFamily::Leading => named_metric(&window.leading, &threshold.model),
        };
        let Some(metric) = metric else {
            return false;
        };
        matched = true;
        if !bounds_accept(&threshold.bounds, metric) {
            return false;
        }
    }
    matched
}

fn named_metric<'a>(metric: &'a MetricReport, name: &str) -> Option<&'a MetricReport> {
    (metric.model == name).then_some(metric)
}

fn bounds_accept(bounds: &MetricBounds, metric: &MetricReport) -> bool {
    bounds
        .max_roughness_mass
        .is_none_or(|limit| metric.roughness_mass <= limit)
        && bounds
            .max_normalized_density
            .is_none_or(|limit| metric.normalized_density <= limit)
        && bounds
            .min_harmonic_context
            .is_none_or(|limit| metric.harmonic_context >= limit)
        && bounds
            .max_harmonic_context
            .is_none_or(|limit| metric.harmonic_context <= limit)
}

fn addition_notes(additions: &[Addition]) -> Vec<&StaffNote> {
    additions
        .iter()
        .flat_map(|addition| addition.notes())
        .collect()
}

fn new_voice_count(additions: &[Addition]) -> usize {
    additions
        .iter()
        .filter(|addition| matches!(addition, Addition::Voice(_)))
        .count()
}

fn maximum_simultaneous(notes: &[&StaffNote]) -> usize {
    let mut boundaries = notes
        .iter()
        .flat_map(|note| [note.onset, note.end()])
        .collect::<Vec<_>>();
    boundaries.sort();
    boundaries.dedup();
    boundaries
        .into_iter()
        .map(|at| {
            notes
                .iter()
                .filter(|note| note.onset <= at && at < note.end())
                .count()
        })
        .max()
        .unwrap_or(0)
}

fn overlaps_note(span: &TimeSpan, note: &StaffNote) -> bool {
    span.start < note.end() && note.onset < span.end
}

fn spans_overlap(left: &TimeSpan, right: &TimeSpan) -> bool {
    left.start < right.end && right.start < left.end
}

fn invalid<T>(reason: impl Into<String>) -> Result<T, ConstraintError> {
    Err(ConstraintError::Invalid(reason.into()))
}

pub(crate) fn changed_spans(report: &ConsonanceReport, additions: &[Addition]) -> Vec<TimeSpan> {
    let event_ids = additions
        .iter()
        .flat_map(|addition| addition.notes())
        .map(|note| note.event_id.clone())
        .collect::<BTreeSet<_>>();
    report
        .windows
        .iter()
        .filter(|window| {
            window
                .window
                .notes
                .iter()
                .any(|note| event_ids.contains(&note.event_id))
        })
        .map(|window| window.window.span.clone())
        .collect()
}
