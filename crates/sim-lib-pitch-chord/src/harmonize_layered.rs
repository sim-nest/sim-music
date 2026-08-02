//! Exact layered harmonization adapter.

use std::time::Instant;

use sim_lib_discrete_graph::{
    AlgorithmControl, AlgorithmInterrupt, GraphError, layered_shortest_path_with_control,
};
use sim_lib_discrete_search::{SearchControl, SearchInterrupt, SearchStatus};

use crate::harmonize::{PlannerContext, PlannerState, compare_results};
use crate::{
    HarmonizationReceipt, HarmonizationRequest, HarmonizationResult, HarmonizationRun,
    HarmonizationStrategy, HarmonyError, HarmonyMetricResolver, HarmonyOptimalityEvidence,
};

#[derive(Clone, Debug)]
enum LayerNode {
    Root,
    Prefix {
        state: PlannerState,
        step_score: i64,
    },
}

impl LayerNode {
    fn state(&self) -> Option<&PlannerState> {
        match self {
            Self::Root => None,
            Self::Prefix { state, .. } => Some(state),
        }
    }
}

pub(crate) fn plan_layered(
    request: &HarmonizationRequest,
    strategy: HarmonizationStrategy,
    control: SearchControl,
    interrupt: &dyn SearchInterrupt,
    resolver: &dyn HarmonyMetricResolver,
) -> Result<HarmonizationRun, HarmonyError> {
    if control.max_results == Some(0) {
        return Ok(stopped(
            strategy,
            SearchStatus::Partial,
            "result bound reached",
            0,
            0,
            Vec::new(),
            Vec::new(),
        ));
    }

    let started = Instant::now();
    let context = PlannerContext::new(request, resolver);
    let mut work_used = 0u64;
    let mut layers = vec![vec![LayerNode::Root]];
    let mut previous = vec![PlannerState::initial()];

    for _position in 0..request.melody.len() {
        let mut current = Vec::new();
        for state in &previous {
            if let Some(reason) = charge(
                &control,
                interrupt,
                started,
                &mut work_used,
                control.costs.expand,
            ) {
                return Ok(partial(
                    strategy,
                    reason,
                    work_used,
                    &previous,
                    &context,
                    control.max_results,
                ));
            }
            for index in 0..request.palette.entries.len() {
                let candidate_cost = control
                    .costs
                    .score
                    .checked_add(control.costs.propagate)
                    .ok_or_else(|| HarmonyError::InvalidField {
                        field: "harmonization.control.costs",
                        reason: "score plus propagation work overflowed".to_owned(),
                    })?;
                if let Some(reason) =
                    charge(&control, interrupt, started, &mut work_used, candidate_cost)
                {
                    let candidates = if current.is_empty() {
                        &previous
                    } else {
                        &current
                    };
                    return Ok(partial(
                        strategy,
                        reason,
                        work_used,
                        candidates,
                        &context,
                        control.max_results,
                    ));
                }
                if let Some(prepared) = context.prepare(state, index)? {
                    current.push(state.append(index, prepared.clone()));
                }
            }
        }

        if current.is_empty() {
            return Ok(stopped(
                strategy,
                SearchStatus::Infeasible,
                "no legal prefix reaches the next melody layer",
                work_used,
                context.stats.borrow().evaluated,
                Vec::new(),
                context.stats.borrow().rejections(),
            ));
        }
        if let Some(limit) = control.max_frontier
            && current.len() > limit
        {
            return Ok(partial(
                strategy,
                "frontier bound reached".to_owned(),
                work_used,
                &current,
                &context,
                control.max_results,
            ));
        }
        let retained = layers
            .iter()
            .map(Vec::len)
            .sum::<usize>()
            .saturating_add(current.len());
        if let Some(limit) = control.max_memory_nodes
            && retained > limit
        {
            return Ok(partial(
                strategy,
                "memory node bound reached".to_owned(),
                work_used,
                &current,
                &context,
                control.max_results,
            ));
        }
        layers.push(
            current
                .iter()
                .map(|state| {
                    let step_score = state
                        .evaluations
                        .last()
                        .map(|evaluation| evaluation.score)
                        .unwrap_or_default();
                    Ok(LayerNode::Prefix {
                        state: state.clone(),
                        step_score: quantized_step(step_score)?,
                    })
                })
                .collect::<Result<Vec<_>, HarmonyError>>()?,
        );
        previous = current;
    }

    let graph_control = graph_control(&control, work_used, started)?;
    let adapter = InterruptAdapter(interrupt);
    let path =
        match layered_shortest_path_with_control(&layers, transition, &graph_control, &adapter) {
            Ok(path) => path,
            Err(GraphError::Disconnected) => {
                return Ok(stopped(
                    strategy,
                    SearchStatus::Infeasible,
                    "no legal progression connects every melody layer",
                    work_used,
                    context.stats.borrow().evaluated,
                    Vec::new(),
                    context.stats.borrow().rejections(),
                ));
            }
            Err(GraphError::ControlStopped(reason)) => {
                let status = if reason.contains("interrupt") {
                    SearchStatus::Cancelled
                } else {
                    SearchStatus::Partial
                };
                let mut run = partial(
                    strategy,
                    reason,
                    work_used,
                    &previous,
                    &context,
                    control.max_results,
                );
                run.receipt.status = status;
                return Ok(run);
            }
            Err(error) => {
                return Err(HarmonyError::InvalidField {
                    field: "harmonization.layered-dp",
                    reason: error.to_string(),
                });
            }
        };
    work_used = work_used
        .checked_add(path.receipt.work_used)
        .ok_or_else(|| HarmonyError::InvalidField {
            field: "harmonization.work",
            reason: "combined layered work overflowed".to_owned(),
        })?;
    let selected = path
        .states
        .last()
        .and_then(LayerNode::state)
        .ok_or_else(|| HarmonyError::InvalidField {
            field: "harmonization.layered-dp",
            reason: "certified path ended without a progression".to_owned(),
        })?;
    if let Some(reason) = charge(
        &control,
        interrupt,
        started,
        &mut work_used,
        control.costs.emit,
    ) {
        return Ok(HarmonizationRun {
            results: vec![selected.result(true)],
            receipt: HarmonizationReceipt {
                strategy,
                status: SearchStatus::Partial,
                reason: Some(reason),
                work_used,
                evaluated_candidates: context.stats.borrow().evaluated,
                result_count: 1,
                optimal: true,
                search: None,
                layered: Some(path.receipt.clone()),
                optimality: Some(HarmonyOptimalityEvidence {
                    total_score_micros: path.total_cost,
                    layer_indices: path.indices,
                    certificate: path.certificate,
                }),
                rejections: context.stats.borrow().rejections(),
            },
        });
    }
    let result = selected.result(true);
    Ok(HarmonizationRun {
        results: vec![result],
        receipt: HarmonizationReceipt {
            strategy,
            status: SearchStatus::Complete,
            reason: None,
            work_used,
            evaluated_candidates: context.stats.borrow().evaluated,
            result_count: 1,
            optimal: true,
            search: None,
            layered: Some(path.receipt.clone()),
            optimality: Some(HarmonyOptimalityEvidence {
                total_score_micros: path.total_cost,
                layer_indices: path.indices,
                certificate: path.certificate,
            }),
            rejections: context.stats.borrow().rejections(),
        },
    })
}

fn transition(left: &LayerNode, right: &LayerNode) -> Option<i64> {
    match (left, right) {
        (LayerNode::Root, LayerNode::Prefix { state, step_score }) if state.indices.len() == 1 => {
            Some(*step_score)
        }
        (
            LayerNode::Prefix {
                state: left_state, ..
            },
            LayerNode::Prefix {
                state: right_state,
                step_score,
            },
        ) if right_state.indices.len() == left_state.indices.len() + 1
            && right_state.indices.starts_with(&left_state.indices) =>
        {
            Some(*step_score)
        }
        _ => None,
    }
}

fn graph_control(
    control: &SearchControl,
    work_used: u64,
    started: Instant,
) -> Result<AlgorithmControl, HarmonyError> {
    let mut graph = AlgorithmControl::default();
    if let Some(max_work) = control.max_work {
        graph.max_work = Some(max_work.saturating_sub(work_used));
    }
    graph.max_memory_cells = control.max_memory_nodes;
    if let Some(max_time) = control.max_time {
        graph.max_time = Some(max_time.saturating_sub(started.elapsed()));
    }
    Ok(graph)
}

fn charge(
    control: &SearchControl,
    interrupt: &dyn SearchInterrupt,
    started: Instant,
    work_used: &mut u64,
    cost: u64,
) -> Option<String> {
    if interrupt.is_cancelled() {
        return Some("interrupt cancelled harmonization".to_owned());
    }
    if control
        .max_time
        .is_some_and(|limit| started.elapsed() >= limit)
    {
        return Some("time bound reached".to_owned());
    }
    let Some(next) = work_used.checked_add(cost) else {
        return Some("work counter overflowed".to_owned());
    };
    if control.max_work.is_some_and(|limit| next > limit) {
        return Some("work bound reached".to_owned());
    }
    *work_used = next;
    None
}

fn partial(
    strategy: HarmonizationStrategy,
    reason: String,
    work_used: u64,
    candidates: &[PlannerState],
    context: &PlannerContext<'_>,
    max_results: Option<usize>,
) -> HarmonizationRun {
    let mut results = candidates
        .iter()
        .map(|state| state.result(state.indices.len() == context.request.melody.len()))
        .collect::<Vec<_>>();
    results.sort_by(|left, right| {
        right
            .progression
            .len()
            .cmp(&left.progression.len())
            .then_with(|| compare_results(left, right))
    });
    results.truncate(max_results.unwrap_or(results.len()));
    stopped(
        strategy,
        if reason.contains("interrupt") {
            SearchStatus::Cancelled
        } else {
            SearchStatus::Partial
        },
        &reason,
        work_used,
        context.stats.borrow().evaluated,
        results,
        context.stats.borrow().rejections(),
    )
}

fn stopped(
    strategy: HarmonizationStrategy,
    status: SearchStatus,
    reason: &str,
    work_used: u64,
    evaluated_candidates: u64,
    results: Vec<HarmonizationResult>,
    rejections: Vec<crate::HarmonyRejection>,
) -> HarmonizationRun {
    let result_count = results.len();
    HarmonizationRun {
        results,
        receipt: HarmonizationReceipt {
            strategy,
            status,
            reason: Some(reason.to_owned()),
            work_used,
            evaluated_candidates,
            result_count,
            optimal: false,
            search: None,
            layered: None,
            optimality: None,
            rejections,
        },
    }
}

fn quantized_step(score: f64) -> Result<i64, HarmonyError> {
    let scaled = score * crate::HARMONY_SCORE_SCALE as f64;
    if !scaled.is_finite() || scaled < i64::MIN as f64 || scaled > i64::MAX as f64 {
        return Err(HarmonyError::InvalidField {
            field: "harmonization.score",
            reason: format!("score {score} cannot be represented at fixed precision"),
        });
    }
    Ok(scaled.round() as i64)
}

struct InterruptAdapter<'a>(&'a dyn SearchInterrupt);

impl AlgorithmInterrupt for InterruptAdapter<'_> {
    fn is_cancelled(&self) -> bool {
        self.0.is_cancelled()
    }
}
