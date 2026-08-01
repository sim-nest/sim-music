//! Aggregate policies and validation evidence.

use crate::alphabet::{validate_alphabet, validate_stable_id};
use crate::{AggregateRuleError, AlphabetId, SerialAlphabet};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};

/// One alphabet symbol paired with its declared aggregate count.
pub type SymbolCount<S> = (S, usize);

/// Stable identity of one class in a projected aggregate.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectionId(String);

impl ProjectionId {
    /// Validates and constructs a projected-class id.
    pub fn try_new(value: impl Into<String>) -> Result<Self, AggregateRuleError> {
        let value = value.into();
        validate_stable_id(&value).map_err(|reason| AggregateRuleError::InvalidProjectionId {
            value: value.clone(),
            reason,
        })?;
        Ok(Self(value))
    }

    /// Returns the stable text identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ProjectionId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, formatter)
    }
}

/// Symbol-based declaration of one projected aggregate class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectedClassSpec<S> {
    /// Stable class identity.
    pub id: ProjectionId,
    /// Source-alphabet symbols that project to this class.
    pub symbols: Vec<S>,
    /// Required number of class occurrences in the series.
    pub multiplicity: usize,
}

impl<S> ProjectedClassSpec<S> {
    /// Constructs a class specification. Full membership is validated by the rule constructor.
    pub fn new(id: ProjectionId, symbols: Vec<S>, multiplicity: usize) -> Self {
        Self {
            id,
            symbols,
            multiplicity,
        }
    }
}

/// Public category of an [`AggregateRule`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum AggregateRuleKind {
    /// Every alphabet symbol occurs exactly once.
    ExhaustiveExactlyOnce,
    /// Symbols may be omitted but no symbol may repeat.
    NoRepeat,
    /// Every symbol has an explicitly declared positive multiplicity.
    DeclaredMultiplicity,
    /// Every non-omitted symbol occurs exactly once.
    DeclaredOmissions,
    /// Occurrence requirements apply to declared projection classes.
    ProjectedAggregate,
    /// Any finite order of alphabet members is accepted, including repeats.
    FreeOrder,
}

/// Aggregate rule data compiled from symbols into private canonical positions.
/// Callers never provide raw ordinals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AggregateRule {
    /// Every alphabet symbol occurs exactly once.
    ExhaustiveExactlyOnce,
    /// Symbols may be omitted but no symbol may repeat.
    NoRepeat,
    /// Explicit per-symbol multiplicity data.
    DeclaredMultiplicity(DeclaredCounts),
    /// Explicit set of omitted symbols; every other symbol occurs once.
    DeclaredOmissions(DeclaredCounts),
    /// Explicit projection classes and their required multiplicities.
    ProjectedAggregate(ProjectedRule),
    /// Any finite order of alphabet members, including repeats.
    FreeOrder,
}

/// Alphabet-bound expected counts retained by a declared aggregate rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredCounts {
    alphabet_id: AlphabetId,
    expected: Vec<usize>,
}

/// One compiled projected class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectedClassRule {
    id: ProjectionId,
    members: Vec<usize>,
    multiplicity: usize,
}

/// Alphabet-bound projected aggregate data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectedRule {
    alphabet_id: AlphabetId,
    cardinality: usize,
    classes: Vec<ProjectedClassRule>,
    class_by_position: Vec<usize>,
}

impl AggregateRule {
    /// Constructs an exhaustive exactly-once rule.
    pub const fn exhaustive_exactly_once() -> Self {
        Self::ExhaustiveExactlyOnce
    }

    /// Constructs a no-repeat rule.
    pub const fn no_repeat() -> Self {
        Self::NoRepeat
    }

    /// Constructs a free-order rule.
    pub const fn free_order() -> Self {
        Self::FreeOrder
    }

    /// Compiles an explicit positive multiplicity for every alphabet symbol.
    pub fn declared_multiplicity<A, I>(
        alphabet: &A,
        declarations: I,
    ) -> Result<Self, AggregateRuleError>
    where
        A: SerialAlphabet,
        I: IntoIterator<Item = (A::Symbol, usize)>,
    {
        let positions = validate_alphabet(alphabet)?;
        let mut expected = vec![None; alphabet.symbols().len()];
        for (symbol, multiplicity) in declarations {
            let Some(&position) = positions.get(&symbol) else {
                return Err(AggregateRuleError::ForeignSymbol {
                    alphabet_id: alphabet.id().clone(),
                });
            };
            if expected[position].is_some() {
                return Err(AggregateRuleError::DuplicateDeclaration { position });
            }
            if multiplicity == 0 {
                return Err(AggregateRuleError::ZeroMultiplicity { position });
            }
            expected[position] = Some(multiplicity);
        }
        let expected = expected
            .into_iter()
            .enumerate()
            .map(|(position, count)| {
                count.ok_or(AggregateRuleError::MissingDeclaration { position })
            })
            .collect::<Result<Vec<_>, _>>()?;
        checked_total(&expected)?;
        Ok(Self::DeclaredMultiplicity(DeclaredCounts {
            alphabet_id: alphabet.id().clone(),
            expected,
        }))
    }

    /// Compiles a set of symbols to omit; every remaining symbol is required once.
    pub fn declared_omissions<A, I>(alphabet: &A, omissions: I) -> Result<Self, AggregateRuleError>
    where
        A: SerialAlphabet,
        I: IntoIterator<Item = A::Symbol>,
    {
        let positions = validate_alphabet(alphabet)?;
        let mut expected = vec![1; alphabet.symbols().len()];
        let mut omitted = BTreeSet::new();
        for symbol in omissions {
            let Some(&position) = positions.get(&symbol) else {
                return Err(AggregateRuleError::ForeignSymbol {
                    alphabet_id: alphabet.id().clone(),
                });
            };
            if !omitted.insert(position) {
                return Err(AggregateRuleError::DuplicateDeclaration { position });
            }
            expected[position] = 0;
        }
        if omitted.is_empty() {
            return Err(AggregateRuleError::NoOmissions);
        }
        if omitted.len() == alphabet.symbols().len() {
            return Err(AggregateRuleError::OmitsEverything(alphabet.id().clone()));
        }
        Ok(Self::DeclaredOmissions(DeclaredCounts {
            alphabet_id: alphabet.id().clone(),
            expected,
        }))
    }

    /// Compiles a complete, disjoint projection of alphabet symbols into classes.
    pub fn projected_aggregate<A, I>(alphabet: &A, classes: I) -> Result<Self, AggregateRuleError>
    where
        A: SerialAlphabet,
        I: IntoIterator<Item = ProjectedClassSpec<A::Symbol>>,
    {
        let positions = validate_alphabet(alphabet)?;
        let mut class_ids = BTreeSet::new();
        let mut class_by_position = vec![None; alphabet.symbols().len()];
        let mut compiled = Vec::new();
        let mut total = 0usize;
        for spec in classes {
            if !class_ids.insert(spec.id.clone()) {
                return Err(AggregateRuleError::DuplicateProjectionId(spec.id));
            }
            if spec.symbols.is_empty() {
                return Err(AggregateRuleError::EmptyProjectionClass(spec.id));
            }
            let class_index = compiled.len();
            let mut members = Vec::with_capacity(spec.symbols.len());
            for symbol in spec.symbols {
                let Some(&position) = positions.get(&symbol) else {
                    return Err(AggregateRuleError::ForeignSymbol {
                        alphabet_id: alphabet.id().clone(),
                    });
                };
                if class_by_position[position].replace(class_index).is_some() {
                    return Err(AggregateRuleError::DuplicateProjectionMember { position });
                }
                members.push(position);
            }
            total = total
                .checked_add(spec.multiplicity)
                .ok_or(AggregateRuleError::MultiplicityOverflow)?;
            compiled.push(ProjectedClassRule {
                id: spec.id,
                members,
                multiplicity: spec.multiplicity,
            });
        }
        for (position, class) in class_by_position.iter().enumerate() {
            if class.is_none() {
                return Err(AggregateRuleError::MissingProjectionMember { position });
            }
        }
        if total == 0 {
            return Err(AggregateRuleError::OmitsEverything(alphabet.id().clone()));
        }
        Ok(Self::ProjectedAggregate(ProjectedRule {
            alphabet_id: alphabet.id().clone(),
            cardinality: alphabet.symbols().len(),
            classes: compiled,
            class_by_position: class_by_position.into_iter().flatten().collect(),
        }))
    }

    /// Returns the public category of this rule.
    pub const fn kind(&self) -> AggregateRuleKind {
        match self {
            Self::ExhaustiveExactlyOnce => AggregateRuleKind::ExhaustiveExactlyOnce,
            Self::NoRepeat => AggregateRuleKind::NoRepeat,
            Self::DeclaredMultiplicity(_) => AggregateRuleKind::DeclaredMultiplicity,
            Self::DeclaredOmissions(_) => AggregateRuleKind::DeclaredOmissions,
            Self::ProjectedAggregate(_) => AggregateRuleKind::ProjectedAggregate,
            Self::FreeOrder => AggregateRuleKind::FreeOrder,
        }
    }

    /// Returns symbol/count declarations for a declared multiplicity or omission rule.
    pub fn declared_counts<A>(
        &self,
        alphabet: &A,
    ) -> Result<Option<Vec<SymbolCount<A::Symbol>>>, AggregateRuleError>
    where
        A: SerialAlphabet,
    {
        let counts = match self {
            Self::DeclaredMultiplicity(counts) | Self::DeclaredOmissions(counts) => counts,
            _ => return Ok(None),
        };
        counts.validate_for(alphabet)?;
        Ok(Some(
            alphabet
                .symbols()
                .iter()
                .cloned()
                .zip(counts.expected.iter().copied())
                .collect(),
        ))
    }

    /// Returns symbolic projected-class declarations for a projected rule.
    pub fn projected_classes<A>(
        &self,
        alphabet: &A,
    ) -> Result<Option<Vec<ProjectedClassSpec<A::Symbol>>>, AggregateRuleError>
    where
        A: SerialAlphabet,
    {
        let Self::ProjectedAggregate(rule) = self else {
            return Ok(None);
        };
        rule.validate_for(alphabet)?;
        Ok(Some(
            rule.classes
                .iter()
                .map(|class| {
                    ProjectedClassSpec::new(
                        class.id.clone(),
                        class
                            .members
                            .iter()
                            .map(|&position| alphabet.symbols()[position].clone())
                            .collect(),
                        class.multiplicity,
                    )
                })
                .collect(),
        ))
    }

    pub(crate) fn declared(&self) -> Option<&DeclaredCounts> {
        match self {
            Self::DeclaredMultiplicity(counts) | Self::DeclaredOmissions(counts) => Some(counts),
            _ => None,
        }
    }

    pub(crate) fn projected(&self) -> Option<&ProjectedRule> {
        match self {
            Self::ProjectedAggregate(rule) => Some(rule),
            _ => None,
        }
    }
}

impl DeclaredCounts {
    pub(crate) fn validate_for<A: SerialAlphabet>(
        &self,
        alphabet: &A,
    ) -> Result<(), AggregateRuleError> {
        validate_binding(&self.alphabet_id, self.expected.len(), alphabet)
    }

    pub(crate) fn expected(&self) -> &[usize] {
        &self.expected
    }
}

impl ProjectedRule {
    pub(crate) fn validate_for<A: SerialAlphabet>(
        &self,
        alphabet: &A,
    ) -> Result<(), AggregateRuleError> {
        validate_binding(&self.alphabet_id, self.cardinality, alphabet)
    }

    pub(crate) fn required_len(&self) -> Result<usize, AggregateRuleError> {
        checked_total(
            &self
                .classes
                .iter()
                .map(|class| class.multiplicity)
                .collect::<Vec<_>>(),
        )
    }

    pub(crate) fn class_by_position(&self) -> &[usize] {
        &self.class_by_position
    }

    pub(crate) fn classes(&self) -> &[ProjectedClassRule] {
        &self.classes
    }
}

impl ProjectedClassRule {
    pub(crate) fn id(&self) -> &ProjectionId {
        &self.id
    }

    pub(crate) fn multiplicity(&self) -> usize {
        self.multiplicity
    }
}

/// Observed and expected counts for one projected class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectedClassEvidence {
    /// Stable projected-class id.
    pub id: ProjectionId,
    /// Required number of occurrences.
    pub expected: usize,
    /// Observed number of occurrences.
    pub observed: usize,
}

/// Construction evidence retained by a valid [`crate::Series`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateLedger<S>
where
    S: Clone + Eq + Ord + std::fmt::Debug,
{
    pub(crate) alphabet_id: AlphabetId,
    pub(crate) rule: AggregateRuleKind,
    pub(crate) series_len: usize,
    pub(crate) observed: BTreeMap<S, usize>,
    pub(crate) expected: Option<BTreeMap<S, usize>>,
    pub(crate) omitted: Vec<S>,
    pub(crate) repeated: Vec<S>,
    pub(crate) projected: Vec<ProjectedClassEvidence>,
}

impl<S> AggregateLedger<S>
where
    S: Clone + Eq + Ord + std::fmt::Debug,
{
    /// Stable alphabet identity validated by this ledger.
    pub fn alphabet_id(&self) -> &AlphabetId {
        &self.alphabet_id
    }

    /// Aggregate rule category applied during validation.
    pub fn rule(&self) -> AggregateRuleKind {
        self.rule
    }

    /// Number of ordered positions validated.
    pub fn series_len(&self) -> usize {
        self.series_len
    }

    /// Observed count of `symbol`, or `None` when it is outside the alphabet.
    pub fn observed_count(&self, symbol: &S) -> Option<usize> {
        self.observed.get(symbol).copied()
    }

    /// Declared expected count of `symbol` when this rule has per-symbol expectations.
    pub fn expected_count(&self, symbol: &S) -> Option<usize> {
        self.expected
            .as_ref()
            .and_then(|counts| counts.get(symbol).copied())
    }

    /// Alphabet symbols absent from the supplied series.
    pub fn omitted_symbols(&self) -> &[S] {
        &self.omitted
    }

    /// Alphabet symbols occurring more than once.
    pub fn repeated_symbols(&self) -> &[S] {
        &self.repeated
    }

    /// Projected-class count evidence, empty for non-projected rules.
    pub fn projected_classes(&self) -> &[ProjectedClassEvidence] {
        &self.projected
    }

    /// Returns true when every alphabet symbol occurred exactly once.
    pub fn is_exhaustive_exactly_once(&self) -> bool {
        self.omitted.is_empty()
            && self.repeated.is_empty()
            && self.observed.values().all(|count| *count == 1)
    }
}

fn validate_binding<A: SerialAlphabet>(
    rule_id: &AlphabetId,
    cardinality: usize,
    alphabet: &A,
) -> Result<(), AggregateRuleError> {
    validate_alphabet(alphabet)?;
    if rule_id != alphabet.id() {
        return Err(AggregateRuleError::AlphabetMismatch {
            rule_id: rule_id.clone(),
            series_id: alphabet.id().clone(),
        });
    }
    if cardinality != alphabet.symbols().len() {
        return Err(AggregateRuleError::CardinalityMismatch {
            expected: cardinality,
            found: alphabet.symbols().len(),
        });
    }
    Ok(())
}

fn checked_total(counts: &[usize]) -> Result<usize, AggregateRuleError> {
    counts.iter().try_fold(0usize, |total, &count| {
        total
            .checked_add(count)
            .ok_or(AggregateRuleError::MultiplicityOverflow)
    })
}
