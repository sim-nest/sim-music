//! Ordered pitch reservoirs used when strict row invariants are relaxed.

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_set::PitchClassMask;

/// One ordered pitch block inside a reservoir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderedPitchBlock {
    /// Exact pitch classes in retained presentation order.
    pub pitch_classes: Vec<PitchClass>,
    /// Unordered pitch-class content of the block.
    pub mask: PitchClassMask,
}

/// A source invariant that no longer holds after a transform.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PitchInvariant {
    /// The result is no longer one strict twelve-position row.
    TotalOrder,
    /// The result no longer contains each pitch class exactly once.
    AggregateIdentity,
}

/// Which row invariants a transform preserved or relaxed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvariantDelta {
    /// Whether the result remains one strict total ordering of twelve positions.
    pub retains_total_order: bool,
    /// Whether the result still contains each pitch class exactly once.
    pub retains_aggregate_identity: bool,
    /// Named invariants intentionally relaxed by the transform.
    pub relaxed_invariants: Vec<PitchInvariant>,
}

/// Provenance for one reservoir block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockProjection {
    /// Result block index within the reservoir.
    pub block_index: usize,
    /// How this block was derived from source material.
    pub source: BlockProjectionSource,
}

/// The source relationship that produced one reservoir block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockProjectionSource {
    /// Several source ordinals collapsed onto one pitch class under a non-bijective affine map.
    OrdinalCollapse {
        /// Source row ordinals contributing to the collapsed block.
        source_ordinals: Vec<u8>,
        /// The common mapped pitch class.
        target_pitch_class: PitchClass,
    },
    /// One block's interval content was projected onto another block's pitches.
    BlockMultiplication {
        /// Source block index providing anchor pitches.
        anchor_block_index: usize,
        /// Source row ordinals providing anchor pitches.
        anchor_ordinals: Vec<u8>,
        /// Source block index providing interval content.
        interval_block_index: usize,
        /// Source row ordinals providing interval content.
        interval_ordinals: Vec<u8>,
        /// Ordered intervals projected from the interval block's first pitch.
        interval_content: Vec<u8>,
    },
}

/// An ordered collection of pitch blocks whose result is not a strict tone row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchReservoir {
    /// Result blocks in retained presentation order.
    pub blocks: Vec<OrderedPitchBlock>,
    /// Provenance for each result block.
    pub provenance: Vec<BlockProjection>,
    /// Which strict-row invariants no longer hold.
    pub invariant_delta: InvariantDelta,
}

impl PitchReservoir {
    /// Constructs a reservoir and derives its invariant delta from the supplied blocks.
    pub fn new(blocks: Vec<OrderedPitchBlock>, provenance: Vec<BlockProjection>) -> Self {
        let mut counts = [0u8; 12];
        for pitch_class in blocks.iter().flat_map(|block| block.pitch_classes.iter()) {
            counts[usize::from(pitch_class.value())] += 1;
        }
        let retains_aggregate_identity = counts.iter().all(|count| *count == 1);
        let retains_total_order = blocks.len() == 1
            && blocks
                .first()
                .is_some_and(|block| block.pitch_classes.len() == 12 && retains_aggregate_identity);
        let mut relaxed_invariants = Vec::new();
        if !retains_total_order {
            relaxed_invariants.push(PitchInvariant::TotalOrder);
        }
        if !retains_aggregate_identity {
            relaxed_invariants.push(PitchInvariant::AggregateIdentity);
        }
        Self {
            blocks,
            provenance,
            invariant_delta: InvariantDelta {
                retains_total_order,
                retains_aggregate_identity,
                relaxed_invariants,
            },
        }
    }
}
