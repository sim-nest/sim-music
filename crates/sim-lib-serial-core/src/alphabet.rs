//! Stable alphabet identity and finite symbol collections.

use crate::AlphabetError;
use std::collections::BTreeMap;
use std::fmt::{Debug, Display, Formatter};

/// Stable, portable identity of a finite serial alphabet.
///
/// Valid ids contain ASCII letters, digits, `.`, `_`, `-`, or `/`, do not
/// begin or end with `/`, and do not contain empty path segments.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AlphabetId(String);

impl AlphabetId {
    /// Validates and constructs an alphabet id.
    pub fn try_new(value: impl Into<String>) -> Result<Self, AlphabetError> {
        let value = value.into();
        validate_stable_id(&value).map_err(|reason| AlphabetError::InvalidId {
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

impl Display for AlphabetId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, formatter)
    }
}

/// A finite, ordered vocabulary used by a [`crate::Series`].
pub trait SerialAlphabet: Clone + Eq + Debug {
    /// Symbol value retained in a series order.
    type Symbol: Clone + Eq + Ord + Debug;

    /// Stable alphabet identity.
    fn id(&self) -> &AlphabetId;

    /// Symbols in canonical alphabet order.
    fn symbols(&self) -> &[Self::Symbol];
}

/// Reusable owned implementation of [`SerialAlphabet`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiniteAlphabet<S>
where
    S: Clone + Eq + Ord + Debug,
{
    id: AlphabetId,
    symbols: Vec<S>,
}

impl<S> FiniteAlphabet<S>
where
    S: Clone + Eq + Ord + Debug,
{
    /// Constructs a non-empty alphabet with unique canonical symbols.
    pub fn try_new(id: AlphabetId, symbols: Vec<S>) -> Result<Self, AlphabetError> {
        validate_symbols(&id, &symbols)?;
        Ok(Self { id, symbols })
    }

    /// Returns the canonical position of `symbol`, when it belongs to this alphabet.
    pub fn position(&self, symbol: &S) -> Option<usize> {
        self.symbols
            .iter()
            .position(|candidate| candidate == symbol)
    }
}

impl<S> SerialAlphabet for FiniteAlphabet<S>
where
    S: Clone + Eq + Ord + Debug,
{
    type Symbol = S;

    fn id(&self) -> &AlphabetId {
        &self.id
    }

    fn symbols(&self) -> &[Self::Symbol] {
        &self.symbols
    }
}

/// A same-type alphabet registry that rejects duplicate stable identities.
///
/// The registry is deliberately a value-layer helper, not a global singleton.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlphabetRegistry<A: SerialAlphabet> {
    alphabets: BTreeMap<AlphabetId, A>,
}

impl<A: SerialAlphabet> Default for AlphabetRegistry<A> {
    fn default() -> Self {
        Self {
            alphabets: BTreeMap::new(),
        }
    }
}

impl<A: SerialAlphabet> AlphabetRegistry<A> {
    /// Constructs an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Validates and inserts an alphabet, rejecting an id already in the registry.
    pub fn insert(&mut self, alphabet: A) -> Result<(), AlphabetError> {
        validate_alphabet(&alphabet)?;
        if self.alphabets.contains_key(alphabet.id()) {
            return Err(AlphabetError::DuplicateId(alphabet.id().clone()));
        }
        self.alphabets.insert(alphabet.id().clone(), alphabet);
        Ok(())
    }

    /// Looks up an alphabet by stable identity.
    pub fn get(&self, id: &AlphabetId) -> Option<&A> {
        self.alphabets.get(id)
    }

    /// Returns the number of registered alphabets.
    pub fn len(&self) -> usize {
        self.alphabets.len()
    }

    /// Returns whether no alphabets are registered.
    pub fn is_empty(&self) -> bool {
        self.alphabets.is_empty()
    }
}

pub(crate) fn validate_alphabet<A: SerialAlphabet>(
    alphabet: &A,
) -> Result<BTreeMap<A::Symbol, usize>, AlphabetError> {
    validate_symbols(alphabet.id(), alphabet.symbols())
}

fn validate_symbols<S>(id: &AlphabetId, symbols: &[S]) -> Result<BTreeMap<S, usize>, AlphabetError>
where
    S: Clone + Eq + Ord + Debug,
{
    if symbols.is_empty() {
        return Err(AlphabetError::Empty { id: id.clone() });
    }
    let mut positions = BTreeMap::new();
    for (position, symbol) in symbols.iter().cloned().enumerate() {
        if let Some(first) = positions.insert(symbol, position) {
            return Err(AlphabetError::DuplicateSymbol {
                id: id.clone(),
                first,
                duplicate: position,
            });
        }
    }
    Ok(positions)
}

pub(crate) fn validate_stable_id(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("id must not be empty");
    }
    if value.starts_with('/') || value.ends_with('/') || value.contains("//") {
        return Err("id must use non-empty path segments");
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'))
    {
        return Err("id contains a non-portable character");
    }
    Ok(())
}
