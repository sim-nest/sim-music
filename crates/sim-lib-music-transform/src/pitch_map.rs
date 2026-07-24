use std::collections::BTreeMap;

use sim_lib_pitch_core::{
    OctaveSpace, Pitch, PitchClass, TieDirection, folded_distance, split_floor,
};
use sim_lib_pitch_scale::Scale;
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

/// Error returned by partial pitch map construction, application, or composition.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum MapError {
    /// A map image length did not match its octave-space domain.
    #[error("pitch map image length {image_len} does not match domain length {domain_len}")]
    ImageLengthMismatch {
        /// Domain length.
        domain_len: usize,
        /// Image length.
        image_len: usize,
    },
    /// Two composed maps used different octave spaces.
    #[error("pitch map domains differ: left {left_len}, right {right_len}")]
    DomainMismatch {
        /// Left map domain length.
        left_len: u16,
        /// Right map domain length.
        right_len: u16,
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
    /// The map can be composed as an integer map but cannot be applied to
    /// octave-aware [`Pitch`] values.
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

/// Witness explaining how a [`PitchMap`] handled one input pitch.
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

/// Result of applying a [`PitchMap`] to one pitch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchMapResult {
    /// Mapped pitch.
    pub pitch: Pitch,
    /// Witness for the mapping path.
    pub witness: MapWitness,
}

/// Partial inverse row for a target value in a [`PitchMap`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapInverseWitness {
    /// Target image value.
    pub target: i32,
    /// Source classes that map directly to `target`.
    pub sources: Vec<u16>,
}

/// Witness for one source class while composing two pitch maps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapCompositionWitness {
    /// Both maps had direct images for the composition path.
    Direct {
        /// Source class in the left map.
        source_class: u16,
        /// Intermediate value produced by the left map.
        via_value: i32,
        /// Target value produced by the right map.
        target_value: i32,
    },
    /// At least one direct image was absent.
    Undefined {
        /// Source class in the left map.
        source_class: u16,
        /// Stable reason the composition is partial at this class.
        reason: &'static str,
    },
}

/// Result of composing two [`PitchMap`] values with per-class witnesses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchMapComposition {
    /// Composed map.
    pub map: PitchMap,
    /// Witnesses for direct and lossy source classes.
    pub witnesses: Vec<MapCompositionWitness>,
}

/// Partial map from folded source classes to absolute target offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchMap {
    /// Source octave-like domain.
    pub domain: OctaveSpace,
    /// Per-source-class target offsets. `None` records a hole.
    pub image: Vec<Option<i32>>,
    /// Policy for holes during pitch application.
    pub policy: PitchMapPolicy,
}

impl PitchMap {
    /// Builds a pitch map and verifies that the image covers the whole domain.
    pub fn new(
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

    /// Builds an identity map over `domain`.
    pub fn identity(domain: OctaveSpace, policy: PitchMapPolicy) -> Self {
        Self {
            domain,
            image: (0..domain.len())
                .map(|value| Some(i32::from(value)))
                .collect(),
            policy,
        }
    }

    /// Builds a chromatic transposition map in twelve-tone semitone space.
    pub fn chromatic_delta(semitones: i32) -> Self {
        let domain = OctaveSpace::twelve_tone();
        Self {
            domain,
            image: (0..domain.len())
                .map(|value| Some(i32::from(value) + semitones))
                .collect(),
            policy: PitchMapPolicy::Reject,
        }
    }

    /// Builds a pitch-class rotation map over any octave space.
    pub fn rotation(domain: OctaveSpace, steps: i32, policy: PitchMapPolicy) -> Self {
        let len = i32::from(domain.len());
        Self {
            domain,
            image: (0..domain.len())
                .map(|value| Some((i32::from(value) + steps).rem_euclid(len)))
                .collect(),
            policy,
        }
    }

    /// Builds a twelve-tone inversion map around `axis`.
    pub fn inversion(axis: PitchClass) -> Self {
        let axis = i32::from(axis.value());
        let domain = OctaveSpace::twelve_tone();
        Self {
            domain,
            image: (0..domain.len())
                .map(|value| Some((2 * axis - i32::from(value)).rem_euclid(12)))
                .collect(),
            policy: PitchMapPolicy::Reject,
        }
    }

    /// Builds a twelve-tone pitch-class substitution map.
    pub fn pitch_class_substitution(
        from: PitchClass,
        to: PitchClass,
        policy: PitchMapPolicy,
    ) -> Self {
        let domain = OctaveSpace::twelve_tone();
        let mut map = Self::identity(domain, policy);
        map.image[usize::from(from.value())] = Some(i32::from(to.value()));
        map
    }

    /// Builds a partial scale-lock map whose holes are handled by `policy`.
    pub fn from_scale(scale: Scale, policy: PitchMapPolicy) -> Self {
        let domain = OctaveSpace::twelve_tone();
        let mut image = vec![None; usize::from(domain.len())];
        for class in scale.pitch_classes() {
            let value = class.value();
            image[usize::from(value)] = Some(i32::from(value));
        }
        Self {
            domain,
            image,
            policy,
        }
    }

    /// Returns `true` when at least one source class has no direct image.
    pub fn is_partial(&self) -> bool {
        self.image.iter().any(Option::is_none)
    }

    /// Applies the map to a pitch, returning the pitch and an explicit witness.
    pub fn map_pitch(&self, pitch: Pitch) -> Result<PitchMapResult, MapError> {
        if self.domain != OctaveSpace::twelve_tone() {
            return Err(MapError::UnsupportedPitchDomain {
                divisions: self.domain.len(),
            });
        }
        let (value, witness) = self.map_value(i64::from(pitch.semitone()))?;
        let value_i32 = i32::try_from(value).map_err(|_| MapError::TargetOutOfRange { value })?;
        Ok(PitchMapResult {
            pitch: Pitch::from_semitone(value_i32),
            witness,
        })
    }

    /// Returns grouped direct inverse witnesses for every target value.
    pub fn inverse_witnesses(&self) -> Vec<MapInverseWitness> {
        let mut groups: BTreeMap<i32, Vec<u16>> = BTreeMap::new();
        for (source, target) in self.image.iter().enumerate() {
            if let Some(target) = target {
                groups
                    .entry(*target)
                    .or_default()
                    .push(u16::try_from(source).expect("source class fits u16"));
            }
        }
        groups
            .into_iter()
            .map(|(target, sources)| MapInverseWitness { target, sources })
            .collect()
    }

    /// Returns `true` when the direct inverse is incomplete or many-to-one.
    pub fn has_partial_inverse(&self) -> bool {
        self.is_partial()
            || self
                .inverse_witnesses()
                .iter()
                .any(|witness| witness.sources.len() != 1)
    }

    fn map_value(&self, value: i64) -> Result<(i64, MapWitness), MapError> {
        let (octave, source_class) = split_floor(value, self.domain);
        let source_index = usize::from(source_class);
        if let Some(target) = self.image[source_index] {
            let target_value = target_value(octave, self.domain, target);
            return Ok((
                target_value,
                MapWitness::Direct {
                    source_class,
                    target_value,
                },
            ));
        }

        match self.policy {
            PitchMapPolicy::Unmapped => Ok((value, MapWitness::Unmapped { source_class })),
            PitchMapPolicy::Reject => Err(MapError::Unmapped {
                class: source_class,
            }),
            PitchMapPolicy::Clamp | PitchMapPolicy::Nearest => {
                let (chosen_value, chosen_class, target) =
                    self.choose_mapped_class(value, source_class)?;
                let target_value = target_value(
                    chosen_value.div_euclid(i64::from(self.domain.len())),
                    self.domain,
                    target,
                );
                Ok((
                    target_value,
                    MapWitness::Nudged {
                        source_class,
                        chosen_class,
                        target_value,
                        policy: self.policy,
                    },
                ))
            }
        }
    }

    fn choose_mapped_class(
        &self,
        value: i64,
        source_class: u16,
    ) -> Result<(i64, u16, i32), MapError> {
        let candidates = self.mapped_candidates();
        if candidates.is_empty() {
            return Err(MapError::NoMappedEntries);
        }
        let len = i64::from(self.domain.len());
        let source_octave = value.div_euclid(len);
        match self.policy {
            PitchMapPolicy::Clamp => {
                let chosen = clamp_candidate(&candidates, source_class);
                Ok((
                    source_octave * len + i64::from(chosen.0),
                    chosen.0,
                    chosen.1,
                ))
            }
            PitchMapPolicy::Nearest => {
                let chosen = nearest_candidate(&candidates, source_class, self.domain);
                Ok((value + i64::from(chosen.2), chosen.0, chosen.1))
            }
            PitchMapPolicy::Unmapped | PitchMapPolicy::Reject => unreachable!(),
        }
    }

    fn mapped_candidates(&self) -> Vec<(u16, i32)> {
        self.image
            .iter()
            .enumerate()
            .filter_map(|(source, target)| {
                target.map(|target| {
                    (
                        u16::try_from(source).expect("source class fits u16"),
                        target,
                    )
                })
            })
            .collect()
    }
}

/// Composes `a` followed by `b`, discarding per-class witnesses.
pub fn compose_pitch_maps(a: &PitchMap, b: &PitchMap) -> Result<PitchMap, MapError> {
    Ok(compose_pitch_map_report(a, b)?.map)
}

/// Composes `a` followed by `b`, returning witnesses for lossy classes.
pub fn compose_pitch_map_report(
    a: &PitchMap,
    b: &PitchMap,
) -> Result<PitchMapComposition, MapError> {
    if a.domain != b.domain {
        return Err(MapError::DomainMismatch {
            left_len: a.domain.len(),
            right_len: b.domain.len(),
        });
    }
    let mut image = Vec::with_capacity(a.image.len());
    let mut witnesses = Vec::with_capacity(a.image.len());
    for (source, first) in a.image.iter().enumerate() {
        let source_class = u16::try_from(source).expect("source class fits u16");
        let Some(via_value) = first else {
            image.push(None);
            witnesses.push(MapCompositionWitness::Undefined {
                source_class,
                reason: "left map has no image",
            });
            continue;
        };
        let (via_octave, via_class) = split_floor(i64::from(*via_value), a.domain);
        let Some(second) = b.image[usize::from(via_class)] else {
            image.push(None);
            witnesses.push(MapCompositionWitness::Undefined {
                source_class,
                reason: "right map has no image",
            });
            continue;
        };
        let target_value = target_value(via_octave, a.domain, second);
        let target_i32 = i32::try_from(target_value).map_err(|_| MapError::TargetOutOfRange {
            value: target_value,
        })?;
        image.push(Some(target_i32));
        witnesses.push(MapCompositionWitness::Direct {
            source_class,
            via_value: *via_value,
            target_value: target_i32,
        });
    }
    Ok(PitchMapComposition {
        map: PitchMap {
            domain: a.domain,
            image,
            policy: b.policy,
        },
        witnesses,
    })
}

fn target_value(octave: i64, domain: OctaveSpace, target: i32) -> i64 {
    octave * i64::from(domain.len()) + i64::from(target)
}

fn clamp_candidate(candidates: &[(u16, i32)], source_class: u16) -> (u16, i32) {
    if source_class <= candidates[0].0 {
        return candidates[0];
    }
    if source_class >= candidates[candidates.len() - 1].0 {
        return candidates[candidates.len() - 1];
    }
    *candidates
        .iter()
        .min_by_key(|(candidate, _)| {
            let distance = candidate.abs_diff(source_class);
            let upward = u8::from(*candidate > source_class);
            (distance, upward, *candidate)
        })
        .expect("candidates are non-empty")
}

fn nearest_candidate(
    candidates: &[(u16, i32)],
    source_class: u16,
    domain: OctaveSpace,
) -> (u16, i32, i32) {
    candidates
        .iter()
        .map(|(candidate, target)| {
            let distance = folded_distance(
                i64::from(source_class),
                i64::from(*candidate),
                domain,
                TieDirection::Ascending,
            );
            (*candidate, *target, distance)
        })
        .min_by_key(|(candidate, _, distance)| {
            let upward = u8::from(*distance < 0);
            (distance.abs(), upward, *candidate)
        })
        .expect("candidates are non-empty")
}
