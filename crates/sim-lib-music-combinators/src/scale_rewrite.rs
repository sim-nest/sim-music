use std::collections::{BTreeMap, BTreeSet};

use sim_lib_discrete_search::{SearchControl, SearchRun};
use sim_lib_music_transform::{PitchMap, PitchMapPolicy};
use sim_lib_pitch_scale::Scale;
use thiserror::Error;

use crate::{
    Alphabet, Derivation, ExpansionLimits, LSystem, LSystemError, Production, SymbolRole, derive,
};

/// One named scale and the first-class pitch map that follows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScaleFollowingState {
    /// Stable rewrite symbol.
    pub id: String,
    /// Target scale described by the symbol.
    pub scale: Scale,
    /// Explicit pitch map used to follow the target scale.
    pub pitch_map: PitchMap,
}

impl ScaleFollowingState {
    /// Builds a state from explicit data and verifies scale-following behavior.
    pub fn new(
        id: impl Into<String>,
        scale: Scale,
        pitch_map: PitchMap,
    ) -> Result<Self, ScaleRewriteError> {
        let state = Self {
            id: id.into(),
            scale,
            pitch_map,
        };
        state.validate()?;
        Ok(state)
    }

    /// Builds a state from the standard partial scale pitch-map constructor.
    pub fn from_scale(
        id: impl Into<String>,
        scale: Scale,
        policy: PitchMapPolicy,
    ) -> Result<Self, ScaleRewriteError> {
        Self::new(id, scale, PitchMap::from_scale(scale, policy))
    }

    fn validate(&self) -> Result<(), ScaleRewriteError> {
        if self.id.trim().is_empty() {
            return Err(ScaleRewriteError::EmptyStateId);
        }
        if self.pitch_map.image.len() != 12 {
            return Err(ScaleRewriteError::NonChromaticMapDomain);
        }
        if self.pitch_map.policy == PitchMapPolicy::Unmapped {
            return Err(ScaleRewriteError::NonFollowingMapPolicy);
        }
        let classes = self
            .scale
            .pitch_classes()
            .into_iter()
            .map(|class| i32::from(class.value()))
            .collect::<BTreeSet<_>>();
        let mut mapped = 0usize;
        for target in self.pitch_map.image.iter().flatten() {
            if !classes.contains(&target.rem_euclid(12)) {
                return Err(ScaleRewriteError::MapLeavesScale { target: *target });
            }
            mapped += 1;
        }
        if mapped == 0 {
            return Err(ScaleRewriteError::EmptyPitchMap);
        }
        Ok(())
    }
}

/// A data-only scale modulation program compiled to the generic L-system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScaleRewriteProgram {
    /// Scale and pitch-map data keyed by rewrite symbol.
    pub states: BTreeMap<String, ScaleFollowingState>,
    /// Generic bounded rewrite system over state identifiers.
    pub system: LSystem<String>,
}

impl ScaleRewriteProgram {
    /// Builds and validates a scale rewrite program.
    pub fn new(
        states: Vec<ScaleFollowingState>,
        axiom: Vec<String>,
        rules: Vec<Production<String>>,
        limits: ExpansionLimits,
    ) -> Result<Self, ScaleRewriteError> {
        let mut state_map = BTreeMap::new();
        for state in states {
            state.validate()?;
            if state_map.insert(state.id.clone(), state).is_some() {
                return Err(ScaleRewriteError::DuplicateStateId);
            }
        }
        let alphabet = Alphabet::new(
            state_map
                .keys()
                .cloned()
                .map(|id| (id, SymbolRole::Variable)),
        )?;
        let system = LSystem::new(alphabet, axiom, rules, limits)?;
        Ok(Self {
            states: state_map,
            system,
        })
    }

    /// Derives scale states and retains both generic and pitch-map evidence.
    pub fn derive(
        &self,
        generations: u32,
        control: SearchControl,
    ) -> Result<SearchRun<ScaleDerivation>, ScaleRewriteError> {
        let run = derive(&self.system, generations, control)?;
        let outputs = run
            .outputs
            .into_iter()
            .map(|derivation| self.materialize(derivation))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SearchRun {
            outputs,
            receipt: run.receipt,
        })
    }

    fn materialize(
        &self,
        derivation: Derivation<String>,
    ) -> Result<ScaleDerivation, ScaleRewriteError> {
        let generations = derivation
            .generations
            .iter()
            .map(|generation| {
                generation
                    .symbols
                    .iter()
                    .map(|id| {
                        self.states.get(id).cloned().ok_or_else(|| {
                            ScaleRewriteError::MissingDerivedState { id: id.clone() }
                        })
                    })
                    .collect()
            })
            .collect::<Result<Vec<Vec<_>>, _>>()?;
        Ok(ScaleDerivation {
            program_digest: program_digest(&self.states, &self.system),
            derivation,
            generations,
        })
    }
}

/// A generic derivation paired with materialized scale and pitch-map states.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScaleDerivation {
    /// Stable digest of all scale states, pitch maps, and productions.
    pub program_digest: String,
    /// Generic inspectable derivation tree and receipt.
    pub derivation: Derivation<String>,
    /// Scale-following states for every retained generation.
    pub generations: Vec<Vec<ScaleFollowingState>>,
}

/// Invalid scale rewrite data or generic L-system policy.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ScaleRewriteError {
    /// State identifiers are required for stable receipts.
    #[error("scale rewrite state identifier is empty")]
    EmptyStateId,
    /// State identifiers must be unique.
    #[error("duplicate scale rewrite state identifier")]
    DuplicateStateId,
    /// Scale following operates on twelve-tone pitch maps.
    #[error("scale rewrite pitch map must use the twelve-tone domain")]
    NonChromaticMapDomain,
    /// Leaving holes unmapped does not guarantee scale following.
    #[error("scale rewrite pitch map cannot use the unmapped policy")]
    NonFollowingMapPolicy,
    /// A direct pitch-map target lies outside the declared scale.
    #[error("scale rewrite pitch-map target {target} lies outside the declared scale")]
    MapLeavesScale {
        /// Invalid absolute or folded map target.
        target: i32,
    },
    /// A pitch map needs at least one direct target.
    #[error("scale rewrite pitch map has no direct targets")]
    EmptyPitchMap,
    /// Materialization found an identifier absent from validated state data.
    #[error("derived scale rewrite state {id} is missing")]
    MissingDerivedState {
        /// Missing identifier.
        id: String,
    },
    /// Generic rewrite-system validation failed.
    #[error(transparent)]
    LSystem(#[from] LSystemError),
}

fn program_digest(
    states: &BTreeMap<String, ScaleFollowingState>,
    system: &LSystem<String>,
) -> String {
    let mut material = String::new();
    for (id, state) in states {
        material.push_str(id);
        material.push('|');
        material.push_str(&state.scale.tonic.value().to_string());
        material.push('|');
        material.push_str(state.scale.mode.name());
        material.push('|');
        material.push_str(&format!(
            "{:?}|{:?};",
            state.pitch_map.policy, state.pitch_map.image
        ));
    }
    for rule in &system.rules {
        material.push_str(&format!(
            "{}|{}|{:?}|{:?};",
            rule.id, rule.predecessor, rule.context, rule.successor
        ));
    }
    let mut hash = 0xcbf29ce484222325u64;
    for byte in material.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
