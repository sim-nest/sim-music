use std::cmp::Ordering;

use sim_lib_discrete_search::{
    SearchControl, SearchInterrupt, SearchProblem, SearchReceipt, SearchStep, solve,
};
use sim_lib_music_core::{ObjectId, Staff};
use thiserror::Error;

use crate::constraints::changed_spans;
use crate::patch::addition_ids;
use crate::{
    Addition, CompletionConstraints, ConsonanceError, ConsonancePatch, ConsonancePolicy,
    ConsonanceReport, ConstraintError, PatchError, TimeSpan, apply_patch, evaluate_staff,
};

/// Candidate additions and explicit acceptance policy for one completion run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompletionRequest {
    /// Deterministically ordered typed candidates considered by the search.
    pub candidates: Vec<Addition>,
    /// Metric, preservation, range, and style constraints.
    pub constraints: CompletionConstraints,
}

/// Provenance of the selected additive completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionProvenance {
    /// Candidate indexes selected in caller-supplied order.
    pub selected_candidates: Vec<usize>,
    /// Every source identity proven present and byte-for-byte unchanged.
    pub preserved_ids: Vec<ObjectId>,
    /// Every voice, note, and event identity introduced by the patch.
    pub added_ids: Vec<ObjectId>,
    /// Stable facts about acceptance and inverse verification.
    pub facts: Vec<String>,
}

/// A reversible patch paired with analysis and bounded-search evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct CompletionResult {
    /// Strictly additive, content-bound patch.
    pub patch: ConsonancePatch,
    /// Consonance analysis before completion.
    pub before: ConsonanceReport,
    /// Consonance analysis after completion.
    pub after: ConsonanceReport,
    /// Exact after-report windows containing introduced events.
    pub changed_windows: Vec<TimeSpan>,
    /// Source, selection, identity, and inverse evidence.
    pub provenance: CompletionProvenance,
    /// Generic bounded-search termination receipt.
    pub search: SearchReceipt,
}

/// Failure to run or satisfy additive completion.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum CompletionError {
    /// Initial consonance evaluation failed.
    #[error(transparent)]
    Consonance(#[from] ConsonanceError),
    /// Patch construction or inversion failed.
    #[error(transparent)]
    Patch(#[from] PatchError),
    /// The request's explicit constraints were malformed.
    #[error(transparent)]
    Constraint(#[from] ConstraintError),
    /// Search terminated without a feasible completion.
    #[error("bounded completion produced no feasible patch")]
    NoCompletion {
        /// Original report retained for diagnosis.
        before: Box<ConsonanceReport>,
        /// Honest complete, partial, cancelled, or infeasible receipt.
        search: Box<SearchReceipt>,
    },
}

/// Searches typed candidate subsets under explicit bounds and constraints.
///
/// The input staff is borrowed and never mutated. A partial search may return a
/// result only when it actually emitted a feasible patch; its `SearchReceipt`
/// remains `Partial` or `Cancelled` rather than being upgraded to complete.
pub fn complete_staff(
    source: &Staff,
    policy: &ConsonancePolicy,
    request: &CompletionRequest,
    control: SearchControl,
    interrupt: &dyn SearchInterrupt,
) -> Result<CompletionResult, CompletionError> {
    request.constraints.validate(source)?;
    for candidate in &request.candidates {
        ConsonancePatch::new(source, vec![candidate.clone()])?;
    }
    let before = evaluate_staff(source, policy)?;
    let problem = CompletionProblem {
        source,
        policy,
        request,
    };
    let run = solve(&problem, control, interrupt);
    let receipt = run.receipt;
    let Some(candidate) = run.outputs.into_iter().min_by(compare_outputs) else {
        return Err(CompletionError::NoCompletion {
            before: Box::new(before),
            search: Box::new(receipt),
        });
    };
    let restored = crate::remove_patch(&candidate.completed, &candidate.patch)?;
    if restored != *source {
        return Err(PatchError::InvalidInverse(
            "remove(apply(source, patch), patch) changed source identities or values".to_owned(),
        )
        .into());
    }
    let preserved_ids = source.object_ids();
    let added_ids = addition_ids(&candidate.patch.additions);
    let changed_windows = changed_spans(&candidate.after, &candidate.patch.additions);
    Ok(CompletionResult {
        patch: candidate.patch,
        before,
        after: candidate.after,
        changed_windows,
        provenance: CompletionProvenance {
            selected_candidates: candidate.selected,
            preserved_ids,
            added_ids,
            facts: vec![
                "source-material=immutable".to_owned(),
                "patch-operation=additions-only".to_owned(),
                "inverse=remove(apply(source,patch),patch)==source".to_owned(),
                "metric-thresholds=checked-per-intersecting-window".to_owned(),
            ],
        },
        search: receipt,
    })
}

struct CompletionProblem<'a> {
    source: &'a Staff,
    policy: &'a ConsonancePolicy,
    request: &'a CompletionRequest,
}

#[derive(Clone, Debug)]
struct CompletionState {
    cursor: usize,
    selected: Vec<usize>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum CompletionChoice {
    Skip,
    Include,
}

#[derive(Clone, Debug)]
struct CompletionOutput {
    selected: Vec<usize>,
    patch: ConsonancePatch,
    completed: Staff,
    after: ConsonanceReport,
}

impl SearchProblem for CompletionProblem<'_> {
    type State = CompletionState;
    type Choice = CompletionChoice;
    type Output = CompletionOutput;

    fn initial_state(&self) -> Self::State {
        CompletionState {
            cursor: 0,
            selected: Vec::new(),
        }
    }

    fn expand(&self, state: &Self::State, out: &mut Vec<Self::Choice>) {
        if state.cursor < self.request.candidates.len() {
            out.extend([CompletionChoice::Skip, CompletionChoice::Include]);
        }
    }

    fn apply(&self, state: &Self::State, choice: &Self::Choice) -> SearchStep<Self::State> {
        let mut next = state.clone();
        if *choice == CompletionChoice::Include {
            next.selected.push(state.cursor);
        }
        next.cursor += 1;
        let additions = self.selected_additions(&next.selected);
        match self
            .request
            .constraints
            .accepts_partial(self.source, &additions)
        {
            Ok(true) => SearchStep::Continue(next),
            Ok(false) => SearchStep::pruned("completion constraints rejected candidate prefix"),
            Err(error) => SearchStep::pruned(error.to_string()),
        }
    }

    fn finish(&self, state: &Self::State) -> Option<Self::Output> {
        if state.cursor != self.request.candidates.len() {
            return None;
        }
        let additions = self.selected_additions(&state.selected);
        let patch = ConsonancePatch::new(self.source, additions.clone()).ok()?;
        let completed = apply_patch(self.source, &patch).ok()?;
        let after = evaluate_staff(&completed, self.policy).ok()?;
        self.request
            .constraints
            .accepts_complete(self.source, &additions, &after)
            .ok()?
            .then(|| CompletionOutput {
                selected: state.selected.clone(),
                patch,
                completed,
                after,
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

impl CompletionProblem<'_> {
    fn selected_additions(&self, selected: &[usize]) -> Vec<Addition> {
        selected
            .iter()
            .map(|index| self.request.candidates[*index].clone())
            .collect()
    }
}

fn compare_outputs(left: &CompletionOutput, right: &CompletionOutput) -> Ordering {
    left.selected
        .len()
        .cmp(&right.selected.len())
        .then_with(|| left.selected.cmp(&right.selected))
}
