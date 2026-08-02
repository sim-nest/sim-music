//! Local pitch-map support retained inside serial so transform depends one-way on serial.

use sim_lib_pitch_core::{OctaveSpace, Pitch, TieDirection, folded_distance, split_floor};
use thiserror::Error;

/// Policy used when a [`PitchMap`] has no direct image for a source class.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PitchMapPolicy {
    /// Keep the input pitch and record an unmapped witness.
    Unmapped,
    /// Clamp to the nearest mapped source class without wrapping around the domain.
    Clamp,
    /// Reject the pitch as a diagnostic or direct mapping error.
    Reject,
    /// Nudge to the nearest mapped source class on the circular octave space.
    Nearest,
}

/// Error returned by partial pitch map construction or application.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum MapError {
    /// A map image length did not match its octave-space domain.
    #[error("pitch map image length {image_len} does not match domain length {domain_len}")]
    ImageLengthMismatch {
        /// Domain length.
        domain_len: usize,
        /// Image length.
        image_len: usize,
    },
    /// A map with a nudge policy had no mapped entries to nudge toward.
    #[error("pitch map has no mapped entries")]
    NoMappedEntries,
    /// A reject policy encountered an unmapped source class.
    #[error("pitch map rejected unmapped class {class}")]
    Unmapped {
        /// Folded source class.
        class: u16,
    },
    /// The map cannot be applied to octave-aware pitches.
    #[error("pitch map domain {divisions} cannot map octave-aware Pitch values")]
    UnsupportedPitchDomain {
        /// Domain division count.
        divisions: u16,
    },
    /// A mapped absolute value cannot be represented as a [`Pitch`] semitone.
    #[error("pitch map target value {value} is outside the supported Pitch range")]
    TargetOutOfRange {
        /// Absolute target value.
        value: i64,
    },
}

/// Witness explaining how the internal pitch map handled one input pitch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapWitness {
    /// The source class had a direct image.
    Direct {
        /// Folded source class.
        source_class: u16,
        /// Mapped absolute semitone.
        target_value: i64,
    },
    /// The map left an unmapped source unchanged.
    Unmapped {
        /// Folded source class.
        source_class: u16,
    },
    /// The map used an explicit policy to choose a mapped source class.
    Nudged {
        /// Folded source class requested by the input pitch.
        source_class: u16,
        /// Folded mapped class selected by the policy.
        chosen_class: u16,
        /// Mapped absolute semitone.
        target_value: i64,
        /// Policy that chose the mapped class.
        policy: PitchMapPolicy,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PitchMapResult {
    pub pitch: Pitch,
    pub witness: MapWitness,
}

/// Partial map from folded source classes to absolute target offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PitchMap {
    pub domain: OctaveSpace,
    pub image: Vec<Option<i32>>,
    pub policy: PitchMapPolicy,
}

impl PitchMap {
    pub(crate) fn new(
        domain: OctaveSpace,
        image: Vec<Option<i32>>,
        policy: PitchMapPolicy,
    ) -> Result<Self, MapError> {
        let domain_len = usize::from(domain.len());
        if image.len() != domain_len {
            return Err(MapError::ImageLengthMismatch {
                domain_len,
                image_len: image.len(),
            });
        }
        Ok(Self {
            domain,
            image,
            policy,
        })
    }

    pub(crate) fn map_pitch(&self, pitch: Pitch) -> Result<PitchMapResult, MapError> {
        if self.domain != OctaveSpace::twelve_tone() {
            return Err(MapError::UnsupportedPitchDomain {
                divisions: self.domain.len(),
            });
        }
        let (value, witness) = self.map_value(i64::from(pitch.semitone()))?;
        let semitone = i32::try_from(value).map_err(|_| MapError::TargetOutOfRange { value })?;
        Ok(PitchMapResult {
            pitch: Pitch::from_semitone(semitone),
            witness,
        })
    }

    fn map_value(&self, value: i64) -> Result<(i64, MapWitness), MapError> {
        let divisions = i64::from(self.domain.len());
        let (octaves, folded) = split_floor(value, self.domain);
        let source_class = folded;
        let Some(mapped) = self.image[usize::from(source_class)] else {
            return self.map_hole(octaves, source_class);
        };
        let target_value = octaves * divisions + i64::from(mapped);
        Ok((
            target_value,
            MapWitness::Direct {
                source_class,
                target_value,
            },
        ))
    }

    fn map_hole(&self, octaves: i64, source_class: u16) -> Result<(i64, MapWitness), MapError> {
        match self.policy {
            PitchMapPolicy::Unmapped => {
                let target_value = octaves * i64::from(self.domain.len()) + i64::from(source_class);
                Ok((target_value, MapWitness::Unmapped { source_class }))
            }
            PitchMapPolicy::Reject => Err(MapError::Unmapped {
                class: source_class,
            }),
            PitchMapPolicy::Clamp | PitchMapPolicy::Nearest => {
                let chosen = self.choose_mapped_class(source_class)?;
                let mapped = self.image[usize::from(chosen)].expect("chosen mapped class");
                let target_value = octaves * i64::from(self.domain.len()) + i64::from(mapped);
                Ok((
                    target_value,
                    MapWitness::Nudged {
                        source_class,
                        chosen_class: chosen,
                        target_value,
                        policy: self.policy,
                    },
                ))
            }
        }
    }

    fn choose_mapped_class(&self, source_class: u16) -> Result<u16, MapError> {
        let mapped = self
            .image
            .iter()
            .enumerate()
            .filter_map(|(index, value)| value.map(|_| u16::try_from(index).expect("class index")))
            .collect::<Vec<_>>();
        if mapped.is_empty() {
            return Err(MapError::NoMappedEntries);
        }
        match self.policy {
            PitchMapPolicy::Clamp => mapped
                .iter()
                .copied()
                .min_by_key(|candidate| {
                    let delta = i32::from(*candidate) - i32::from(source_class);
                    (delta.abs(), delta.is_negative())
                })
                .ok_or(MapError::NoMappedEntries),
            PitchMapPolicy::Nearest => mapped
                .iter()
                .copied()
                .min_by_key(|candidate| {
                    (
                        folded_distance(
                            i64::from(*candidate),
                            i64::from(source_class),
                            self.domain,
                            TieDirection::Descending,
                        ),
                        candidate.cmp(&source_class).is_gt(),
                    )
                })
                .ok_or(MapError::NoMappedEntries),
            PitchMapPolicy::Unmapped | PitchMapPolicy::Reject => unreachable!("handled above"),
        }
    }
}
