//! Validated symbol-bearing series.

use crate::aggregate::ProjectedClassEvidence;
use crate::alphabet::validate_alphabet;
use crate::{AggregateLedger, AggregateRule, SerialAlphabet, SeriesError};
use sim_lib_discrete_rank::PermutationSpace;
use sim_lib_rank::Nat;
use std::collections::BTreeMap;

/// An ordered series of alphabet symbols validated against an aggregate rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Series<A: SerialAlphabet> {
    alphabet: A,
    rule: AggregateRule,
    order: Vec<A::Symbol>,
    ledger: AggregateLedger<A::Symbol>,
}

impl<A: SerialAlphabet> Series<A> {
    /// Constructs a series after validating alphabet, membership, and aggregate policy.
    pub fn try_new(
        alphabet: A,
        rule: AggregateRule,
        order: Vec<A::Symbol>,
    ) -> Result<Self, SeriesError> {
        let positions = validate_alphabet(&alphabet)?;
        let mut observed = alphabet
            .symbols()
            .iter()
            .cloned()
            .map(|symbol| (symbol, 0usize))
            .collect::<BTreeMap<_, _>>();
        let mut first_occurrence = vec![None; alphabet.symbols().len()];
        let mut order_positions = Vec::with_capacity(order.len());
        for (series_position, symbol) in order.iter().enumerate() {
            let Some(&alphabet_position) = positions.get(symbol) else {
                return Err(SeriesError::ForeignSymbol {
                    position: series_position,
                    alphabet_id: alphabet.id().clone(),
                });
            };
            order_positions.push(alphabet_position);
            if first_occurrence[alphabet_position].is_none() {
                first_occurrence[alphabet_position] = Some(series_position);
            }
            if let Some(count) = observed.get_mut(symbol) {
                *count += 1;
            }
        }

        let expected = validate_rule(
            &alphabet,
            &rule,
            &observed,
            &first_occurrence,
            &order_positions,
            order.len(),
        )?;
        let omitted = alphabet
            .symbols()
            .iter()
            .filter(|symbol| observed.get(*symbol) == Some(&0))
            .cloned()
            .collect();
        let repeated = alphabet
            .symbols()
            .iter()
            .filter(|symbol| observed.get(*symbol).is_some_and(|count| *count > 1))
            .cloned()
            .collect();
        let projected = projected_evidence(&rule, &order_positions)?;
        let ledger = AggregateLedger {
            alphabet_id: alphabet.id().clone(),
            rule: rule.kind(),
            series_len: order.len(),
            observed,
            expected,
            omitted,
            repeated,
            projected,
        };
        Ok(Self {
            alphabet,
            rule,
            order,
            ledger,
        })
    }

    /// Returns the alphabet value retained by this series.
    pub fn alphabet(&self) -> &A {
        &self.alphabet
    }

    /// Returns the aggregate rule retained by this series.
    pub fn rule(&self) -> &AggregateRule {
        &self.rule
    }

    /// Returns the ordered symbols, never caller-provided ordinals.
    pub fn order(&self) -> &[A::Symbol] {
        &self.order
    }

    /// Returns construction evidence for membership and aggregate counts.
    pub fn ledger(&self) -> &AggregateLedger<A::Symbol> {
        &self.ledger
    }

    /// Returns the shared Lehmer rank when the series is exactly one permutation.
    ///
    /// This method delegates to [`PermutationSpace`] and does not enumerate any
    /// permutations in this crate.
    pub fn permutation_rank(&self) -> Result<Nat, SeriesError> {
        if !self.ledger.is_exhaustive_exactly_once() {
            return Err(SeriesError::NotPermutation(self.alphabet.id().clone()));
        }
        let positions = validate_alphabet(&self.alphabet)?;
        let permutation = self
            .order
            .iter()
            .map(|symbol| {
                positions
                    .get(symbol)
                    .copied()
                    .ok_or_else(|| SeriesError::NotPermutation(self.alphabet.id().clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PermutationSpace::try_new(self.alphabet.symbols().len())?.rank(&permutation)?)
    }

    /// Consumes the series into its validated parts.
    pub fn into_parts(self) -> (A, AggregateRule, Vec<A::Symbol>) {
        (self.alphabet, self.rule, self.order)
    }
}

fn validate_rule<A: SerialAlphabet>(
    alphabet: &A,
    rule: &AggregateRule,
    observed: &BTreeMap<A::Symbol, usize>,
    first_occurrence: &[Option<usize>],
    order_positions: &[usize],
    order_len: usize,
) -> Result<Option<BTreeMap<A::Symbol, usize>>, SeriesError> {
    match rule {
        AggregateRule::ExhaustiveExactlyOnce => {
            require_length(alphabet.symbols().len(), order_len)?;
            let expected = vec![1; alphabet.symbols().len()];
            compare_symbol_counts(alphabet, observed, &expected)?;
            Ok(Some(expected_map(alphabet, &expected)))
        }
        AggregateRule::NoRepeat => {
            reject_repeats(order_positions, first_occurrence)?;
            Ok(None)
        }
        AggregateRule::DeclaredMultiplicity(_) | AggregateRule::DeclaredOmissions(_) => {
            let declared = rule
                .declared()
                .ok_or_else(|| SeriesError::NotPermutation(alphabet.id().clone()))?;
            declared.validate_for(alphabet)?;
            let expected = declared.expected();
            require_length(checked_len(expected)?, order_len)?;
            compare_symbol_counts(alphabet, observed, expected)?;
            Ok(Some(expected_map(alphabet, expected)))
        }
        AggregateRule::ProjectedAggregate(_) => {
            let projected = rule
                .projected()
                .ok_or_else(|| SeriesError::NotPermutation(alphabet.id().clone()))?;
            projected.validate_for(alphabet)?;
            require_length(projected.required_len()?, order_len)?;
            let evidence = projected_evidence(rule, order_positions)?;
            for class in evidence {
                if class.observed != class.expected {
                    return Err(SeriesError::ProjectionMismatch {
                        class_id: class.id,
                        expected: class.expected,
                        found: class.observed,
                    });
                }
            }
            Ok(None)
        }
        AggregateRule::FreeOrder => Ok(None),
    }
}

fn reject_repeats(
    order_positions: &[usize],
    first_occurrence: &[Option<usize>],
) -> Result<(), SeriesError> {
    let mut seen = vec![false; first_occurrence.len()];
    for (position, &alphabet_position) in order_positions.iter().enumerate() {
        if seen[alphabet_position] {
            return Err(SeriesError::RepeatedSymbol {
                position,
                first: first_occurrence[alphabet_position].unwrap_or(position),
            });
        }
        seen[alphabet_position] = true;
    }
    Ok(())
}

fn compare_symbol_counts<A: SerialAlphabet>(
    alphabet: &A,
    observed: &BTreeMap<A::Symbol, usize>,
    expected: &[usize],
) -> Result<(), SeriesError> {
    for (alphabet_position, symbol) in alphabet.symbols().iter().enumerate() {
        let found = observed.get(symbol).copied().unwrap_or(0);
        if found != expected[alphabet_position] {
            return Err(SeriesError::MultiplicityMismatch {
                alphabet_position,
                expected: expected[alphabet_position],
                found,
            });
        }
    }
    Ok(())
}

fn expected_map<A: SerialAlphabet>(alphabet: &A, expected: &[usize]) -> BTreeMap<A::Symbol, usize> {
    alphabet
        .symbols()
        .iter()
        .cloned()
        .zip(expected.iter().copied())
        .collect()
}

fn projected_evidence(
    rule: &AggregateRule,
    order_positions: &[usize],
) -> Result<Vec<ProjectedClassEvidence>, SeriesError> {
    let Some(projected) = rule.projected() else {
        return Ok(Vec::new());
    };
    let mut observed = vec![0usize; projected.classes().len()];
    for &position in order_positions {
        let Some(&class) = projected.class_by_position().get(position) else {
            return Err(SeriesError::Rule(
                crate::AggregateRuleError::CardinalityMismatch {
                    expected: projected.class_by_position().len(),
                    found: position.saturating_add(1),
                },
            ));
        };
        observed[class] += 1;
    }
    Ok(projected
        .classes()
        .iter()
        .zip(observed)
        .map(|(class, observed)| ProjectedClassEvidence {
            id: class.id().clone(),
            expected: class.multiplicity(),
            observed,
        })
        .collect())
}

fn require_length(expected: usize, found: usize) -> Result<(), SeriesError> {
    if expected == found {
        Ok(())
    } else {
        Err(SeriesError::WrongLength { expected, found })
    }
}

fn checked_len(counts: &[usize]) -> Result<usize, SeriesError> {
    counts.iter().try_fold(0usize, |total, &count| {
        total.checked_add(count).ok_or(SeriesError::Rule(
            crate::AggregateRuleError::MultiplicityOverflow,
        ))
    })
}
