//! Total, evidence-producing transforms over validated finite series.

use crate::alphabet::validate_alphabet;
use crate::{
    AggregateRule, AggregateRuleKind, BlockPartition, OrdinalMap, ProjectedClassSpec,
    RelaxedInvariant, SerialAlphabet, Series, SeriesTransformError, SymbolBijectionError,
    TransformCertificate, TransformedSeries,
};

/// A caller-defined, validated bijection between two finite alphabets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolBijection<A: SerialAlphabet> {
    source: A,
    target: A,
    source_to_target: OrdinalMap,
}

impl<A: SerialAlphabet> SymbolBijection<A> {
    /// Validates a complete symbol-pair bijection from `source` to `target`.
    pub fn try_new<I>(source: A, target: A, pairs: I) -> Result<Self, SymbolBijectionError>
    where
        I: IntoIterator<Item = (A::Symbol, A::Symbol)>,
    {
        let source_positions = validate_alphabet(&source)?;
        let target_positions = validate_alphabet(&target)?;
        if source.symbols().len() != target.symbols().len() {
            return Err(SymbolBijectionError::CardinalityMismatch {
                source_cardinality: source.symbols().len(),
                target_cardinality: target.symbols().len(),
            });
        }

        let cardinality = source.symbols().len();
        let mut source_to_target = vec![None; cardinality];
        let mut target_sources = vec![None; cardinality];
        for (source_symbol, target_symbol) in pairs {
            let Some(&source_position) = source_positions.get(&source_symbol) else {
                return Err(SymbolBijectionError::ForeignSourceSymbol {
                    alphabet_id: source.id().clone(),
                });
            };
            let Some(&target_position) = target_positions.get(&target_symbol) else {
                return Err(SymbolBijectionError::ForeignTargetSymbol {
                    alphabet_id: target.id().clone(),
                });
            };
            if source_to_target[source_position]
                .replace(target_position)
                .is_some()
            {
                return Err(SymbolBijectionError::DuplicateSource {
                    position: source_position,
                });
            }
            if target_sources[target_position]
                .replace(source_position)
                .is_some()
            {
                return Err(SymbolBijectionError::DuplicateTarget {
                    position: target_position,
                });
            }
        }

        let mut complete = Vec::with_capacity(cardinality);
        for (position, target_position) in source_to_target.into_iter().enumerate() {
            let Some(target_position) = target_position else {
                return Err(SymbolBijectionError::MissingSource { position });
            };
            complete.push(target_position);
        }
        for (position, source_position) in target_sources.into_iter().enumerate() {
            if source_position.is_none() {
                return Err(SymbolBijectionError::MissingTarget { position });
            }
        }
        Self::from_ordinals(source, target, complete)
    }

    /// Constructs a cyclic relabeling over one alphabet's canonical order.
    pub fn cyclic(alphabet: A, steps: usize) -> Result<Self, SymbolBijectionError> {
        validate_alphabet(&alphabet)?;
        let cardinality = alphabet.symbols().len();
        let shift = steps % cardinality;
        let source_to_target = (0..cardinality)
            .map(|source| (source + shift) % cardinality)
            .collect();
        Self::from_ordinals(alphabet.clone(), alphabet, source_to_target)
    }

    fn from_ordinals(
        source: A,
        target: A,
        source_to_target: Vec<usize>,
    ) -> Result<Self, SymbolBijectionError> {
        validate_alphabet(&source)?;
        validate_alphabet(&target)?;
        if source.symbols().len() != target.symbols().len() {
            return Err(SymbolBijectionError::CardinalityMismatch {
                source_cardinality: source.symbols().len(),
                target_cardinality: target.symbols().len(),
            });
        }
        if source_to_target.len() != source.symbols().len() {
            return Err(SymbolBijectionError::CardinalityMismatch {
                source_cardinality: source.symbols().len(),
                target_cardinality: source_to_target.len(),
            });
        }
        Ok(Self {
            source,
            target,
            source_to_target: OrdinalMap::try_new(source_to_target)?,
        })
    }

    /// Returns the source alphabet.
    pub fn source(&self) -> &A {
        &self.source
    }

    /// Returns the target alphabet.
    pub fn target(&self) -> &A {
        &self.target
    }

    /// Returns the validated source-position to target-position bijection.
    pub fn source_to_target(&self) -> &[usize] {
        self.source_to_target.output_to_input()
    }

    /// Maps one source symbol to its target symbol.
    pub fn map_symbol(&self, symbol: &A::Symbol) -> Result<A::Symbol, SymbolBijectionError> {
        let Some(source_position) = self
            .source
            .symbols()
            .iter()
            .position(|candidate| candidate == symbol)
        else {
            return Err(SymbolBijectionError::ForeignSourceSymbol {
                alphabet_id: self.source.id().clone(),
            });
        };
        let Some(&target_position) = self.source_to_target().get(source_position) else {
            return Err(SymbolBijectionError::MissingSource {
                position: source_position,
            });
        };
        self.target.symbols().get(target_position).cloned().ok_or(
            SymbolBijectionError::MissingTarget {
                position: target_position,
            },
        )
    }

    /// Returns whether symbols and alphabet identity are unchanged.
    pub fn is_identity(&self) -> bool {
        self.source == self.target && self.source_to_target.is_identity()
    }

    /// Returns the exact inverse bijection.
    pub fn inverse(&self) -> Result<Self, SymbolBijectionError> {
        Self::from_ordinals(
            self.target.clone(),
            self.source.clone(),
            self.source_to_target.inverse()?.output_to_input().to_vec(),
        )
    }

    /// Composes `self` followed by `next`.
    pub fn compose(&self, next: &Self) -> Result<Self, SeriesTransformError> {
        if self.target != next.source {
            return Err(SeriesTransformError::CompositionAlphabetMismatch {
                first_target: self.target.id().clone(),
                second_source: next.source.id().clone(),
            });
        }
        let mut composed = Vec::with_capacity(self.source.symbols().len());
        for (source_position, &intermediate) in self.source_to_target().iter().enumerate() {
            let Some(&target_position) = next.source_to_target().get(intermediate) else {
                return Err(SymbolBijectionError::MissingSource {
                    position: source_position,
                }
                .into());
            };
            composed.push(target_position);
        }
        Ok(Self::from_ordinals(
            self.source.clone(),
            next.target.clone(),
            composed,
        )?)
    }

    /// Returns the deterministic identity-and-ordinal representation.
    pub fn canonical_form(&self) -> String {
        format!(
            "bijection/v1:{}->{}:{}",
            self.source.id(),
            self.target.id(),
            self.source_to_target.canonical_form()
        )
    }
}

/// A normalized positional and/or symbolic series transform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeriesTransform<A: SerialAlphabet> {
    order_map: Option<OrdinalMap>,
    relabeling: Option<SymbolBijection<A>>,
}

impl<A: SerialAlphabet> SeriesTransform<A> {
    /// Constructs an identity transform for a known series cardinality.
    pub fn identity(cardinality: usize) -> Self {
        Self::ordinal_permutation(OrdinalMap::identity(cardinality))
    }

    /// Constructs a retrograde transform for a known series cardinality.
    pub fn retrograde(cardinality: usize) -> Self {
        Self::ordinal_permutation(OrdinalMap::retrograde(cardinality))
    }

    /// Constructs a left position rotation reduced modulo the series cardinality.
    pub fn rotation(cardinality: usize, steps: usize) -> Self {
        Self::ordinal_permutation(OrdinalMap::rotation(cardinality, steps))
    }

    /// Constructs the order transform induced by an exhaustive block partition.
    pub fn block_partition(partition: BlockPartition) -> Self {
        Self::ordinal_permutation(partition.order_map().clone())
    }

    /// Constructs a transform from a prevalidated ordinal permutation.
    pub fn ordinal_permutation(order_map: OrdinalMap) -> Self {
        Self {
            order_map: Some(order_map),
            relabeling: None,
        }
    }

    /// Constructs a cyclic relabeling over an alphabet's canonical order.
    pub fn cyclic_relabeling(alphabet: A, steps: usize) -> Result<Self, SymbolBijectionError> {
        Ok(Self::bijection(SymbolBijection::cyclic(alphabet, steps)?))
    }

    /// Constructs a transform from a caller-supplied validated symbol bijection.
    pub fn bijection(relabeling: SymbolBijection<A>) -> Self {
        Self {
            order_map: None,
            relabeling: Some(relabeling),
        }
    }

    /// Returns the explicit order map, or `None` when positions are retained.
    pub fn order_map(&self) -> Option<&OrdinalMap> {
        self.order_map.as_ref()
    }

    /// Returns the symbolic relabeling, or `None` when symbols are retained.
    pub fn relabeling(&self) -> Option<&SymbolBijection<A>> {
        self.relabeling.as_ref()
    }

    /// Composes `self` followed by `next` into one normalized transform.
    pub fn compose(&self, next: &Self) -> Result<Self, SeriesTransformError> {
        let order_map = match (&self.order_map, &next.order_map) {
            (Some(first), Some(second)) => Some(first.compose(second)?),
            (Some(first), None) => Some(first.clone()),
            (None, Some(second)) => Some(second.clone()),
            (None, None) => None,
        };
        let relabeling = match (&self.relabeling, &next.relabeling) {
            (Some(first), Some(second)) => Some(first.compose(second)?),
            (Some(first), None) => Some(first.clone()),
            (None, Some(second)) => Some(second.clone()),
            (None, None) => None,
        };
        Ok(Self {
            order_map,
            relabeling,
        })
    }

    /// Returns the exact inverse transform.
    pub fn inverse(&self) -> Result<Self, SeriesTransformError> {
        Ok(Self {
            order_map: self
                .order_map
                .as_ref()
                .map(OrdinalMap::inverse)
                .transpose()?,
            relabeling: self
                .relabeling
                .as_ref()
                .map(SymbolBijection::inverse)
                .transpose()?,
        })
    }

    /// Returns a deterministic canonical representation of the normalized maps.
    pub fn canonical_form(&self) -> String {
        let order = self
            .order_map
            .as_ref()
            .map_or_else(|| "retain".to_owned(), OrdinalMap::canonical_form);
        let relabeling = self
            .relabeling
            .as_ref()
            .map_or_else(|| "retain".to_owned(), SymbolBijection::canonical_form);
        format!("series-transform/v1;order={order};symbols={relabeling}")
    }
}

impl<A: SerialAlphabet> Series<A> {
    /// Applies a validated transform and returns a valid series plus algebra evidence.
    pub fn apply(
        &self,
        operation: &SeriesTransform<A>,
    ) -> Result<TransformedSeries<A>, SeriesTransformError> {
        let order_map = operation
            .order_map
            .clone()
            .unwrap_or_else(|| OrdinalMap::identity(self.order().len()));
        let ordered = order_map.apply(self.order())?;

        let (target_alphabet, target_rule, target_order) =
            if let Some(relabeling) = &operation.relabeling {
                if self.alphabet() != relabeling.source() {
                    return Err(SeriesTransformError::SourceAlphabetMismatch {
                        expected: relabeling.source().id().clone(),
                        found: self.alphabet().id().clone(),
                    });
                }
                let mapped = ordered
                    .iter()
                    .map(|symbol| relabeling.map_symbol(symbol))
                    .collect::<Result<Vec<_>, _>>()?;
                let rule = remap_rule(self.rule(), self.alphabet(), relabeling)?;
                (relabeling.target().clone(), rule, mapped)
            } else {
                (self.alphabet().clone(), self.rule().clone(), ordered)
            };

        let series = Series::try_new(target_alphabet, target_rule, target_order)?;
        let mut relaxed_invariants = Vec::new();
        if !order_map.is_identity() {
            relaxed_invariants.push(RelaxedInvariant::SourceOrder);
        }
        if let Some(relabeling) = &operation.relabeling
            && !relabeling.is_identity()
        {
            relaxed_invariants.push(RelaxedInvariant::SymbolIdentity);
            if relabeling.source().id() != relabeling.target().id() {
                relaxed_invariants.push(RelaxedInvariant::AlphabetIdentity);
            }
        }
        relaxed_invariants.sort_unstable();

        let certificate = TransformCertificate {
            source_alphabet: self.alphabet().id().clone(),
            target_alphabet: series.alphabet().id().clone(),
            aggregate_preserved: true,
            order_map,
            inverse: Some(operation.inverse()?),
            relaxed_invariants,
        };
        Ok(TransformedSeries {
            series,
            certificate,
        })
    }
}

fn remap_rule<A: SerialAlphabet>(
    rule: &AggregateRule,
    source: &A,
    relabeling: &SymbolBijection<A>,
) -> Result<AggregateRule, SeriesTransformError> {
    let target = relabeling.target();
    match rule.kind() {
        AggregateRuleKind::ExhaustiveExactlyOnce => Ok(AggregateRule::exhaustive_exactly_once()),
        AggregateRuleKind::NoRepeat => Ok(AggregateRule::no_repeat()),
        AggregateRuleKind::FreeOrder => Ok(AggregateRule::free_order()),
        AggregateRuleKind::DeclaredMultiplicity => {
            let counts =
                rule.declared_counts(source)?
                    .ok_or(SeriesTransformError::RuleKindMismatch(
                        AggregateRuleKind::DeclaredMultiplicity,
                    ))?;
            let mapped = counts
                .into_iter()
                .map(|(symbol, count)| Ok((relabeling.map_symbol(&symbol)?, count)))
                .collect::<Result<Vec<_>, SeriesTransformError>>()?;
            Ok(AggregateRule::declared_multiplicity(target, mapped)?)
        }
        AggregateRuleKind::DeclaredOmissions => {
            let counts =
                rule.declared_counts(source)?
                    .ok_or(SeriesTransformError::RuleKindMismatch(
                        AggregateRuleKind::DeclaredOmissions,
                    ))?;
            let omitted = counts
                .into_iter()
                .filter(|(_, count)| *count == 0)
                .map(|(symbol, _)| relabeling.map_symbol(&symbol))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(AggregateRule::declared_omissions(target, omitted)?)
        }
        AggregateRuleKind::ProjectedAggregate => {
            let classes =
                rule.projected_classes(source)?
                    .ok_or(SeriesTransformError::RuleKindMismatch(
                        AggregateRuleKind::ProjectedAggregate,
                    ))?;
            let mapped = classes
                .into_iter()
                .map(|class| {
                    let symbols = class
                        .symbols
                        .iter()
                        .map(|symbol| relabeling.map_symbol(symbol))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(ProjectedClassSpec::new(
                        class.id,
                        symbols,
                        class.multiplicity,
                    ))
                })
                .collect::<Result<Vec<_>, SymbolBijectionError>>()?;
            Ok(AggregateRule::projected_aggregate(target, mapped)?)
        }
    }
}
