use std::{collections::BTreeMap, fmt::Debug};

use sim_lib_discrete_search::{
    NeverInterrupt, SearchControl, SearchInterrupt, SearchOrder, SearchProblem, SearchRun,
    SearchStep, solve,
};

use crate::{
    Derivation, DerivationGeneration, DerivationNode, DerivationReceipt, ExpansionLimits, LSystem,
    LSystemError, ProductionContext, RewriteEvidence, SymbolRole,
};

/// Derives results admitted by the bounded deterministic search.
pub fn derive<S>(
    system: &LSystem<S>,
    generations: u32,
    control: SearchControl,
) -> Result<SearchRun<Derivation<S>>, LSystemError>
where
    S: Ord + Clone + Debug,
{
    derive_with_interrupt(system, generations, control, &NeverInterrupt)
}

/// Derives results while polling a caller-owned interrupt between charged work.
pub fn derive_with_interrupt<S>(
    system: &LSystem<S>,
    generations: u32,
    control: SearchControl,
    interrupt: &dyn SearchInterrupt,
) -> Result<SearchRun<Derivation<S>>, LSystemError>
where
    S: Ord + Clone + Debug,
{
    validate_request(system, generations, &control)?;
    let problem = RewriteProblem {
        system,
        generations,
        seed: control.seed,
    };
    Ok(solve(&problem, control, interrupt))
}

fn validate_request<S: Ord + Clone>(
    system: &LSystem<S>,
    generations: u32,
    control: &SearchControl,
) -> Result<(), LSystemError> {
    system.validate()?;
    if generations > system.limits.max_generations {
        return Err(LSystemError::GenerationLimit {
            requested: generations,
            maximum: system.limits.max_generations,
        });
    }
    validate_search_control(control)?;

    let maximum_replacement = system
        .rules
        .iter()
        .map(|rule| rule.successor.len())
        .max()
        .unwrap_or(1)
        .max(1);
    let mut generation_symbols = system.axiom.len();
    let mut total_symbols = generation_symbols;
    for generation in 1..=generations {
        generation_symbols = generation_symbols.checked_mul(maximum_replacement).ok_or(
            LSystemError::ExpansionCouldExceed {
                generation,
                symbols: None,
            },
        )?;
        if generation_symbols > system.limits.max_symbols_per_generation {
            return Err(LSystemError::ExpansionCouldExceed {
                generation,
                symbols: Some(generation_symbols),
            });
        }
        total_symbols = total_symbols.checked_add(generation_symbols).ok_or(
            LSystemError::ExpansionCouldExceed {
                generation,
                symbols: None,
            },
        )?;
        if total_symbols > system.limits.max_total_symbols {
            return Err(LSystemError::ExpansionCouldExceed {
                generation,
                symbols: Some(total_symbols),
            });
        }
    }
    Ok(())
}

fn validate_search_control(control: &SearchControl) -> Result<(), LSystemError> {
    for (field, value) in [
        ("max_work", control.max_work),
        (
            "max_results",
            control
                .max_results
                .and_then(|value| u64::try_from(value).ok()),
        ),
        (
            "max_frontier",
            control
                .max_frontier
                .and_then(|value| u64::try_from(value).ok()),
        ),
        (
            "max_memory_nodes",
            control
                .max_memory_nodes
                .and_then(|value| u64::try_from(value).ok()),
        ),
    ] {
        match value {
            None => return Err(LSystemError::UnboundedSearch { field }),
            Some(0) => return Err(LSystemError::ZeroSearchBound { field }),
            Some(_) => {}
        }
    }
    if control.max_time.is_some() {
        return Err(LSystemError::WallClockLimit);
    }
    if matches!(control.order, SearchOrder::Beam { width: 0 }) {
        return Err(LSystemError::ZeroBeamWidth);
    }
    if control.costs.expand == 0
        || control.costs.score == 0
        || control.costs.propagate == 0
        || control.costs.emit == 0
    {
        return Err(LSystemError::ZeroWorkCost);
    }
    Ok(())
}

#[derive(Clone)]
struct Expansion<S> {
    source: Vec<S>,
    output: Vec<S>,
    rewrites: Vec<RewriteEvidence>,
    cursor: usize,
}

#[derive(Clone)]
struct RewriteState<S> {
    levels: Vec<Vec<S>>,
    steps: Vec<Vec<RewriteEvidence>>,
    expansion: Option<Expansion<S>>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ProductionChoice {
    seeded_order: u64,
    id: String,
    index: usize,
}

struct RewriteProblem<'a, S> {
    system: &'a LSystem<S>,
    generations: u32,
    seed: u64,
}

impl<S> SearchProblem for RewriteProblem<'_, S>
where
    S: Ord + Clone + Debug,
{
    type State = RewriteState<S>;
    type Choice = ProductionChoice;
    type Output = Derivation<S>;

    fn initial_state(&self) -> Self::State {
        RewriteState {
            levels: vec![self.system.axiom.clone()],
            steps: Vec::new(),
            expansion: None,
        }
    }

    fn expand(&self, state: &Self::State, out: &mut Vec<Self::Choice>) {
        let Some(expansion) = state.expansion.as_ref() else {
            return;
        };
        let Some(symbol) = expansion.source.get(expansion.cursor) else {
            return;
        };
        let mut applicable = self
            .system
            .rules
            .iter()
            .enumerate()
            .filter(|(_, rule)| {
                &rule.predecessor == symbol
                    && context_matches(&rule.context, &expansion.source, expansion.cursor)
            })
            .collect::<Vec<_>>();
        let specificity = applicable
            .iter()
            .map(|(_, rule)| rule.context.specificity())
            .max()
            .unwrap_or(0);
        applicable.retain(|(_, rule)| rule.context.specificity() == specificity);
        out.extend(
            applicable
                .into_iter()
                .map(|(index, rule)| ProductionChoice {
                    seeded_order: seeded_order(self.seed, &rule.id),
                    id: rule.id.clone(),
                    index,
                }),
        );
    }

    fn apply(&self, state: &Self::State, choice: &Self::Choice) -> SearchStep<Self::State> {
        let Some(rule) = self.system.rules.get(choice.index) else {
            return SearchStep::infeasible("production choice is outside rule table");
        };
        let mut next = state.clone();
        let Some(expansion) = next.expansion.as_mut() else {
            return SearchStep::infeasible("production choice has no active generation");
        };
        let Some(symbol) = expansion.source.get(expansion.cursor) else {
            return SearchStep::infeasible("production cursor is outside source generation");
        };
        if symbol != &rule.predecessor
            || !context_matches(&rule.context, &expansion.source, expansion.cursor)
        {
            return SearchStep::pruned("production does not match current symbol and context");
        }
        let output_start = expansion.output.len();
        expansion.output.extend(rule.successor.iter().cloned());
        expansion.rewrites.push(RewriteEvidence {
            production_id: Some(rule.id.clone()),
            output_start,
            output_end: expansion.output.len(),
        });
        expansion.cursor += 1;
        SearchStep::Continue(next)
    }

    fn propagate(&self, state: Self::State) -> SearchStep<Self::State> {
        normalize_state(state, self.system, self.generations)
    }

    fn finish(&self, state: &Self::State) -> Option<Self::Output> {
        if state.expansion.is_some()
            || state.levels.len() != usize::try_from(self.generations).ok()?.saturating_add(1)
        {
            return None;
        }
        Some(materialize(state, self.system.limits, self.seed))
    }
}

fn normalize_state<S: Ord + Clone>(
    mut state: RewriteState<S>,
    system: &LSystem<S>,
    generations: u32,
) -> SearchStep<RewriteState<S>> {
    loop {
        if state.levels.len() == generations as usize + 1 {
            state.expansion = None;
            return SearchStep::Continue(state);
        }
        if state.expansion.is_none() {
            state.expansion = Some(Expansion {
                source: state
                    .levels
                    .last()
                    .expect("validated derivation has generation zero")
                    .clone(),
                output: Vec::new(),
                rewrites: Vec::new(),
                cursor: 0,
            });
        }
        let expansion = state.expansion.as_mut().expect("expansion was initialized");
        while let Some(symbol) = expansion.source.get(expansion.cursor) {
            if system.alphabet.role(symbol) != Some(SymbolRole::Constant) {
                return SearchStep::Continue(state);
            }
            let output_start = expansion.output.len();
            expansion.output.push(symbol.clone());
            expansion.rewrites.push(RewriteEvidence {
                production_id: None,
                output_start,
                output_end: output_start + 1,
            });
            expansion.cursor += 1;
        }
        let complete = state.expansion.take().expect("complete expansion");
        state.steps.push(complete.rewrites);
        state.levels.push(complete.output);
    }
}

fn context_matches<S: PartialEq>(
    context: &ProductionContext<S>,
    source: &[S],
    cursor: usize,
) -> bool {
    let left_matches = context
        .left
        .as_ref()
        .is_none_or(|left| cursor.checked_sub(1).and_then(|index| source.get(index)) == Some(left));
    let right_matches = context
        .right
        .as_ref()
        .is_none_or(|right| source.get(cursor + 1) == Some(right));
    left_matches && right_matches
}

fn materialize<S: Clone>(
    state: &RewriteState<S>,
    limits: ExpansionLimits,
    seed: u64,
) -> Derivation<S> {
    let generations = state
        .levels
        .iter()
        .enumerate()
        .map(|(index, symbols)| DerivationGeneration {
            index: u32::try_from(index).expect("generation count was bounded"),
            symbols: symbols.clone(),
            rewrites: state.steps.get(index).cloned().unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    let forest = state.levels[0]
        .iter()
        .enumerate()
        .map(|(index, _)| build_node(state, 0, index))
        .collect();
    let mut production_uses = BTreeMap::new();
    for rewrite in state.steps.iter().flatten() {
        if let Some(id) = &rewrite.production_id {
            *production_uses.entry(id.clone()).or_insert(0) += 1;
        }
    }
    Derivation {
        generations,
        forest,
        receipt: DerivationReceipt {
            generations: u32::try_from(state.steps.len()).expect("generation count was bounded"),
            seed,
            total_symbols: state.levels.iter().map(Vec::len).sum(),
            production_uses,
            limits,
        },
    }
}

fn build_node<S: Clone>(
    state: &RewriteState<S>,
    generation: usize,
    index: usize,
) -> DerivationNode<S> {
    let rewrite = state
        .steps
        .get(generation)
        .and_then(|steps| steps.get(index));
    let children = rewrite
        .map(|evidence| {
            (evidence.output_start..evidence.output_end)
                .map(|child| build_node(state, generation + 1, child))
                .collect()
        })
        .unwrap_or_default();
    DerivationNode {
        symbol: state.levels[generation][index].clone(),
        generation: u32::try_from(generation).expect("generation count was bounded"),
        production_id: rewrite.and_then(|evidence| evidence.production_id.clone()),
        children,
    }
}

fn seeded_order(seed: u64, id: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64 ^ seed;
    for byte in id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
