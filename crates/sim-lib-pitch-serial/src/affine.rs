//! Ordinal and affine transforms over strict tone rows.

use std::collections::BTreeMap;

use sim_lib_pitch_core::PitchClass;
use sim_lib_serial_core::OrdinalMap;

use crate::{
    BlockProjection, BlockProjectionSource, OrderedPitchBlock, PitchReservoir, RowError, ToneRow,
};

/// Result of a pitch transform that may or may not preserve strict row identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PitchTransformOutput {
    /// The transform preserved a strict tone row.
    Row(ToneRow),
    /// The transform relaxed strict row invariants into an ordered reservoir.
    Reservoir(PitchReservoir),
}

/// An affine pitch-class map `x -> ax + b mod 12`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AffinePitchMap {
    /// Multiplicative factor `a`, reduced modulo twelve.
    pub multiplier: u8,
    /// Additive factor `b`, reduced modulo twelve.
    pub addend: u8,
}

impl AffinePitchMap {
    /// Constructs an affine pitch-class map with canonical modulo-twelve factors.
    pub const fn new(multiplier: u8, addend: u8) -> Self {
        Self {
            multiplier: multiplier % 12,
            addend: addend % 12,
        }
    }

    /// Returns `true` exactly when the map is bijective over pitch classes.
    pub const fn is_bijective(self) -> bool {
        matches!(self.multiplier % 12, 1 | 5 | 7 | 11)
    }

    /// Applies the affine map to `row`, returning a strict row only for bijections.
    pub fn apply(self, row: &ToneRow) -> PitchTransformOutput {
        let mapped = row
            .classes()
            .map(|pitch_class| self.map_pitch_class(pitch_class));
        if self.is_bijective() {
            PitchTransformOutput::Row(ToneRow::from_valid_classes(mapped))
        } else {
            PitchTransformOutput::Reservoir(self.into_reservoir(row, mapped))
        }
    }

    fn map_pitch_class(self, pitch_class: PitchClass) -> PitchClass {
        from_mod12(
            (u16::from(self.multiplier) * u16::from(pitch_class.value()) + u16::from(self.addend))
                % 12,
        )
    }

    fn into_reservoir(self, row: &ToneRow, mapped: [PitchClass; 12]) -> PitchReservoir {
        let mut ordinals_by_pitch = BTreeMap::<u8, Vec<u8>>::new();
        for (ordinal, pitch_class) in mapped.iter().enumerate() {
            ordinals_by_pitch
                .entry(pitch_class.value())
                .or_default()
                .push(ordinal as u8);
        }
        let mut blocks = Vec::with_capacity(ordinals_by_pitch.len());
        let mut provenance = Vec::with_capacity(ordinals_by_pitch.len());
        for (block_index, (pitch_value, ordinals)) in ordinals_by_pitch.into_iter().enumerate() {
            let target_pitch_class = from_mod12(u16::from(pitch_value));
            let pitch_classes = ordinals
                .iter()
                .map(|ordinal| row.classes()[usize::from(*ordinal)])
                .map(|pitch_class| self.map_pitch_class(pitch_class))
                .collect::<Vec<_>>();
            blocks.push(OrderedPitchBlock {
                mask: sim_lib_pitch_set::PitchClassMask::from_pitch_classes(&pitch_classes),
                pitch_classes,
            });
            provenance.push(BlockProjection {
                block_index,
                source: BlockProjectionSource::OrdinalCollapse {
                    source_ordinals: ordinals,
                    target_pitch_class,
                },
            });
        }
        PitchReservoir::new(blocks, provenance)
    }
}

impl ToneRow {
    /// Returns the row rotated left by `steps`, reduced modulo twelve.
    pub fn rotate(&self, steps: usize) -> Self {
        self.permute_ordinals(&OrdinalMap::rotation(12, steps))
            .expect("fixed-cardinality rotation is always valid")
    }

    /// Applies a caller-supplied validated ordinal permutation to the row.
    pub fn permute_ordinals(&self, permutation: &OrdinalMap) -> Result<Self, RowError> {
        let classes = permutation.apply(self.classes())?;
        let classes = std::array::from_fn(|index| classes[index]);
        Ok(Self::from_valid_classes(classes))
    }

    /// Validates and applies a raw output-to-input ordinal permutation.
    pub fn try_permute_ordinals(&self, output_to_input: Vec<usize>) -> Result<Self, RowError> {
        self.permute_ordinals(&OrdinalMap::try_new(output_to_input)?)
    }
}

fn from_mod12(value: u16) -> PitchClass {
    match value % 12 {
        0 => PitchClass::C,
        1 => PitchClass::CS,
        2 => PitchClass::D,
        3 => PitchClass::DS,
        4 => PitchClass::E,
        5 => PitchClass::F,
        6 => PitchClass::FS,
        7 => PitchClass::G,
        8 => PitchClass::GS,
        9 => PitchClass::A,
        10 => PitchClass::AS,
        11 => PitchClass::B,
        _ => unreachable!(),
    }
}
