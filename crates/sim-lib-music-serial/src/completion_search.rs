use std::collections::BTreeSet;

use sim_lib_discrete_search::{SearchProblem, SearchStep};
use sim_lib_music_core::{ObjectId, Staff, StaffNote, Time};

use crate::additive::{AdditiveStaffPatch, apply_additive_staff_patch};

use super::{CompletionCandidate, CompletionError, CompletionRequest, PitchRangeConstraint};

#[derive(Clone, Debug)]
pub(super) struct GenericCompletionProblem<'a> {
    pub(super) source: &'a Staff,
    pub(super) request: &'a CompletionRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct GenericCompletionState {
    cursor: usize,
    selected: Vec<usize>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum GenericCompletionChoice {
    Skip,
    Include,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct GenericCompletionOutput {
    pub(super) selected: Vec<usize>,
    pub(super) patch: AdditiveStaffPatch,
    pub(super) completed: Staff,
    pub(super) added_ids: Vec<ObjectId>,
}

impl SearchProblem for GenericCompletionProblem<'_> {
    type State = GenericCompletionState;
    type Choice = GenericCompletionChoice;
    type Output = GenericCompletionOutput;

    fn initial_state(&self) -> Self::State {
        GenericCompletionState {
            cursor: 0,
            selected: Vec::new(),
        }
    }

    fn expand(&self, state: &Self::State, out: &mut Vec<Self::Choice>) {
        if state.cursor < self.request.candidates.len() {
            out.extend([
                GenericCompletionChoice::Skip,
                GenericCompletionChoice::Include,
            ]);
        }
    }

    fn apply(&self, state: &Self::State, choice: &Self::Choice) -> SearchStep<Self::State> {
        let mut next = state.clone();
        if *choice == GenericCompletionChoice::Include {
            next.selected.push(state.cursor);
        }
        if self
            .request
            .max_candidates
            .is_some_and(|limit| next.selected.len() > limit)
        {
            return SearchStep::pruned("candidate-count bound exceeded");
        }
        next.cursor += 1;
        let candidates = next
            .selected
            .iter()
            .map(|index| self.request.candidates[*index].clone())
            .collect::<Vec<_>>();
        if compile_patch(self.source, &candidates, &self.request.pitch_ranges).is_err() {
            return SearchStep::pruned("candidate prefix failed additive validation");
        }
        SearchStep::Continue(next)
    }

    fn finish(&self, state: &Self::State) -> Option<Self::Output> {
        if state.cursor != self.request.candidates.len()
            || state.selected.len() < self.request.min_candidates
            || self
                .request
                .max_candidates
                .is_some_and(|limit| state.selected.len() > limit)
        {
            return None;
        }
        let candidates = state
            .selected
            .iter()
            .map(|index| self.request.candidates[*index].clone())
            .collect::<Vec<_>>();
        let patch = compile_patch(self.source, &candidates, &self.request.pitch_ranges).ok()?;
        let completed = apply_additive_staff_patch(self.source, &patch).ok()?;
        let added_ids = candidates
            .iter()
            .flat_map(|candidate| candidate.notes())
            .flat_map(|note| {
                [
                    note.voice_id.clone(),
                    note.note_id.clone(),
                    note.event_id.clone(),
                ]
            })
            .chain(candidates.iter().filter_map(|candidate| match candidate {
                CompletionCandidate::Voice(value) => Some(value.voice.id.clone()),
                _ => None,
            }))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        Some(GenericCompletionOutput {
            selected: state.selected.clone(),
            patch,
            completed,
            added_ids,
        })
    }

    fn score_state(&self, state: &Self::State) -> i64 {
        i64::try_from(state.selected.len()).unwrap_or(i64::MAX)
    }

    fn bound(&self, state: &Self::State) -> Option<i64> {
        Some(i64::try_from(state.selected.len()).unwrap_or(i64::MAX))
    }

    fn output_score(&self, output: &Self::Output) -> Option<i64> {
        Some(i64::try_from(output.selected.len()).unwrap_or(i64::MAX))
    }
}

pub(super) fn compile_patch(
    source: &Staff,
    candidates: &[CompletionCandidate],
    pitch_ranges: &[PitchRangeConstraint],
) -> Result<AdditiveStaffPatch, CompletionError> {
    validate_candidates(source, candidates, pitch_ranges)?;
    let mut patch = AdditiveStaffPatch::default();
    for candidate in candidates {
        candidate.compile_into(&mut patch);
    }
    apply_additive_staff_patch(source, &patch).map_err(CompletionError::InvalidCandidate)?;
    Ok(patch)
}

fn validate_candidates(
    source: &Staff,
    candidates: &[CompletionCandidate],
    pitch_ranges: &[PitchRangeConstraint],
) -> Result<(), CompletionError> {
    let duration = source.duration();
    let source_events = source
        .notes()
        .map(|note| (note.event_id.clone(), note))
        .collect::<std::collections::BTreeMap<_, _>>();
    for candidate in candidates {
        validate_candidate_semantics(source, candidate, &source_events)?;
        for note in candidate.notes() {
            if note.note.duration <= Time::from_integer(0)
                || note.onset < Time::from_integer(0)
                || note.end() > duration
            {
                return super::invalid(
                    "added notes must have positive spans inside the source duration",
                );
            }
            if !note_in_ranges(note, pitch_ranges) {
                return super::invalid(format!(
                    "added note {} falls outside the declared pitch ranges",
                    note.event_id
                ));
            }
        }
    }
    Ok(())
}

fn validate_candidate_semantics(
    source: &Staff,
    candidate: &CompletionCandidate,
    _source_events: &std::collections::BTreeMap<ObjectId, &StaffNote>,
) -> Result<(), CompletionError> {
    match candidate {
        CompletionCandidate::Note(_) => {}
        CompletionCandidate::Ornament(value) => {
            if value.notes.is_empty() {
                return super::invalid("an ornament needs at least one note");
            }
        }
        CompletionCandidate::Chord(value) => {
            let Some(first) = value.notes.first() else {
                return super::invalid("a chord addition must contain notes");
            };
            if value.notes.len() < 2
                || value
                    .notes
                    .iter()
                    .any(|note| note.onset != first.onset || note.end() != first.end())
            {
                return super::invalid(
                    "a chord addition needs at least two notes with one exact span",
                );
            }
        }
        CompletionCandidate::Pedal(_) => {}
        CompletionCandidate::Doubling(value) => {
            if value.source_event_id.as_str().trim().is_empty() {
                return super::invalid("a doubling must name a non-empty source event");
            }
        }
        CompletionCandidate::Voice(value) => {
            if value.voice.duration != source.duration() {
                return super::invalid("added voices must retain the source staff duration");
            }
        }
    }
    if let CompletionCandidate::Voice(value) = candidate {
        for note in &value.voice.notes {
            if note.voice_id != value.voice.id {
                return super::invalid("voice additions must keep note voice ids aligned");
            }
        }
    }
    let _ = source;
    Ok(())
}

fn note_in_ranges(note: &StaffNote, ranges: &[PitchRangeConstraint]) -> bool {
    ranges
        .iter()
        .filter(|range| {
            range
                .voice_id
                .as_ref()
                .is_none_or(|voice_id| voice_id == &note.voice_id)
        })
        .all(|range| {
            (range.lowest.semitone()..=range.highest.semitone())
                .contains(&note.note.pitch.semitone())
        })
}
