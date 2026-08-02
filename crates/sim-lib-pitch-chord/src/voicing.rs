use std::collections::BTreeSet;

use crate::Chord;
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
    /// Keep the caller's exact pitch order and registers.
    Preserve,
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
        match self {
            Self::Preserve => notes,
            Self::Closed => compact_closed(notes),
            Self::Open { spread } => {
                sort_by_semitone(&mut notes);
                open_voicing(notes, spread)
            }
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

/// One unique application of a [`VoicingPolicy`] to a chord.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoicingPaletteEntry {
    /// Stable position-based identifier within the palette.
    pub id: String,
    /// Policy that produced this first occurrence.
    pub policy: VoicingPolicy,
    /// Exact ordered pitches produced by the policy.
    pub pitches: Vec<Pitch>,
}

/// Deterministically deduplicated voicings of one chord.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoicingPalette {
    /// Unique voicings in caller-supplied policy order.
    pub entries: Vec<VoicingPaletteEntry>,
}

impl VoicingPalette {
    /// Builds a palette, retaining the first policy that produces each exact
    /// ordered pitch vector.
    pub fn from_policies(chord: &Chord, policies: impl IntoIterator<Item = VoicingPolicy>) -> Self {
        let pitches = chord.pitches();
        let mut seen = BTreeSet::new();
        let mut entries = Vec::new();
        for policy in policies {
            let voiced = policy.apply(pitches.clone());
            if !seen.insert(voiced.clone()) {
                continue;
            }
            entries.push(VoicingPaletteEntry {
                id: format!("voicing/{}", entries.len()),
                policy,
                pitches: voiced,
            });
        }
        Self { entries }
    }

    /// Returns the palette entry with `id`.
    pub fn get(&self, id: &str) -> Option<&VoicingPaletteEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }
}

#[cfg(test)]
mod palette_tests {
    use super::*;

    #[test]
    fn palette_deduplicates_by_exact_voicing_and_keeps_first_policy() {
        let chord = Chord::from_root_intervals(Pitch::from_midi(60), &[4, 7]);
        let palette = VoicingPalette::from_policies(
            &chord,
            [
                VoicingPolicy::Closed,
                VoicingPolicy::Closed,
                VoicingPolicy::Open { spread: 12 },
            ],
        );

        assert_eq!(palette.entries.len(), 2);
        assert_eq!(palette.entries[0].id, "voicing/0");
        assert_eq!(palette.entries[0].policy, VoicingPolicy::Closed);
        assert_eq!(
            palette.entries[1]
                .pitches
                .iter()
                .map(|pitch| pitch.to_midi().expect("MIDI pitch"))
                .collect::<Vec<_>>(),
            vec![60, 76, 91]
        );
        assert_eq!(palette.get("voicing/1"), palette.entries.get(1));
    }
}
