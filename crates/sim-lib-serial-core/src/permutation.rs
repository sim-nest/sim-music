//! Validated finite ordinal maps and ordered block partitions.

use crate::{BlockPartitionError, OrdinalMapError};

/// A complete bijection from output positions to input positions.
///
/// Entry `i` names the input position copied into output position `i`. The
/// constructor validates the complete finite map before it can be applied.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrdinalMap {
    output_to_input: Vec<usize>,
}

impl OrdinalMap {
    /// Validates a complete bijection over `0..output_to_input.len()`.
    pub fn try_new(output_to_input: Vec<usize>) -> Result<Self, OrdinalMapError> {
        let cardinality = output_to_input.len();
        let mut first_outputs = vec![None; cardinality];
        for (output, &input) in output_to_input.iter().enumerate() {
            if input >= cardinality {
                return Err(OrdinalMapError::OutOfRange {
                    output,
                    input,
                    cardinality,
                });
            }
            if let Some(first_output) = first_outputs[input].replace(output) {
                return Err(OrdinalMapError::DuplicateInput {
                    input,
                    first_output,
                    duplicate_output: output,
                });
            }
        }
        Ok(Self { output_to_input })
    }

    /// Constructs the identity map of the requested cardinality.
    pub fn identity(cardinality: usize) -> Self {
        Self {
            output_to_input: (0..cardinality).collect(),
        }
    }

    /// Constructs a retrograde map of the requested cardinality.
    pub fn retrograde(cardinality: usize) -> Self {
        Self {
            output_to_input: (0..cardinality).rev().collect(),
        }
    }

    /// Constructs a left rotation by `steps`, reduced modulo the cardinality.
    pub fn rotation(cardinality: usize, steps: usize) -> Self {
        if cardinality == 0 {
            return Self::identity(0);
        }
        let shift = steps % cardinality;
        Self {
            output_to_input: (0..cardinality)
                .map(|output| (output + shift) % cardinality)
                .collect(),
        }
    }

    /// Returns the finite domain cardinality.
    pub fn cardinality(&self) -> usize {
        self.output_to_input.len()
    }

    /// Returns the validated output-to-input ordinal map.
    pub fn output_to_input(&self) -> &[usize] {
        &self.output_to_input
    }

    /// Returns whether this map preserves every position.
    pub fn is_identity(&self) -> bool {
        self.output_to_input
            .iter()
            .enumerate()
            .all(|(position, &input)| position == input)
    }

    /// Applies this map to a slice after checking its cardinality.
    pub fn apply<T: Clone>(&self, source: &[T]) -> Result<Vec<T>, OrdinalMapError> {
        if source.len() != self.cardinality() {
            return Err(OrdinalMapError::CardinalityMismatch {
                expected: self.cardinality(),
                found: source.len(),
            });
        }
        self.output_to_input
            .iter()
            .enumerate()
            .map(|(output, &input)| {
                source
                    .get(input)
                    .cloned()
                    .ok_or(OrdinalMapError::OutOfRange {
                        output,
                        input,
                        cardinality: source.len(),
                    })
            })
            .collect()
    }

    /// Returns the exact inverse map.
    pub fn inverse(&self) -> Result<Self, OrdinalMapError> {
        let mut inverse = vec![0; self.cardinality()];
        for (output, &input) in self.output_to_input.iter().enumerate() {
            let Some(slot) = inverse.get_mut(input) else {
                return Err(OrdinalMapError::OutOfRange {
                    output,
                    input,
                    cardinality: self.cardinality(),
                });
            };
            *slot = output;
        }
        Self::try_new(inverse)
    }

    /// Composes `self` followed by `next` into one canonical map.
    pub fn compose(&self, next: &Self) -> Result<Self, OrdinalMapError> {
        if self.cardinality() != next.cardinality() {
            return Err(OrdinalMapError::CompositionCardinalityMismatch {
                first: self.cardinality(),
                second: next.cardinality(),
            });
        }
        let mut composed = Vec::with_capacity(self.cardinality());
        for (output, &intermediate) in next.output_to_input.iter().enumerate() {
            let Some(&input) = self.output_to_input.get(intermediate) else {
                return Err(OrdinalMapError::OutOfRange {
                    output,
                    input: intermediate,
                    cardinality: self.cardinality(),
                });
            };
            composed.push(input);
        }
        Self::try_new(composed)
    }

    /// Returns the deterministic canonical ordinal representation.
    pub fn canonical_form(&self) -> String {
        let ordinals = self
            .output_to_input
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!("ordinal-map/v1:[{ordinals}]")
    }
}

/// Compatibility name emphasizing that an [`OrdinalMap`] is a permutation.
pub type OrdinalPermutation = OrdinalMap;

/// An ordered, exhaustive partition of source positions into non-empty blocks.
///
/// Applying a partition concatenates its blocks in declaration order. This is
/// a validated structural spelling of an ordinal permutation, not a search or
/// partition enumerator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockPartition {
    cardinality: usize,
    blocks: Vec<Vec<usize>>,
    order_map: OrdinalMap,
}

impl BlockPartition {
    /// Validates non-empty blocks that cover every source position exactly once.
    pub fn try_new(
        cardinality: usize,
        blocks: Vec<Vec<usize>>,
    ) -> Result<Self, BlockPartitionError> {
        let mut flattened = Vec::with_capacity(cardinality);
        for (block, positions) in blocks.iter().enumerate() {
            if positions.is_empty() {
                return Err(BlockPartitionError::EmptyBlock { block });
            }
            flattened.extend(positions.iter().copied());
        }
        if flattened.len() != cardinality {
            return Err(BlockPartitionError::CardinalityMismatch {
                expected: cardinality,
                found: flattened.len(),
            });
        }
        let order_map = OrdinalMap::try_new(flattened)?;
        Ok(Self {
            cardinality,
            blocks,
            order_map,
        })
    }

    /// Builds contiguous blocks from positive block lengths.
    pub fn contiguous(block_lengths: Vec<usize>) -> Result<Self, BlockPartitionError> {
        let mut cardinality = 0usize;
        let mut blocks = Vec::with_capacity(block_lengths.len());
        for (block, length) in block_lengths.into_iter().enumerate() {
            if length == 0 {
                return Err(BlockPartitionError::EmptyBlock { block });
            }
            let end = cardinality
                .checked_add(length)
                .ok_or(BlockPartitionError::CardinalityOverflow)?;
            blocks.push((cardinality..end).collect());
            cardinality = end;
        }
        Self::try_new(cardinality, blocks)
    }

    /// Returns the number of source positions covered by the partition.
    pub fn cardinality(&self) -> usize {
        self.cardinality
    }

    /// Returns the ordered blocks of source positions.
    pub fn blocks(&self) -> &[Vec<usize>] {
        &self.blocks
    }

    /// Returns the exact linearization map induced by the ordered blocks.
    pub fn order_map(&self) -> &OrdinalMap {
        &self.order_map
    }
}
