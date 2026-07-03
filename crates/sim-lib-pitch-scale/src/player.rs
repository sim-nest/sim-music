use sim_lib_pitch_core::{Pitch, PitchClass};

use crate::{Mode, PitchScaleError, Scale};

/// A performance-oriented scale that owns its interval list, supporting both the
/// built-in [`Mode`]s and arbitrary custom scales.
///
/// Unlike [`Scale`], which is a fixed mode plus tonic, a `PlayerScale` can hold a
/// caller-supplied set of intervals and provides quantization and remapping for
/// live input.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PlayerScale {
    /// The tonic pitch class.
    pub tonic: PitchClass,
    intervals: Vec<u8>,
}

impl PlayerScale {
    /// Builds a player scale from a fixed [`Scale`].
    pub fn from_scale(scale: Scale) -> Self {
        Self {
            tonic: scale.tonic,
            intervals: scale.mode.intervals().to_vec(),
        }
    }

    /// Builds a player scale from a `tonic` and [`Mode`].
    pub fn from_key(tonic: PitchClass, mode: Mode) -> Self {
        Self::from_scale(Scale::new(tonic, mode))
    }

    /// Builds a player scale from a custom interval list (semitone offsets from the
    /// tonic), which is sorted and deduplicated.
    ///
    /// Returns [`PitchScaleError::EmptyScale`] if no intervals are given, or
    /// [`PitchScaleError::InvalidScaleInterval`] if any interval is 12 or more.
    pub fn custom(
        tonic: PitchClass,
        intervals: impl Into<Vec<u8>>,
    ) -> Result<Self, PitchScaleError> {
        let mut intervals = intervals.into();
        if intervals.is_empty() {
            return Err(PitchScaleError::EmptyScale);
        }
        for interval in &intervals {
            if *interval >= 12 {
                return Err(PitchScaleError::InvalidScaleInterval(*interval));
            }
        }
        intervals.sort_unstable();
        intervals.dedup();
        Ok(Self { tonic, intervals })
    }

    /// Returns the scale's semitone offsets from the tonic, in ascending order.
    pub fn intervals(&self) -> &[u8] {
        &self.intervals
    }

    /// Returns the scale's pitch classes in ascending degree order.
    pub fn pitch_classes(&self) -> Vec<PitchClass> {
        self.intervals
            .iter()
            .map(|interval| self.tonic.transpose(i32::from(*interval)))
            .collect()
    }

    /// Returns `true` if `class` is a member of the scale.
    pub fn contains(&self, class: PitchClass) -> bool {
        self.degree_of(class).is_some()
    }

    /// Returns the one-based scale degree of `class`, or `None` if it is not in the
    /// scale.
    pub fn degree_of(&self, class: PitchClass) -> Option<usize> {
        self.pitch_classes()
            .iter()
            .position(|candidate| *candidate == class)
            .map(|index| index + 1)
    }

    /// Returns the pitch class at the one-based `degree`, wrapping past the octave.
    pub fn pitch_at_degree(&self, degree: usize) -> PitchClass {
        let index = degree.saturating_sub(1) % self.intervals.len();
        self.tonic.transpose(i32::from(self.intervals[index]))
    }

    /// Returns the in-scale pitch nearest to `pitch`, breaking ties upward.
    pub fn nearest_pitch(&self, pitch: Pitch) -> Pitch {
        let source = pitch.semitone();
        self.pitch_classes()
            .into_iter()
            .flat_map(|class| {
                (-1..=1).map(move |octave_offset| Pitch {
                    class,
                    octave: pitch.octave + octave_offset,
                })
            })
            .min_by_key(|candidate| {
                let delta = candidate.semitone() - source;
                (delta.abs(), if delta >= 0 { 0 } else { 1 })
            })
            .unwrap_or(pitch)
    }

    /// Remaps `pitch` onto the scale by treating its chromatic offset from the
    /// tonic as a scale degree, preserving the octave.
    pub fn remap_pitch(&self, pitch: Pitch) -> Pitch {
        let offset = (i32::from(pitch.class.0) - i32::from(self.tonic.0)).rem_euclid(12);
        let degree = offset as usize % self.intervals.len() + 1;
        Pitch {
            class: self.pitch_at_degree(degree),
            octave: pitch.octave,
        }
    }
}

/// The strategy a [`ScaleLockPlayer`] uses to force pitches onto its scale.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ScaleLockPolicy {
    /// Snap each pitch to the nearest in-scale pitch.
    Quantize,
    /// Drop pitches that are not already in the scale.
    Filter,
    /// Reinterpret each pitch's chromatic offset as a scale degree.
    Remap,
}

/// Applies a [`ScaleLockPolicy`] to incoming pitches against a [`PlayerScale`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ScaleLockPlayer {
    /// The scale to lock pitches onto.
    pub scale: PlayerScale,
    /// The policy used to handle out-of-scale pitches.
    pub policy: ScaleLockPolicy,
}

impl ScaleLockPlayer {
    /// Constructs a scale-lock player from a [`PlayerScale`] and a policy.
    pub fn new(scale: PlayerScale, policy: ScaleLockPolicy) -> Self {
        Self { scale, policy }
    }

    /// Constructs a scale-lock player directly from a [`Scale`] and a policy.
    pub fn from_scale(scale: Scale, policy: ScaleLockPolicy) -> Self {
        Self::new(PlayerScale::from_scale(scale), policy)
    }

    /// Applies the policy to a single pitch, returning `None` when a
    /// [`ScaleLockPolicy::Filter`] policy rejects it.
    pub fn process_pitch(&self, pitch: Pitch) -> Option<Pitch> {
        match self.policy {
            ScaleLockPolicy::Quantize => Some(self.scale.nearest_pitch(pitch)),
            ScaleLockPolicy::Filter => self.scale.contains(pitch.class).then_some(pitch),
            ScaleLockPolicy::Remap => Some(self.scale.remap_pitch(pitch)),
        }
    }

    /// Applies the policy to a sequence of pitches, collecting the surviving
    /// results.
    pub fn process_pitches(&self, pitches: impl IntoIterator<Item = Pitch>) -> Vec<Pitch> {
        pitches
            .into_iter()
            .filter_map(|pitch| self.process_pitch(pitch))
            .collect()
    }
}
