use sim_lib_discrete_search::{
    NeverInterrupt, SearchControl, SearchProblem, SearchRun, SearchStep, solve,
};
use sim_lib_sound_core::Envelope;

use crate::{AttackKind, Filter, Timbre, TimbreMeta, TimbreRecipe, render::default_env};

/// Discrete design space for bounded timbre enumeration.
#[derive(Clone, Debug, PartialEq)]
pub struct TimbreFamily {
    /// Family label used in generated timbre names and metadata.
    pub name: String,
    /// Candidate recipes visited as the first enumeration axis.
    pub recipes: Vec<TimbreRecipe>,
    /// Candidate envelopes visited as the second enumeration axis.
    pub envelopes: Vec<Envelope>,
    /// Candidate filters visited as the third enumeration axis.
    pub filters: Vec<Vec<Filter>>,
}

impl TimbreFamily {
    /// Builds a family with default envelopes and no filters.
    pub fn new(name: impl Into<String>, recipes: Vec<TimbreRecipe>) -> Self {
        Self {
            name: name.into(),
            recipes,
            envelopes: vec![default_env()],
            filters: vec![Vec::new()],
        }
    }
}

/// Enumerates a timbre family with deterministic discrete search controls.
pub fn enumerate_timbres(spec: &TimbreFamily, control: SearchControl) -> SearchRun<Timbre> {
    solve(&TimbreFamilyProblem { spec }, control, &NeverInterrupt)
}

struct TimbreFamilyProblem<'a> {
    spec: &'a TimbreFamily,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TimbreFamilyState {
    recipe: Option<usize>,
    envelope: Option<usize>,
    filters: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TimbreChoice {
    Recipe(usize),
    Envelope(usize),
    Filters(usize),
}

impl SearchProblem for TimbreFamilyProblem<'_> {
    type State = TimbreFamilyState;
    type Choice = TimbreChoice;
    type Output = Timbre;

    fn initial_state(&self) -> Self::State {
        TimbreFamilyState::default()
    }

    fn expand(&self, state: &Self::State, out: &mut Vec<Self::Choice>) {
        if state.recipe.is_none() {
            out.extend((0..self.spec.recipes.len()).map(TimbreChoice::Recipe));
        } else if state.envelope.is_none() {
            out.extend((0..self.spec.envelopes.len()).map(TimbreChoice::Envelope));
        } else if state.filters.is_none() {
            out.extend((0..self.spec.filters.len()).map(TimbreChoice::Filters));
        }
    }

    fn apply(&self, state: &Self::State, choice: &Self::Choice) -> SearchStep<Self::State> {
        let mut next = state.clone();
        match *choice {
            TimbreChoice::Recipe(index) if index < self.spec.recipes.len() => {
                next.recipe = Some(index);
            }
            TimbreChoice::Envelope(index) if index < self.spec.envelopes.len() => {
                next.envelope = Some(index);
            }
            TimbreChoice::Filters(index) if index < self.spec.filters.len() => {
                next.filters = Some(index);
            }
            _ => return SearchStep::infeasible("choice index outside timbre family axis"),
        }
        SearchStep::Continue(next)
    }

    fn finish(&self, state: &Self::State) -> Option<Self::Output> {
        let recipe = state.recipe?;
        let envelope = state.envelope?;
        let filters = state.filters?;
        Some(Timbre {
            name: format!("{}-{recipe}-{envelope}-{filters}", self.spec.name),
            recipe: self.spec.recipes[recipe].clone(),
            default_envelope: self.spec.envelopes[envelope].clone(),
            metadata: TimbreMeta {
                brightness: 2.0 + recipe as f64,
                roughness: filters as f64 * 0.1,
                attack_kind: AttackKind::Soft,
                category: self.spec.name.clone(),
            },
            filters: self.spec.filters[filters].clone(),
        })
    }
}
