//! Generic typed parameter alphabets and series for integral serialism.

use std::fmt::Debug;

use sim_lib_serial_core::{
    AggregateLedger, AggregateRule, AlphabetError, AlphabetId, FiniteAlphabet, SerialAlphabet,
    Series, SeriesError, SeriesTransform, SeriesTransformError,
};
use thiserror::Error;

/// Value bound accepted by generic serial parameter tracks.
pub trait ParameterValue: Clone + Eq + Ord + Debug + 'static {}

impl<T> ParameterValue for T where T: Clone + Eq + Ord + Debug + 'static {}

/// Generic finite alphabet declared by one parameter owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterAlphabet<T: ParameterValue> {
    inner: FiniteAlphabet<T>,
}

impl<T: ParameterValue> ParameterAlphabet<T> {
    /// Constructs a finite parameter alphabet with a stable caller-owned id.
    pub fn try_new(id: impl Into<String>, symbols: Vec<T>) -> Result<Self, ParameterError> {
        Ok(Self {
            inner: FiniteAlphabet::try_new(AlphabetId::try_new(id.into())?, symbols)?,
        })
    }

    /// Returns the stable identity of this alphabet.
    pub fn id(&self) -> &AlphabetId {
        self.inner.id()
    }

    /// Returns the canonical symbol ladder retained by this alphabet.
    pub fn symbols(&self) -> &[T] {
        self.inner.symbols()
    }
}

impl<T: ParameterValue> SerialAlphabet for ParameterAlphabet<T> {
    type Symbol = T;

    fn id(&self) -> &AlphabetId {
        self.inner.id()
    }

    fn symbols(&self) -> &[Self::Symbol] {
        self.inner.symbols()
    }
}

/// Thin typed wrapper over an unchanged serial-core [`Series`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterSeries<T: ParameterValue> {
    inner: Series<ParameterAlphabet<T>>,
}

impl<T: ParameterValue> ParameterSeries<T> {
    /// Constructs one exhaustive exactly-once parameter series.
    pub fn try_new(id: impl Into<String>, order: Vec<T>) -> Result<Self, ParameterError> {
        let id = id.into();
        Self::try_new_with_rule(id.clone(), AggregateRule::exhaustive_exactly_once(), order)
    }

    /// Constructs one parameter series under an explicit aggregate rule.
    pub fn try_new_with_rule(
        id: impl Into<String>,
        rule: AggregateRule,
        order: Vec<T>,
    ) -> Result<Self, ParameterError> {
        let alphabet = ParameterAlphabet::try_new(id, order.clone())?;
        Ok(Self {
            inner: Series::try_new(alphabet, rule, order)?,
        })
    }

    /// Returns the retained generic parameter alphabet.
    pub fn alphabet(&self) -> &ParameterAlphabet<T> {
        self.inner.alphabet()
    }

    /// Returns the unchanged aggregate rule carried by the series.
    pub fn rule(&self) -> &AggregateRule {
        self.inner.rule()
    }

    /// Returns the ordered parameter values.
    pub fn order(&self) -> &[T] {
        self.inner.order()
    }

    /// Returns the source aggregate ledger retained by serial-core.
    pub fn ledger(&self) -> &AggregateLedger<T> {
        self.inner.ledger()
    }

    /// Applies one serial-core transform and returns a new typed parameter series.
    pub fn apply(
        &self,
        transform: &SeriesTransform<ParameterAlphabet<T>>,
    ) -> Result<Self, ParameterError> {
        Ok(Self {
            inner: self.inner.apply(transform)?.series,
        })
    }
}

/// Failure while constructing or transforming a typed parameter series.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ParameterError {
    /// The parameter alphabet id or symbol ladder was invalid.
    #[error(transparent)]
    Alphabet(#[from] AlphabetError),
    /// The supplied order violated the declared aggregate rule.
    #[error(transparent)]
    Series(#[from] SeriesError),
    /// The requested transform was invalid for this parameter series.
    #[error(transparent)]
    Transform(#[from] SeriesTransformError),
}
