use std::collections::BTreeSet;

use sim_lib_music_core::{Music, MusicObject, Time, TimedNote};
use sim_lib_pitch_core::Pitch;
use sim_lib_pitch_scale::Scale;
use thiserror::Error;

use crate::{TransformError, canonical_roll, to_piano_roll};

mod ops;
mod rng;
mod wire;

use ops::{apply_op, restore_locks};
use rng::PatternRng;
use wire::{op_wire, parse_number, parse_op};

/// Error raised while parsing or validating a pattern mutator.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PatternMutatorError {
    /// The wire string was not a valid pattern mutator encoding.
    #[error("invalid pattern mutator wire format")]
    InvalidWire,
    /// A numeric field could not be parsed.
    #[error("invalid pattern mutator number")]
    InvalidNumber,
    /// A scale mode name was not recognized.
    #[error("invalid pattern mutator mode: {0}")]
    InvalidMode(String),
    /// A pitch class value was out of range.
    #[error("invalid pattern mutator pitch class: {0}")]
    InvalidPitchClass(u8),
}

/// Set of source note indices held fixed (locked) during mutation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PatternLockSet {
    note_indices: BTreeSet<usize>,
}

impl PatternLockSet {
    /// Builds a lock set from a collection of source note indices.
    pub fn from_note_indices(indices: impl IntoIterator<Item = usize>) -> Self {
        Self {
            note_indices: indices.into_iter().collect(),
        }
    }

    /// Returns whether the given source index is locked.
    pub fn contains(&self, index: usize) -> bool {
        self.note_indices.contains(&index)
    }

    /// Returns the set of locked source note indices.
    pub fn note_indices(&self) -> &BTreeSet<usize> {
        &self.note_indices
    }
}

/// A single mutation operation applied to a pattern's notes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MutationOp {
    /// Reverse note onsets within the pattern span.
    Reverse,
    /// Rotate notes across their distinct onset slots by `steps`.
    Rotate {
        /// Number of slots to rotate (signed).
        steps: i32,
    },
    /// Transpose unlocked notes by `semitones`.
    Transpose {
        /// Semitone offset.
        semitones: i32,
    },
    /// Invert unlocked notes about `axis`.
    Invert {
        /// Inversion axis pitch.
        axis: Pitch,
    },
    /// Shuffle note onsets within each beat-sized bucket.
    ShuffleWithinBeat {
        /// Bucket width in beats.
        beat: Time,
    },
    /// Randomly drop notes, keeping roughly `keep_percent` of them.
    Thin {
        /// Target percentage of notes to keep.
        keep_percent: u8,
    },
    /// Duplicate notes transposed by `semitones` to thicken the texture.
    Thicken {
        /// Semitone offset of the added copies.
        semitones: i32,
    },
    /// Remap velocities into the `[low, high]` range.
    VelocityRemap {
        /// Lower velocity bound.
        low: u8,
        /// Upper velocity bound.
        high: u8,
    },
    /// Displace note onsets forward or backward by `offset`.
    RhythmDisplace {
        /// Displacement magnitude.
        offset: Time,
    },
    /// Conform note pitches to the nearest tone of `scale`.
    ScaleConform {
        /// Scale notes are conformed to.
        scale: Scale,
    },
}

/// Configuration describing a sequence of pattern mutations and their controls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternMutatorConfig {
    /// Operations applied in order.
    pub operations: Vec<MutationOp>,
    /// Strength of each operation, from 0 to 100.
    pub amount: u8,
    /// Seed for the deterministic pseudo-random generator.
    pub seed: u64,
    /// Source notes held fixed across all operations.
    pub locks: PatternLockSet,
}

impl PatternMutatorConfig {
    /// Builds a config from operations with default amount, seed, and locks.
    pub fn new(operations: Vec<MutationOp>) -> Self {
        Self {
            operations,
            amount: 100,
            seed: 0,
            locks: PatternLockSet::default(),
        }
    }

    /// Sets the mutation strength, clamped to at most 100.
    pub fn with_amount(mut self, amount: u8) -> Self {
        self.amount = amount.min(100);
        self
    }

    /// Sets the random seed.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Sets the locked note set.
    pub fn with_locks(mut self, locks: PatternLockSet) -> Self {
        self.locks = locks;
        self
    }

    /// Applies the configured mutations to the input and returns the result.
    pub fn apply(&self, object: &dyn MusicObject) -> Result<Music, TransformError> {
        mutate_pattern(object, self)
    }

    /// Serializes this config to its `pattern-mutator|...` wire string.
    pub fn to_wire(&self) -> String {
        let locks = self
            .locks
            .note_indices()
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let ops = self
            .operations
            .iter()
            .map(op_wire)
            .collect::<Vec<_>>()
            .join(";");
        format!(
            "pattern-mutator|amount={}|seed={}|locks={}|ops={}",
            self.amount, self.seed, locks, ops
        )
    }

    /// Parses a config from its wire string, validating each field.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_music_transform::{MutationOp, PatternMutatorConfig};
    ///
    /// let config = PatternMutatorConfig::new(vec![MutationOp::Reverse]).with_amount(80);
    /// let wire = config.to_wire();
    /// assert_eq!(PatternMutatorConfig::from_wire(&wire), Ok(config));
    /// ```
    pub fn from_wire(value: &str) -> Result<Self, PatternMutatorError> {
        let Some(rest) = value.strip_prefix("pattern-mutator|") else {
            return Err(PatternMutatorError::InvalidWire);
        };
        let mut amount = 100;
        let mut seed = 0;
        let mut locks = PatternLockSet::default();
        let mut operations = Vec::new();

        for part in rest.split('|') {
            let (key, value) = part
                .split_once('=')
                .ok_or(PatternMutatorError::InvalidWire)?;
            match key {
                "amount" => amount = parse_number::<u8>(value)?.min(100),
                "seed" => seed = parse_number(value)?,
                "locks" if value.is_empty() => locks = PatternLockSet::default(),
                "locks" => {
                    locks = PatternLockSet::from_note_indices(
                        value
                            .split(',')
                            .map(parse_number)
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                }
                "ops" if value.is_empty() => operations = Vec::new(),
                "ops" => {
                    operations = value
                        .split(';')
                        .map(parse_op)
                        .collect::<Result<Vec<_>, _>>()?
                }
                _ => return Err(PatternMutatorError::InvalidWire),
            }
        }

        Ok(Self {
            operations,
            amount,
            seed,
            locks,
        })
    }
}

/// Applies a [`PatternMutatorConfig`] to material and returns the mutated music.
pub fn mutate_pattern(
    object: &dyn MusicObject,
    config: &PatternMutatorConfig,
) -> Result<Music, TransformError> {
    let original = to_piano_roll(object)?
        .items
        .into_iter()
        .enumerate()
        .map(|(source_index, item)| PatternNote { source_index, item })
        .collect::<Vec<_>>();
    let mut notes = original.clone();
    let mut rng = PatternRng::new(config.seed);
    let mut next_source_index = original.len();

    for op in &config.operations {
        apply_op(
            &mut notes,
            op,
            config.amount,
            &config.locks,
            &mut rng,
            &mut next_source_index,
        )?;
        restore_locks(&mut notes, &original, &config.locks);
    }

    Ok(Music::PianoRoll(canonical_roll(
        notes.into_iter().map(|note| note.item).collect(),
    )?))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PatternNote {
    source_index: usize,
    item: TimedNote,
}
