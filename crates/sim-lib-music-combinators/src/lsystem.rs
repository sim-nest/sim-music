use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

/// Hard safety ceiling for caller-declared L-system generation limits.
pub const MAX_LSYSTEM_GENERATIONS: u32 = 256;
/// Hard safety ceiling for caller-declared symbols per generation.
pub const MAX_LSYSTEM_SYMBOLS_PER_GENERATION: usize = 1_000_000;
/// Hard safety ceiling for caller-declared symbols across one derivation.
pub const MAX_LSYSTEM_TOTAL_SYMBOLS: usize = 4_000_000;

/// Whether an alphabet member is rewritten or copied between generations.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SymbolRole {
    /// A variable must have at least one context-free productive production.
    Variable,
    /// A constant is copied unchanged into the next generation.
    Constant,
}

/// A finite typed alphabet with an explicit role for every symbol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alphabet<S> {
    symbols: BTreeMap<S, SymbolRole>,
}

impl<S: Ord> Alphabet<S> {
    /// Builds an alphabet, rejecting duplicate symbols and empty alphabets.
    pub fn new(entries: impl IntoIterator<Item = (S, SymbolRole)>) -> Result<Self, LSystemError> {
        let mut symbols = BTreeMap::new();
        for (symbol, role) in entries {
            if symbols.insert(symbol, role).is_some() {
                return Err(LSystemError::DuplicateAlphabetSymbol);
            }
        }
        if symbols.is_empty() {
            return Err(LSystemError::EmptyAlphabet);
        }
        Ok(Self { symbols })
    }

    /// Returns the declared role of `symbol`.
    pub fn role(&self, symbol: &S) -> Option<SymbolRole> {
        self.symbols.get(symbol).copied()
    }

    /// Iterates over symbols and roles in stable symbol order.
    pub fn iter(&self) -> impl Iterator<Item = (&S, SymbolRole)> {
        self.symbols.iter().map(|(symbol, role)| (symbol, *role))
    }

    /// Returns the number of declared symbols.
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Returns whether the alphabet has no symbols.
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }
}

/// Immediate-neighbor context required by a production.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProductionContext<S> {
    /// Required symbol immediately to the left, when present.
    pub left: Option<S>,
    /// Required symbol immediately to the right, when present.
    pub right: Option<S>,
}

impl<S> ProductionContext<S> {
    /// Builds a context-free production context.
    pub const fn free() -> Self {
        Self {
            left: None,
            right: None,
        }
    }

    /// Builds a context with optional immediate neighbors.
    pub const fn new(left: Option<S>, right: Option<S>) -> Self {
        Self { left, right }
    }

    pub(crate) fn specificity(&self) -> u8 {
        u8::from(self.left.is_some()) + u8::from(self.right.is_some())
    }
}

/// One named, productive parallel-rewrite production.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Production<S> {
    /// Stable production identifier retained in derivation receipts.
    pub id: String,
    /// Variable replaced by this production.
    pub predecessor: S,
    /// Optional immediate-neighbor requirements.
    pub context: ProductionContext<S>,
    /// Non-empty replacement symbols.
    pub successor: Vec<S>,
}

impl<S> Production<S> {
    /// Builds a production from data.
    pub fn new(
        id: impl Into<String>,
        predecessor: S,
        context: ProductionContext<S>,
        successor: Vec<S>,
    ) -> Self {
        Self {
            id: id.into(),
            predecessor,
            context,
            successor,
        }
    }
}

/// Explicit finite expansion bounds carried by an [`LSystem`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ExpansionLimits {
    /// Largest generation index that may be requested.
    pub max_generations: u32,
    /// Largest symbol count admitted in any generation.
    pub max_symbols_per_generation: usize,
    /// Largest sum of symbol counts across all generations.
    pub max_total_symbols: usize,
}

impl ExpansionLimits {
    /// Builds expansion limits. They are validated with the system before use.
    pub const fn new(
        max_generations: u32,
        max_symbols_per_generation: usize,
        max_total_symbols: usize,
    ) -> Self {
        Self {
            max_generations,
            max_symbols_per_generation,
            max_total_symbols,
        }
    }

    fn validate(self) -> Result<(), LSystemError> {
        if self.max_generations > MAX_LSYSTEM_GENERATIONS {
            return Err(LSystemError::LimitPolicy {
                detail: format!(
                    "max_generations {} exceeds hard ceiling {MAX_LSYSTEM_GENERATIONS}",
                    self.max_generations
                ),
            });
        }
        if self.max_symbols_per_generation == 0
            || self.max_symbols_per_generation > MAX_LSYSTEM_SYMBOLS_PER_GENERATION
        {
            return Err(LSystemError::LimitPolicy {
                detail: format!(
                    "max_symbols_per_generation must be in 1..={MAX_LSYSTEM_SYMBOLS_PER_GENERATION}"
                ),
            });
        }
        if self.max_total_symbols == 0 || self.max_total_symbols > MAX_LSYSTEM_TOTAL_SYMBOLS {
            return Err(LSystemError::LimitPolicy {
                detail: format!("max_total_symbols must be in 1..={MAX_LSYSTEM_TOTAL_SYMBOLS}"),
            });
        }
        Ok(())
    }
}

/// A finite parallel-rewrite system over a typed alphabet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LSystem<S> {
    /// Typed finite alphabet.
    pub alphabet: Alphabet<S>,
    /// Non-empty generation-zero seed.
    pub axiom: Vec<S>,
    /// Named productions, including a context-free fallback for every variable.
    pub rules: Vec<Production<S>>,
    /// Generation and expansion limits.
    pub limits: ExpansionLimits,
}

impl<S: Ord + Clone> LSystem<S> {
    /// Builds and validates a finite productive rewrite system.
    pub fn new(
        alphabet: Alphabet<S>,
        axiom: Vec<S>,
        rules: Vec<Production<S>>,
        limits: ExpansionLimits,
    ) -> Result<Self, LSystemError> {
        let system = Self {
            alphabet,
            axiom,
            rules,
            limits,
        };
        system.validate()?;
        Ok(system)
    }

    /// Validates alphabet membership, productivity, identifiers, and limits.
    pub fn validate(&self) -> Result<(), LSystemError> {
        self.limits.validate()?;
        if self.axiom.is_empty() {
            return Err(LSystemError::EmptyAxiom);
        }
        if self.axiom.len() > self.limits.max_symbols_per_generation
            || self.axiom.len() > self.limits.max_total_symbols
        {
            return Err(LSystemError::AxiomExceedsLimits);
        }
        for symbol in &self.axiom {
            if self.alphabet.role(symbol).is_none() {
                return Err(LSystemError::UnknownAxiomSymbol);
            }
        }

        let mut ids = BTreeSet::new();
        let mut fallbacks = BTreeSet::new();
        for rule in &self.rules {
            if rule.id.trim().is_empty() {
                return Err(LSystemError::EmptyProductionId);
            }
            if !ids.insert(rule.id.as_str()) {
                return Err(LSystemError::DuplicateProductionId {
                    id: rule.id.clone(),
                });
            }
            match self.alphabet.role(&rule.predecessor) {
                Some(SymbolRole::Variable) => {}
                Some(SymbolRole::Constant) => return Err(LSystemError::RewritesConstant),
                None => return Err(LSystemError::UnknownPredecessor),
            }
            if rule.successor.is_empty() {
                return Err(LSystemError::NonProductiveProduction {
                    id: rule.id.clone(),
                });
            }
            for symbol in rule
                .context
                .left
                .iter()
                .chain(rule.context.right.iter())
                .chain(rule.successor.iter())
            {
                if self.alphabet.role(symbol).is_none() {
                    return Err(LSystemError::UnknownProductionSymbol {
                        id: rule.id.clone(),
                    });
                }
            }
            if rule.context == ProductionContext::free() {
                fallbacks.insert(rule.predecessor.clone());
            }
        }
        for (symbol, role) in self.alphabet.iter() {
            if role == SymbolRole::Variable && !fallbacks.contains(symbol) {
                return Err(LSystemError::VariableWithoutFallback);
            }
        }
        Ok(())
    }
}

/// One source-symbol rewrite and its child span in the next generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewriteEvidence {
    /// Production identifier, or `None` for a copied constant.
    pub production_id: Option<String>,
    /// Inclusive child offset in the next generation.
    pub output_start: usize,
    /// Exclusive child offset in the next generation.
    pub output_end: usize,
}

/// One materialized generation and the rewrites leading to the next generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivationGeneration<S> {
    /// Zero-based generation index.
    pub index: u32,
    /// Symbols in stable left-to-right order.
    pub symbols: Vec<S>,
    /// One rewrite per symbol, empty only for the final generation.
    pub rewrites: Vec<RewriteEvidence>,
}

/// One node in an inspectable derivation forest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivationNode<S> {
    /// Symbol at this node.
    pub symbol: S,
    /// Zero-based generation containing the node.
    pub generation: u32,
    /// Production used to create `children`, or `None` for constants and leaves.
    pub production_id: Option<String>,
    /// Rewritten successor nodes in stable order.
    pub children: Vec<DerivationNode<S>>,
}

/// Domain receipt for one completed derivation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivationReceipt {
    /// Requested generation count.
    pub generations: u32,
    /// Caller-supplied deterministic search seed.
    pub seed: u64,
    /// Number of symbols across every retained generation.
    pub total_symbols: usize,
    /// Number of uses of each production in stable identifier order.
    pub production_uses: BTreeMap<String, usize>,
    /// Expansion policy applied to this derivation.
    pub limits: ExpansionLimits,
}

/// Materialized generations, derivation forest, and reproducibility receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Derivation<S> {
    /// All generations from the axiom through the requested result.
    pub generations: Vec<DerivationGeneration<S>>,
    /// One derivation-tree root per axiom symbol.
    pub forest: Vec<DerivationNode<S>>,
    /// Domain-specific derivation receipt.
    pub receipt: DerivationReceipt,
}

/// Invalid system or policy rejected before search starts.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LSystemError {
    /// An alphabet must contain at least one symbol.
    #[error("L-system alphabet is empty")]
    EmptyAlphabet,
    /// An alphabet declared one symbol more than once.
    #[error("L-system alphabet contains a duplicate symbol")]
    DuplicateAlphabetSymbol,
    /// Generation zero must contain at least one symbol.
    #[error("L-system axiom is empty")]
    EmptyAxiom,
    /// The axiom contains a symbol absent from the alphabet.
    #[error("L-system axiom contains an undeclared symbol")]
    UnknownAxiomSymbol,
    /// The axiom exceeds its declared expansion limits.
    #[error("L-system axiom exceeds expansion limits")]
    AxiomExceedsLimits,
    /// A production identifier is blank.
    #[error("L-system production identifier is empty")]
    EmptyProductionId,
    /// Two productions share one identifier.
    #[error("duplicate L-system production identifier {id}")]
    DuplicateProductionId {
        /// Duplicated identifier.
        id: String,
    },
    /// A production predecessor is absent from the alphabet.
    #[error("L-system production predecessor is undeclared")]
    UnknownPredecessor,
    /// A production attempted to rewrite a constant.
    #[error("L-system production cannot rewrite a constant")]
    RewritesConstant,
    /// A production context or successor contains an undeclared symbol.
    #[error("L-system production {id} contains an undeclared symbol")]
    UnknownProductionSymbol {
        /// Production identifier.
        id: String,
    },
    /// Empty successors are non-productive and unsupported.
    #[error("L-system production {id} has an empty successor")]
    NonProductiveProduction {
        /// Production identifier.
        id: String,
    },
    /// Every variable needs a context-free productive fallback.
    #[error("L-system variable has no context-free productive fallback")]
    VariableWithoutFallback,
    /// A caller-declared limit is zero or beyond a hard safety ceiling.
    #[error("invalid L-system limit policy: {detail}")]
    LimitPolicy {
        /// Validation detail.
        detail: String,
    },
    /// Requested generations exceed the system policy.
    #[error("requested {requested} generations exceeds limit {maximum}")]
    GenerationLimit {
        /// Requested generation count.
        requested: u32,
        /// Declared maximum.
        maximum: u32,
    },
    /// Conservative preflight proves the system could exceed an expansion limit.
    #[error("L-system expansion could exceed limits at generation {generation}")]
    ExpansionCouldExceed {
        /// First generation whose conservative upper bound is invalid.
        generation: u32,
        /// Computed symbol or total-symbol upper bound, when representable.
        symbols: Option<usize>,
    },
    /// Search controls must bound work, results, frontier, and memory.
    #[error("L-system search control is unbounded: {field}")]
    UnboundedSearch {
        /// Missing bound.
        field: &'static str,
    },
    /// A search bound was explicitly zero.
    #[error("L-system search control bound {field} must be positive")]
    ZeroSearchBound {
        /// Invalid bound.
        field: &'static str,
    },
    /// Wall-clock stopping is not reproducible.
    #[error("L-system derivation rejects wall-clock limits; use charged work bounds")]
    WallClockLimit,
    /// A beam search must retain at least one node.
    #[error("L-system beam width must be positive")]
    ZeroBeamWidth,
    /// Every work charge must be positive.
    #[error("L-system search work costs must be positive")]
    ZeroWorkCost,
}
