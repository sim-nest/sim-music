use sim_lib_pitch_core::Pitch;

/// A policy for deriving an output velocity from an input velocity.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum VelocityPolicy {
    /// Pass the input velocity through unchanged.
    Preserve,
    /// Replace the velocity with a fixed value (clamped to `1..=127`).
    Fixed(u8),
    /// Add a signed offset to the velocity (clamped to `1..=127`).
    Offset(i16),
}

impl VelocityPolicy {
    /// Applies the policy to `velocity`, clamping the result to `1..=127`.
    pub fn apply(self, velocity: u8) -> u8 {
        match self {
            Self::Preserve => velocity,
            Self::Fixed(value) => value.clamp(1, 127),
            Self::Offset(offset) => {
                let shifted = i16::from(velocity) + offset;
                shifted.clamp(1, 127) as u8
            }
        }
    }
}

/// A policy for arranging a chord's notes across registers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum VoicingPolicy {
    /// Pack the notes into the closest possible position.
    Closed,
    /// Spread the notes apart, transposing each successive note by `spread`
    /// semitones.
    Open {
        /// The per-voice spread in semitones.
        spread: i32,
    },
    /// Drop one voice down by a number of octaves (a drop voicing).
    Drop {
        /// The voice to drop, counted from the top (0 is the highest).
        voice_index_from_top: usize,
        /// The number of octaves to drop it.
        octaves: i16,
    },
}

impl VoicingPolicy {
    /// Applies the voicing policy to `notes`, returning the rearranged pitches.
    pub fn apply(self, mut notes: Vec<Pitch>) -> Vec<Pitch> {
        sort_by_semitone(&mut notes);
        match self {
            Self::Closed => compact_closed(notes),
            Self::Open { spread } => open_voicing(notes, spread),
            Self::Drop {
                voice_index_from_top,
                octaves,
            } => drop_voice(notes, voice_index_from_top, octaves),
        }
    }
}

fn compact_closed(mut notes: Vec<Pitch>) -> Vec<Pitch> {
    sort_by_semitone(&mut notes);
    for index in 1..notes.len() {
        while notes[index].semitone() - notes[index - 1].semitone() > 12 {
            notes[index] = notes[index].transpose(-12);
        }
    }
    sort_by_semitone(&mut notes);
    notes
}

fn open_voicing(notes: Vec<Pitch>, spread: i32) -> Vec<Pitch> {
    notes
        .into_iter()
        .enumerate()
        .map(|(index, pitch)| pitch.transpose(spread * index as i32))
        .collect()
}

fn drop_voice(mut notes: Vec<Pitch>, voice_index_from_top: usize, octaves: i16) -> Vec<Pitch> {
    if notes.is_empty() {
        return notes;
    }
    sort_by_semitone(&mut notes);
    let index = notes
        .len()
        .saturating_sub(1)
        .saturating_sub(voice_index_from_top);
    notes[index] = notes[index].transpose(-12 * i32::from(octaves.max(0)));
    sort_by_semitone(&mut notes);
    notes
}

fn sort_by_semitone(notes: &mut [Pitch]) {
    notes.sort_by_key(|pitch| pitch.semitone());
}
