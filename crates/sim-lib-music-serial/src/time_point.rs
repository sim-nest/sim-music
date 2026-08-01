//! Exact time-point systems with onset-class order kept separate from duration series.

use std::num::NonZeroU16;

use sim_lib_music_core::Time;
use sim_lib_serial_core::{
    AggregateRule, AlphabetError, AlphabetId, FiniteAlphabet, SerialAlphabet, Series, SeriesError,
};
use thiserror::Error;

use crate::rotate_sequence_left;

/// Finite onset-class alphabet for one exact time-point system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimePointAlphabet {
    modulus: NonZeroU16,
    inner: FiniteAlphabet<u16>,
}

impl TimePointAlphabet {
    /// Constructs the canonical onset-class alphabet `0..modulus`.
    pub fn try_new(modulus: NonZeroU16) -> Result<Self, TimePointError> {
        let symbols = (0..modulus.get()).collect::<Vec<_>>();
        let inner = FiniteAlphabet::try_new(
            AlphabetId::try_new(format!("time-point/{}-v1", modulus.get()))?,
            symbols,
        )?;
        Ok(Self { modulus, inner })
    }

    /// Returns the system modulus retained by this alphabet.
    pub const fn modulus(&self) -> NonZeroU16 {
        self.modulus
    }

    /// Returns the canonical onset classes in system order.
    pub fn classes(&self) -> &[u16] {
        self.inner.symbols()
    }
}

impl SerialAlphabet for TimePointAlphabet {
    type Symbol = u16;

    fn id(&self) -> &AlphabetId {
        self.inner.id()
    }

    fn symbols(&self) -> &[Self::Symbol] {
        self.inner.symbols()
    }
}

/// Exact time-point system with one finite modulus and one unit duration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimePointSystem {
    /// Finite onset-class modulus.
    pub modulus: NonZeroU16,
    /// Exact unit mapped to one onset-class step.
    pub unit: Time,
}

impl TimePointSystem {
    /// Returns the canonical onset-class alphabet for this system.
    pub fn alphabet(&self) -> Result<TimePointAlphabet, TimePointError> {
        TimePointAlphabet::try_new(self.modulus)
    }

    /// Converts one onset class to an exact onset measured in this system's unit.
    pub fn onset_for(&self, point: u16) -> Option<Time> {
        (point < self.modulus.get()).then(|| self.unit * i64::from(point))
    }
}

/// One ordered onset-class series over a [`TimePointSystem`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimePointRow {
    /// Validated onset-class order over the system alphabet.
    pub points: Series<TimePointAlphabet>,
}

impl TimePointRow {
    /// Constructs an exhaustive onset-class row for `system`.
    pub fn try_new(system: &TimePointSystem, points: Vec<u16>) -> Result<Self, TimePointError> {
        Ok(Self {
            points: Series::try_new(
                system.alphabet()?,
                AggregateRule::exhaustive_exactly_once(),
                points,
            )?,
        })
    }

    /// Returns the system modulus inferred from the retained alphabet.
    pub fn modulus(&self) -> NonZeroU16 {
        self.points.alphabet().modulus()
    }

    /// Returns the retained onset classes in serial order.
    pub fn order(&self) -> &[u16] {
        self.points.order()
    }

    /// Returns exact onset positions under `system`, without implying any durations.
    pub fn onsets(&self, system: &TimePointSystem) -> Result<Vec<Time>, TimePointError> {
        if system.modulus != self.modulus() {
            return Err(TimePointError::SystemMismatch {
                expected: self.modulus(),
                found: system.modulus,
            });
        }
        self.order()
            .iter()
            .map(|&point| {
                system
                    .onset_for(point)
                    .ok_or(TimePointError::PointOutsideSystem {
                        point,
                        modulus: system.modulus,
                    })
            })
            .collect()
    }

    /// Returns a left-rotated onset-class row reduced modulo the system modulus.
    pub fn rotate(&self, steps: usize) -> Result<Self, TimePointError> {
        Ok(Self {
            points: Series::try_new(
                self.points.alphabet().clone(),
                self.points.rule().clone(),
                rotate_sequence_left(self.points.order(), steps),
            )?,
        })
    }
}

/// Failure while constructing or realizing exact time-point data.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum TimePointError {
    /// The canonical onset-class alphabet was invalid.
    #[error(transparent)]
    Alphabet(#[from] AlphabetError),
    /// The caller-supplied onset-class order violated the system aggregate contract.
    #[error(transparent)]
    Series(#[from] SeriesError),
    /// The caller tried to realize a row under another modulus.
    #[error("time-point row expects modulus {expected}, got {found}")]
    SystemMismatch {
        /// Row modulus retained by the row alphabet.
        expected: NonZeroU16,
        /// Realization modulus supplied by the caller.
        found: NonZeroU16,
    },
    /// One onset class lay outside the selected finite system.
    #[error("time-point class {point} lies outside modulus {modulus}")]
    PointOutsideSystem {
        /// Rejected onset class.
        point: u16,
        /// Finite system modulus.
        modulus: NonZeroU16,
    },
}
