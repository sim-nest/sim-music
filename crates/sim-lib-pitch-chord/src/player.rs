use sim_lib_pitch_core::Pitch;
use sim_lib_pitch_scale::{PlayerScale, Scale, ScaleLockPlayer, ScaleLockPolicy};

use crate::{PitchChordError, VelocityPolicy, VoicingPolicy};

/// A chord shape used by [`AutoChordPlayer`] to harmonize a played degree.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChordType {
    /// Stack diatonic thirds from the played scale rather than fixed intervals.
    ScaleStack,
    /// A major triad.
    Major,
    /// A minor triad.
    Minor,
    /// A dominant seventh chord.
    DominantSeventh,
    /// A major seventh chord.
    MajorSeventh,
    /// A minor seventh chord.
    MinorSeventh,
    /// A diminished triad.
    Diminished,
    /// A suspended-second chord.
    SuspendedSecond,
    /// A suspended-fourth chord.
    SuspendedFourth,
    /// A power chord (root and fifth).
    Power,
}

impl ChordType {
    fn intervals(self) -> &'static [i32] {
        match self {
            Self::ScaleStack => &[],
            Self::Major => &[0, 4, 7, 12],
            Self::Minor => &[0, 3, 7, 12],
            Self::DominantSeventh => &[0, 4, 7, 10],
            Self::MajorSeventh => &[0, 4, 7, 11],
            Self::MinorSeventh => &[0, 3, 7, 10],
            Self::Diminished => &[0, 3, 6, 12],
            Self::SuspendedSecond => &[0, 2, 7, 12],
            Self::SuspendedFourth => &[0, 5, 7, 12],
            Self::Power => &[0, 7, 12],
        }
    }
}

/// A mapping that assigns a [`ChordType`] to a specific scale degree.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DegreeChordType {
    /// The one-based scale degree this entry applies to.
    pub degree: usize,
    /// The chord type to build on that degree.
    pub chord_type: ChordType,
}

impl DegreeChordType {
    /// Constructs a degree-to-chord-type mapping.
    pub const fn new(degree: usize, chord_type: ChordType) -> Self {
        Self { degree, chord_type }
    }
}

/// Configuration for an [`AutoChordPlayer`]: which scale to harmonize against and
/// how to build, voice, and articulate the resulting chords.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AutoChordConfig {
    /// The scale that anchors the harmonization.
    pub scale: Scale,
    /// Per-degree chord-type assignments.
    pub degree_chords: Vec<DegreeChordType>,
    /// The number of notes to build per chord.
    pub note_count: usize,
    /// The number of inversions to apply.
    pub inversion: usize,
    /// The voicing policy applied to each chord.
    pub voicing: VoicingPolicy,
    /// An octave shift applied to the chord root.
    pub octave_shift: i16,
    /// The velocity policy applied to each note.
    pub velocity: VelocityPolicy,
}

impl AutoChordConfig {
    /// Builds a default configuration for `scale`, assigning scale-stack chords to
    /// every degree.
    pub fn new(scale: Scale) -> Self {
        let degree_chords = (1..=scale.mode.intervals().len())
            .map(|degree| DegreeChordType::new(degree, ChordType::ScaleStack))
            .collect();
        Self {
            scale,
            degree_chords,
            note_count: 3,
            inversion: 0,
            voicing: VoicingPolicy::Closed,
            octave_shift: 0,
            velocity: VelocityPolicy::Preserve,
        }
    }

    /// Replaces the per-degree chord assignments, validating each degree against
    /// the scale.
    ///
    /// Returns [`PitchChordError::EmptyDegreeChordMap`] if empty, or
    /// [`PitchChordError::InvalidScaleDegree`] for an out-of-range degree.
    pub fn with_degree_chords(
        mut self,
        degree_chords: Vec<DegreeChordType>,
    ) -> Result<Self, PitchChordError> {
        if degree_chords.is_empty() {
            return Err(PitchChordError::EmptyDegreeChordMap);
        }
        for entry in &degree_chords {
            if entry.degree == 0 || entry.degree > self.scale.mode.intervals().len() {
                return Err(PitchChordError::InvalidScaleDegree(entry.degree));
            }
        }
        degree_chords.clone_into(&mut self.degree_chords);
        Ok(self)
    }

    /// Sets the number of notes per chord, rejecting zero with
    /// [`PitchChordError::InvalidNoteCount`].
    pub fn with_note_count(mut self, note_count: usize) -> Result<Self, PitchChordError> {
        if note_count == 0 {
            return Err(PitchChordError::InvalidNoteCount);
        }
        self.note_count = note_count;
        Ok(self)
    }

    /// Sets the number of inversions applied to each chord.
    pub fn with_inversion(mut self, inversion: usize) -> Self {
        self.inversion = inversion;
        self
    }

    /// Sets the voicing policy.
    pub fn with_voicing(mut self, voicing: VoicingPolicy) -> Self {
        self.voicing = voicing;
        self
    }

    /// Sets the octave shift applied to the chord root.
    pub fn with_octave_shift(mut self, octave_shift: i16) -> Self {
        self.octave_shift = octave_shift;
        self
    }

    /// Sets the velocity policy.
    pub fn with_velocity(mut self, velocity: VelocityPolicy) -> Self {
        self.velocity = velocity;
        self
    }
}

/// An input note (pitch plus velocity) fed to a chord player.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ScalesChordInput {
    /// The played pitch.
    pub pitch: Pitch,
    /// The played velocity, clamped to `1..=127`.
    pub velocity: u8,
}

impl ScalesChordInput {
    /// Constructs an input note, clamping `velocity` into the `1..=127` range.
    pub fn new(pitch: Pitch, velocity: u8) -> Self {
        Self {
            pitch,
            velocity: velocity.clamp(1, 127),
        }
    }
}

/// A single rendered chord note (pitch plus velocity) emitted by a chord player.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ScalesChordNote {
    /// The rendered pitch.
    pub pitch: Pitch,
    /// The rendered velocity.
    pub velocity: u8,
}

/// Harmonizes incoming notes into chords by interpreting each played pitch as a
/// scale degree and building the configured [`ChordType`] on it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AutoChordPlayer {
    config: AutoChordConfig,
    scale: PlayerScale,
}

impl AutoChordPlayer {
    /// Builds a player from `config`, rejecting empty note counts or degree maps.
    pub fn new(config: AutoChordConfig) -> Result<Self, PitchChordError> {
        if config.note_count == 0 {
            return Err(PitchChordError::InvalidNoteCount);
        }
        if config.degree_chords.is_empty() {
            return Err(PitchChordError::EmptyDegreeChordMap);
        }
        Ok(Self {
            scale: PlayerScale::from_scale(config.scale),
            config,
        })
    }

    /// Returns the player's configuration.
    pub fn config(&self) -> &AutoChordConfig {
        &self.config
    }

    /// Renders the chord for a single input note, applying inversion, voicing, and
    /// velocity policy.
    pub fn render_note(&self, input: ScalesChordInput) -> Vec<ScalesChordNote> {
        let root = self
            .scale
            .nearest_pitch(input.pitch)
            .transpose(12 * i32::from(self.config.octave_shift));
        let degree = self.scale.degree_of(root.class).unwrap_or(1);
        let chord_type = self.chord_type_for_degree(degree);
        let mut notes = self.build_chord(root, chord_type);
        for _ in 0..self.config.inversion {
            sort_by_semitone(&mut notes);
            let lowest = notes.remove(0);
            notes.push(lowest.transpose(12));
        }
        self.config
            .voicing
            .apply(notes)
            .into_iter()
            .map(|pitch| ScalesChordNote {
                pitch,
                velocity: self.config.velocity.apply(input.velocity),
            })
            .collect()
    }

    fn chord_type_for_degree(&self, degree: usize) -> ChordType {
        self.config
            .degree_chords
            .iter()
            .find(|entry| entry.degree == degree)
            .map(|entry| entry.chord_type)
            .unwrap_or(ChordType::ScaleStack)
    }

    fn build_chord(&self, root: Pitch, chord_type: ChordType) -> Vec<Pitch> {
        if chord_type == ChordType::ScaleStack {
            return (0..self.config.note_count)
                .map(|index| {
                    self.config
                        .scale
                        .transpose_diatonic(root, (index * 2) as i32)
                        .unwrap_or_else(|_| root.transpose((index * 4) as i32))
                })
                .collect();
        }
        let intervals = chord_type.intervals();
        (0..self.config.note_count)
            .map(|index| {
                let interval =
                    intervals[index % intervals.len()] + 12 * (index / intervals.len()) as i32;
                root.transpose(interval)
            })
            .collect()
    }
}

fn sort_by_semitone(notes: &mut [Pitch]) {
    notes.sort_by_key(|pitch| pitch.semitone());
}

/// Combines a [`ScaleLockPlayer`] with an [`AutoChordPlayer`] so that incoming
/// pitches are first locked to a scale and then harmonized into chords.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ScalesChordsPlayer {
    /// The scale-lock stage applied before harmonization.
    pub scale_lock: ScaleLockPlayer,
    /// The chord-harmonization stage.
    pub chord: AutoChordPlayer,
}

impl ScalesChordsPlayer {
    /// Constructs a combined player from its two stages.
    pub fn new(scale_lock: ScaleLockPlayer, chord: AutoChordPlayer) -> Self {
        Self { scale_lock, chord }
    }

    /// Constructs a combined player from a scale, a scale-lock policy, and a chord
    /// configuration.
    pub fn from_config(
        scale: Scale,
        scale_policy: ScaleLockPolicy,
        chord: AutoChordConfig,
    ) -> Result<Self, PitchChordError> {
        Ok(Self::new(
            ScaleLockPlayer::from_scale(scale, scale_policy),
            AutoChordPlayer::new(chord)?,
        ))
    }

    /// Locks `input` to the scale, then harmonizes it, returning an empty vector
    /// when the scale-lock filter rejects the pitch.
    pub fn process_note(&self, input: ScalesChordInput) -> Vec<ScalesChordNote> {
        let Some(pitch) = self.scale_lock.process_pitch(input.pitch) else {
            return Vec::new();
        };
        self.chord.render_note(ScalesChordInput {
            pitch,
            velocity: input.velocity,
        })
    }
}
