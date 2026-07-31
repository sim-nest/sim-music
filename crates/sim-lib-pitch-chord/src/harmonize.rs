//! Bounded harmonization over declarative palettes and rules.

use std::{cell::RefCell, cmp::Ordering, collections::BTreeMap};

use sim_lib_discrete_search::{
    SearchControl, SearchInterrupt, SearchProblem, SearchStatus, SearchStep, solve,
};

use crate::{
    ChordTemplate, HARMONY_SCORE_SCALE, HarmonizationReceipt, HarmonizationRequest,
    HarmonizationResult, HarmonizationRun, HarmonizationStrategy, HarmonyError, HarmonyEvaluation,
    HarmonyEvaluationContext, HarmonyHeuristic, HarmonyMetricResolver, HarmonyRejection,
    evaluate_harmony,
};

/// Plans chord progressions with one declarative problem and explicit strategy.
pub fn plan_harmony(
    request: &HarmonizationRequest,
    strategy: HarmonizationStrategy,
    control: SearchControl,
    interrupt: &dyn SearchInterrupt,
    resolver: &dyn HarmonyMetricResolver,
) -> Result<HarmonizationRun, HarmonyError> {
    request.validate()?;
    if let HarmonizationStrategy::Beam { width, heuristic } = &strategy {
        if *width == 0 {
            return Err(HarmonyError::InvalidField {
                field: "harmonization.beam-width",
                reason: "beam width must be positive".to_owned(),
            });
        }
        heuristic.validate()?;
    }
    if strategy == HarmonizationStrategy::LayeredDp {
        return crate::harmonize_layered::plan_layered(
            request, strategy, control, interrupt, resolver,
        );
    }

    let context = PlannerContext::new(request, resolver);
    let factored = !matches!(strategy, HarmonizationStrategy::RecursiveExhaustive);
    let heuristic = match &strategy {
        HarmonizationStrategy::Beam { heuristic, .. } => Some(heuristic.clone()),
        _ => None,
    };
    let problem = HarmonizationProblem {
        context: &context,
        factored,
        heuristic,
    };
    let mut search_control = control;
    search_control.order = match strategy {
        HarmonizationStrategy::Beam { width, .. } => {
            sim_lib_discrete_search::SearchOrder::Beam { width }
        }
        _ => sim_lib_discrete_search::SearchOrder::DepthFirst,
    };
    let search = solve(&problem, search_control, interrupt);
    if let Some(error) = context.error.borrow_mut().take() {
        return Err(error);
    }
    let mut results = search.outputs;
    results.sort_by(compare_results);
    let stats = context.stats.borrow();
    let optimal = search.receipt.status == SearchStatus::Complete
        && !matches!(strategy, HarmonizationStrategy::Beam { .. });
    let receipt = HarmonizationReceipt {
        strategy,
        status: search.receipt.status.clone(),
        reason: search.receipt.reason.clone(),
        work_used: search.receipt.work_used,
        evaluated_candidates: stats.evaluated,
        result_count: results.len(),
        optimal,
        search: Some(search.receipt),
        layered: None,
        optimality: None,
        rejections: stats.rejections(),
    };
    Ok(HarmonizationRun { results, receipt })
}

pub(crate) struct PlannerContext<'a> {
    pub(crate) request: &'a HarmonizationRequest,
    resolver: &'a dyn HarmonyMetricResolver,
    pub(crate) stats: RefCell<PlannerStats>,
    pub(crate) error: RefCell<Option<HarmonyError>>,
}

impl<'a> PlannerContext<'a> {
    pub(crate) fn new(
        request: &'a HarmonizationRequest,
        resolver: &'a dyn HarmonyMetricResolver,
    ) -> Self {
        Self {
            request,
            resolver,
            stats: RefCell::new(PlannerStats::default()),
            error: RefCell::new(None),
        }
    }

    pub(crate) fn prepare(
        &self,
        state: &PlannerState,
        index: usize,
    ) -> Result<Option<PreparedStep>, HarmonyError> {
        let mut progression = state.progression.clone();
        progression.push(self.request.palette.entries[index].clone());
        let evaluation = evaluate_harmony(
            &self.request.rules,
            HarmonyEvaluationContext::progression(&self.request.melody, &progression),
            self.resolver,
        )?;
        self.stats.borrow_mut().evaluated += 1;
        if !evaluation.legal {
            self.stats.borrow_mut().reject(&evaluation);
            return Ok(None);
        }
        let step_score = quantize_score(evaluation.score)?;
        let total_score = state.score_micros.checked_add(step_score).ok_or_else(|| {
            HarmonyError::InvalidField {
                field: "harmonization.score",
                reason: "accumulated fixed-precision score overflowed".to_owned(),
            }
        })?;
        Ok(Some(PreparedStep {
            progression,
            evaluation,
            total_score,
        }))
    }
}

#[derive(Default)]
pub(crate) struct PlannerStats {
    pub(crate) evaluated: u64,
    rejected: BTreeMap<(String, Vec<String>), u64>,
}

impl PlannerStats {
    fn reject(&mut self, evaluation: &HarmonyEvaluation) {
        for evidence in evaluation.hard.iter().filter(|evidence| !evidence.passed) {
            *self
                .rejected
                .entry((evidence.rule_id.clone(), evidence.facts.clone()))
                .or_default() += 1;
        }
    }

    pub(crate) fn rejections(&self) -> Vec<HarmonyRejection> {
        self.rejected
            .iter()
            .map(|((rule_id, facts), count)| HarmonyRejection {
                rule_id: rule_id.clone(),
                facts: facts.clone(),
                count: *count,
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PlannerState {
    pub(crate) indices: Vec<usize>,
    pub(crate) progression: Vec<ChordTemplate>,
    pub(crate) evaluations: Vec<HarmonyEvaluation>,
    pub(crate) score_micros: i64,
}

impl PlannerState {
    pub(crate) fn initial() -> Self {
        Self {
            indices: Vec::new(),
            progression: Vec::new(),
            evaluations: Vec::new(),
            score_micros: 0,
        }
    }

    pub(crate) fn append(&self, index: usize, prepared: PreparedStep) -> Self {
        let mut indices = self.indices.clone();
        indices.push(index);
        let mut evaluations = self.evaluations.clone();
        evaluations.push(prepared.evaluation);
        Self {
            indices,
            progression: prepared.progression,
            evaluations,
            score_micros: prepared.total_score,
        }
    }

    pub(crate) fn result(&self, complete: bool) -> HarmonizationResult {
        HarmonizationResult {
            progression: self.progression.clone(),
            palette_indices: self.indices.clone(),
            evaluations: self.evaluations.clone(),
            score_micros: self.score_micros,
            complete,
        }
    }
}

#[derive(Clone)]
pub(crate) struct PreparedStep {
    progression: Vec<ChordTemplate>,
    evaluation: HarmonyEvaluation,
    total_score: i64,
}

struct HarmonizationProblem<'a> {
    context: &'a PlannerContext<'a>,
    factored: bool,
    heuristic: Option<HarmonyHeuristic>,
}

#[derive(Clone)]
struct HarmonizationChoice {
    index: usize,
    prepared: Option<PreparedStep>,
}

impl PartialEq for HarmonizationChoice {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl Eq for HarmonizationChoice {}

impl PartialOrd for HarmonizationChoice {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HarmonizationChoice {
    fn cmp(&self, other: &Self) -> Ordering {
        self.index.cmp(&other.index)
    }
}

impl SearchProblem for HarmonizationProblem<'_> {
    type State = PlannerState;
    type Choice = HarmonizationChoice;
    type Output = HarmonizationResult;

    fn initial_state(&self) -> Self::State {
        PlannerState::initial()
    }

    fn expand(&self, state: &Self::State, out: &mut Vec<Self::Choice>) {
        if state.indices.len() >= self.context.request.melody.len() {
            return;
        }
        for index in 0..self.context.request.palette.entries.len() {
            if self.factored {
                match self.context.prepare(state, index) {
                    Ok(Some(prepared)) => out.push(HarmonizationChoice {
                        index,
                        prepared: Some(prepared),
                    }),
                    Ok(None) => {}
                    Err(error) => {
                        *self.context.error.borrow_mut() = Some(error);
                        break;
                    }
                }
            } else {
                out.push(HarmonizationChoice {
                    index,
                    prepared: None,
                });
            }
        }
    }

    fn apply(&self, state: &Self::State, choice: &Self::Choice) -> SearchStep<Self::State> {
        let prepared = match &choice.prepared {
            Some(prepared) => prepared.clone(),
            None => match self.context.prepare(state, choice.index) {
                Ok(Some(prepared)) => prepared,
                Ok(None) => return SearchStep::pruned("declarative harmony rule rejected choice"),
                Err(error) => {
                    *self.context.error.borrow_mut() = Some(error);
                    return SearchStep::infeasible("harmony metric evaluation failed");
                }
            },
        };
        SearchStep::Continue(state.append(choice.index, prepared))
    }

    fn finish(&self, state: &Self::State) -> Option<Self::Output> {
        (state.indices.len() == self.context.request.melody.len()).then(|| state.result(true))
    }

    fn score_state(&self, state: &Self::State) -> i64 {
        state.score_micros
    }

    fn estimate_remaining(&self, state: &Self::State) -> i64 {
        let remaining = self
            .context
            .request
            .melody
            .len()
            .saturating_sub(state.indices.len());
        self.heuristic.as_ref().map_or(0, |heuristic| {
            heuristic
                .lower_bound_per_remaining_micros
                .saturating_mul(i64::try_from(remaining).unwrap_or(i64::MAX))
        })
    }

    fn bound(&self, state: &Self::State) -> Option<i64> {
        self.heuristic
            .as_ref()
            .filter(|heuristic| heuristic.admissible)
            .map(|_| {
                state
                    .score_micros
                    .saturating_add(self.estimate_remaining(state))
            })
    }

    fn output_score(&self, output: &Self::Output) -> Option<i64> {
        Some(output.score_micros)
    }
}

pub(crate) fn compare_results(left: &HarmonizationResult, right: &HarmonizationResult) -> Ordering {
    left.score_micros
        .cmp(&right.score_micros)
        .then_with(|| left.palette_indices.cmp(&right.palette_indices))
}

fn quantize_score(score: f64) -> Result<i64, HarmonyError> {
    let scaled = score * HARMONY_SCORE_SCALE as f64;
    if !scaled.is_finite() || scaled < i64::MIN as f64 || scaled > i64::MAX as f64 {
        return Err(HarmonyError::InvalidField {
            field: "harmonization.score",
            reason: format!("score {score} cannot be represented at fixed precision"),
        });
    }
    Ok(scaled.round() as i64)
}
