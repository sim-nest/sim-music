//! The canonical chromatic pitch-class alphabet.

use sim_lib_pitch_core::PitchClass;
use sim_lib_serial_core::{AlphabetError, AlphabetId, FiniteAlphabet, SerialAlphabet};

const CANONICAL_CLASSES: [PitchClass; 12] = [
    PitchClass::C,
    PitchClass::CS,
    PitchClass::D,
    PitchClass::DS,
    PitchClass::E,
    PitchClass::F,
    PitchClass::FS,
    PitchClass::G,
    PitchClass::GS,
    PitchClass::A,
    PitchClass::AS,
    PitchClass::B,
];

/// The stable twelve-symbol alphabet of canonical [`PitchClass`] values.
///
/// Construction delegates uniqueness and stable-id validation to
/// [`FiniteAlphabet`]. The order is the canonical numeric order `C = 0` through
/// `B = 11`; no spelling aliases or private pitch representation are introduced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchClassAlphabet {
    inner: FiniteAlphabet<PitchClass>,
}

impl PitchClassAlphabet {
    /// Constructs the canonical pitch-class alphabet.
    pub fn try_new() -> Result<Self, AlphabetError> {
        Ok(Self {
            inner: FiniteAlphabet::try_new(
                AlphabetId::try_new("pitch-class/12tet-v1")?,
                CANONICAL_CLASSES.to_vec(),
            )?,
        })
    }

    /// Returns the stable alphabet identity.
    pub fn id(&self) -> &AlphabetId {
        self.inner.id()
    }

    /// Returns all twelve canonical classes in numeric order.
    pub fn classes(&self) -> &[PitchClass] {
        self.inner.symbols()
    }
}

impl SerialAlphabet for PitchClassAlphabet {
    type Symbol = PitchClass;

    fn id(&self) -> &AlphabetId {
        self.inner.id()
    }

    fn symbols(&self) -> &[Self::Symbol] {
        self.inner.symbols()
    }
}
