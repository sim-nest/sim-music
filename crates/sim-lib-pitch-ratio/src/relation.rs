//! Lazy, cycle-safe relation trees over exact ratios.

use sim_lib_discrete_search::{
    NeverInterrupt, SearchControl, SearchProblem, SearchRun, SearchStep, solve,
};

use crate::{PitchRatio, RatioPolicy};

/// One labeled ratio relation step.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RatioRelation {
    /// Stable relation label.
    pub label: String,
    /// Exact ratio applied from the current node to the next node.
    pub interval: PitchRatio,
}

impl RatioRelation {
    /// Construct a relation step.
    pub fn new(label: impl Into<String>, interval: PitchRatio) -> Self {
        Self {
            label: label.into(),
            interval,
        }
    }
}

/// A path emitted from a bounded ratio relation tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RatioRelationPath {
    /// Ratios visited from the root through the terminal node.
    pub nodes: Vec<PitchRatio>,
    /// Relation labels applied between adjacent nodes.
    pub labels: Vec<String>,
}

/// Lazily expand a cycle-safe relation tree under generic search control.
pub fn expand_ratio_relation_tree(
    root: PitchRatio,
    relations: &[RatioRelation],
    policy: RatioPolicy,
    control: SearchControl,
) -> SearchRun<RatioRelationPath> {
    solve(
        &RatioRelationProblem {
            root: root.canonical(policy).unwrap_or(root),
            relations,
            policy,
        },
        control,
        &NeverInterrupt,
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RatioRelationState {
    nodes: Vec<PitchRatio>,
    labels: Vec<String>,
    terminal: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RatioRelationChoice {
    Emit,
    Step { label: String, interval: PitchRatio },
}

struct RatioRelationProblem<'a> {
    root: PitchRatio,
    relations: &'a [RatioRelation],
    policy: RatioPolicy,
}

impl SearchProblem for RatioRelationProblem<'_> {
    type State = RatioRelationState;
    type Choice = RatioRelationChoice;
    type Output = RatioRelationPath;

    fn initial_state(&self) -> Self::State {
        RatioRelationState {
            nodes: vec![self.root],
            labels: Vec::new(),
            terminal: false,
        }
    }

    fn expand(&self, state: &Self::State, out: &mut Vec<Self::Choice>) {
        if state.terminal {
            return;
        }
        if !state.labels.is_empty() {
            out.push(RatioRelationChoice::Emit);
        }
        out.extend(
            self.relations
                .iter()
                .map(|relation| RatioRelationChoice::Step {
                    label: relation.label.clone(),
                    interval: relation.interval,
                }),
        );
    }

    fn apply(&self, state: &Self::State, choice: &Self::Choice) -> SearchStep<Self::State> {
        let RatioRelationChoice::Step { label, interval } = choice else {
            let mut terminal = state.clone();
            terminal.terminal = true;
            return SearchStep::Continue(terminal);
        };
        let Some(current) = state.nodes.last().copied() else {
            return SearchStep::infeasible("relation path has no current node");
        };
        let Ok(next) = current
            .multiply(*interval)
            .and_then(|ratio| ratio.canonical(self.policy))
        else {
            return SearchStep::pruned("relation exceeds ratio policy");
        };
        if state.nodes.contains(&next) {
            return SearchStep::pruned("ratio relation cycle");
        }
        let mut nodes = state.nodes.clone();
        nodes.push(next);
        let mut labels = state.labels.clone();
        labels.push(label.clone());
        SearchStep::Continue(RatioRelationState {
            nodes,
            labels,
            terminal: false,
        })
    }

    fn finish(&self, state: &Self::State) -> Option<Self::Output> {
        if !state.terminal {
            return None;
        }
        Some(RatioRelationPath {
            nodes: state.nodes.clone(),
            labels: state.labels.clone(),
        })
    }

    fn score_state(&self, state: &Self::State) -> i64 {
        i64::try_from(state.labels.len()).unwrap_or(i64::MAX)
    }
}
