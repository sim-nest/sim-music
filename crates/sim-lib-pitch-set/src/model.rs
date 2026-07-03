use thiserror::Error;

use sim_lib_pitch_core::{Pitch, PitchClass};

/// Error returned when a pitch-set value cannot be constructed, encoded, or decoded.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PitchSetError {
    /// A MIDI key outside the valid `0..128` range was supplied.
    #[error("invalid MIDI key {0}")]
    InvalidMidiKey(u8),
    /// A third-stack bit pattern could not be decoded into a valid signature.
    #[error("invalid third stack encoding")]
    InvalidThirdStackEncoding,
    /// A third-stack signature violated the run-length constraints on its steps.
    #[error("invalid third stack signature")]
    InvalidThirdStack,
}

/// A bitmask over the twelve pitch classes, with bit `n` set when pitch class `n`
/// is present.
///
/// The low twelve bits of the inner `u16` represent pitch classes C through B.
///
/// # Examples
///
/// ```
/// use sim_lib_pitch_core::PitchClass;
/// use sim_lib_pitch_set::PitchClassMask;
///
/// let triad = PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::E, PitchClass::G]);
/// assert_eq!(triad.count_bits(), 3);
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct PitchClassMask(pub u16);

impl PitchClassMask {
    /// Builds a mask from a slice of pitch classes; duplicates collapse to one bit.
    pub fn from_pitch_classes(pitch_classes: &[PitchClass]) -> Self {
        let mut bits = 0u16;
        for pitch_class in pitch_classes {
            bits |= 1u16 << pitch_class.0;
        }
        Self(bits)
    }

    /// Returns the set pitch classes in ascending order.
    pub fn pitch_classes(self) -> Vec<PitchClass> {
        (0..12)
            .filter(|bit| self.0 & (1u16 << bit) != 0)
            .map(|bit| PitchClass(bit as u8))
            .collect()
    }

    /// Returns this mask transposed by `semitones`, wrapping within the octave.
    pub fn rotate(self, semitones: i32) -> Self {
        let shift = semitones.rem_euclid(12) as u32;
        let bits = self.0 & 0x0fff;
        Self(((bits << shift) | (bits >> (12 - shift))) & 0x0fff)
    }

    /// Returns this mask inverted about `axis`.
    pub fn invert(self, axis: PitchClass) -> Self {
        let mut out = 0u16;
        for pitch_class in self.pitch_classes() {
            out |= 1u16 << pitch_class.invert(axis).0;
        }
        Self(out)
    }

    /// Returns the rotation of this mask with the smallest numeric value, a
    /// transposition-invariant normal form.
    pub fn normalize(self) -> Self {
        (0..12)
            .map(|shift| self.rotate(-shift))
            .min_by_key(|mask| mask.0)
            .unwrap_or(self)
    }

    /// Returns the number of pitch classes in the set (the population count).
    pub fn count_bits(self) -> u32 {
        self.0.count_ones()
    }

    /// Returns the [`IntervalVector`] tallying interval classes among the set's
    /// pitch classes.
    pub fn interval_vector(self) -> IntervalVector {
        let pitch_classes = self.pitch_classes();
        let mut bins = [0u16; 6];
        for (index, a) in pitch_classes.iter().enumerate() {
            for b in pitch_classes.iter().skip(index + 1) {
                let class = a.interval_class(*b);
                if class > 0 {
                    bins[(class - 1) as usize] += 1;
                }
            }
        }
        IntervalVector(bins)
    }
}

/// A bitmask over the 128 MIDI keys, with bit `n` set when MIDI key `n` is present.
///
/// Unlike [`PitchClassMask`], this preserves octave, so it represents a concrete
/// set of sounding pitches rather than pitch classes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct PitchRangeMask {
    /// The packed key bits, with bit `n` corresponding to MIDI key `n`.
    pub bits: u128,
}

impl PitchRangeMask {
    /// Adds `midi_key` to the set.
    pub fn set(&mut self, midi_key: u8) {
        self.bits |= 1u128 << midi_key;
    }

    /// Removes `midi_key` from the set.
    pub fn clear(&mut self, midi_key: u8) {
        self.bits &= !(1u128 << midi_key);
    }

    /// Returns `true` if `midi_key` is present in the set.
    pub fn contains(self, midi_key: u8) -> bool {
        self.bits & (1u128 << midi_key) != 0
    }

    /// Returns the union of this mask with `other`.
    pub fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    /// Returns the intersection of this mask with `other`.
    pub fn intersection(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }

    /// Returns the keys present in this mask but not in `other`.
    pub fn difference(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }

    /// Returns the set keys as concrete [`Pitch`] values in ascending order.
    pub fn to_pitches(self) -> Vec<Pitch> {
        (0..128u8)
            .filter(|key| self.contains(*key))
            .map(Pitch::from_midi)
            .collect()
    }
}

/// An interval-class vector: counts of each of the six interval classes (1..=6)
/// occurring among a set's pitch classes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntervalVector(pub [u16; 6]);

/// A chord represented as a [`PitchClassMask`] with an optional designated root.
///
/// When `root` is `None` the chord is rootless and can be reduced to its
/// transposition-invariant normal form via [`BitChord::canonical`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BitChord {
    /// The pitch classes that make up the chord.
    pub mask: PitchClassMask,
    /// The chord root, or `None` for a rootless chord.
    pub root: Option<PitchClass>,
}

impl BitChord {
    /// Returns a canonical form: rooted chords are returned unchanged, while
    /// rootless chords are normalized to their lowest-valued rotation.
    pub fn canonical(self) -> Self {
        if self.root.is_some() {
            self
        } else {
            Self {
                mask: self.mask.normalize(),
                root: None,
            }
        }
    }
}

/// A single step in a stack of thirds: a minor third (3 semitones) or major third
/// (4 semitones).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ThirdStep {
    /// A minor third (3 semitones).
    Minor,
    /// A major third (4 semitones).
    Major,
}

/// A chord described as a root pitch class plus an ordered stack of third [`ThirdStep`]s.
///
/// This tertian encoding captures triads, sevenths, and extended chords as a
/// sequence of stacked thirds, with run-length limits that reject implausible
/// stacks.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ThirdStackSignature {
    /// The pitch class at the bottom of the stack.
    pub root: PitchClass,
    /// The ordered thirds stacked above the root.
    pub steps: Vec<ThirdStep>,
    /// A guard bit reserved by the bit encoding to mark the end of the step run.
    pub guard: bool,
}

impl ThirdStackSignature {
    /// Validates the step run, rejecting four consecutive minor thirds or three
    /// consecutive major thirds.
    pub fn validate(&self) -> Result<(), PitchSetError> {
        let mut minor_run = 0usize;
        let mut major_run = 0usize;
        for step in &self.steps {
            match step {
                ThirdStep::Minor => {
                    minor_run += 1;
                    major_run = 0;
                }
                ThirdStep::Major => {
                    major_run += 1;
                    minor_run = 0;
                }
            }
            if minor_run >= 4 || major_run >= 3 {
                return Err(PitchSetError::InvalidThirdStack);
            }
        }
        Ok(())
    }

    /// Encodes this signature into a compact `u32`, validating it first.
    pub fn encode(&self) -> Result<u32, PitchSetError> {
        self.validate()?;
        let mut encoded = self.root.0 as u32;
        for (index, step) in self.steps.iter().enumerate() {
            let bit = matches!(step, ThirdStep::Major) as u32;
            encoded |= bit << (4 + index);
        }
        if self.guard {
            encoded |= 1u32 << (4 + self.steps.len());
        }
        Ok(encoded)
    }

    /// Decodes a `u32` produced by [`ThirdStackSignature::encode`] back into a
    /// validated signature.
    pub fn decode(encoded: u32) -> Result<Self, PitchSetError> {
        let root = PitchClass((encoded & 0x0f) as u8);
        let mut steps = Vec::new();
        let mut index = 4u32;
        let mut guard = false;
        while index < 31 {
            let bit = (encoded >> index) & 1;
            if ((encoded >> (index + 1)) & 1) == 0 && bit == 1 && index > 4 {
                guard = true;
                break;
            }
            steps.push(if bit == 0 {
                ThirdStep::Minor
            } else {
                ThirdStep::Major
            });
            index += 1;
            if steps.len() >= 8 {
                break;
            }
        }
        let signature = Self { root, steps, guard };
        signature.validate()?;
        Ok(signature)
    }

    /// Returns a single-character family tag classifying the stack by its count of
    /// major thirds.
    pub fn family_tag(&self) -> char {
        let majors = self
            .steps
            .iter()
            .filter(|step| matches!(step, ThirdStep::Major))
            .count();
        match majors {
            0..=2 => 'w',
            3 => 'x',
            4 => 'y',
            _ => 'z',
        }
    }

    /// Realizes the stacked thirds into the [`PitchClassMask`] of the chord's
    /// pitch classes.
    pub fn to_mask(&self) -> PitchClassMask {
        let mut pitch_classes = vec![self.root];
        let mut current = self.root;
        for step in &self.steps {
            current = current.transpose(match step {
                ThirdStep::Minor => 3,
                ThirdStep::Major => 4,
            });
            pitch_classes.push(current);
        }
        PitchClassMask::from_pitch_classes(&pitch_classes)
    }
}
