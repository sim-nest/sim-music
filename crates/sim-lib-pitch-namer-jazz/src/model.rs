use std::fmt;

use sim_lib_pitch_chord::Chord;
use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_pitch_set::PitchClassMask;

/// A jazz chord quality.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum JazzQuality {
    /// A major triad.
    Major,
    /// A minor triad.
    Minor,
    /// A dominant seventh chord.
    Dominant7,
    /// A major seventh chord.
    Major7,
    /// A minor seventh chord.
    Minor7,
    /// A diminished triad.
    Diminished,
    /// An augmented triad.
    Augmented,
    /// A suspended-second chord.
    Suspended2,
    /// A suspended-fourth chord.
    Suspended4,
    /// A major sixth chord.
    Sixth,
    /// A minor sixth chord.
    Minor6,
}

impl JazzQuality {
    /// Returns the chord-symbol suffix for this quality (for example `"maj7"`).
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Major => "",
            Self::Minor => "m",
            Self::Dominant7 => "7",
            Self::Major7 => "maj7",
            Self::Minor7 => "m7",
            Self::Diminished => "dim",
            Self::Augmented => "aug",
            Self::Suspended2 => "sus2",
            Self::Suspended4 => "sus4",
            Self::Sixth => "6",
            Self::Minor6 => "m6",
        }
    }

    /// Returns the chord's semitone intervals above the root.
    pub fn intervals(self) -> &'static [i32] {
        match self {
            Self::Major => &[4, 7],
            Self::Minor => &[3, 7],
            Self::Dominant7 => &[4, 7, 10],
            Self::Major7 => &[4, 7, 11],
            Self::Minor7 => &[3, 7, 10],
            Self::Diminished => &[3, 6],
            Self::Augmented => &[4, 8],
            Self::Suspended2 => &[2, 7],
            Self::Suspended4 => &[5, 7],
            Self::Sixth => &[4, 7, 9],
            Self::Minor6 => &[3, 7, 9],
        }
    }

    /// Returns every jazz quality, in the order tried during recognition.
    pub fn all() -> &'static [Self] {
        &[
            Self::Major,
            Self::Minor,
            Self::Dominant7,
            Self::Major7,
            Self::Minor7,
            Self::Diminished,
            Self::Augmented,
            Self::Suspended2,
            Self::Suspended4,
            Self::Sixth,
            Self::Minor6,
        ]
    }
}

/// A jazz chord symbol: a root pitch class, a [`JazzQuality`], and an optional
/// slash bass.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct JazzChordSymbol {
    /// The chord root.
    pub root: PitchClass,
    /// The chord quality.
    pub quality: JazzQuality,
    /// An optional slash bass pitch class.
    pub slash_bass: Option<PitchClass>,
}

impl JazzChordSymbol {
    /// Realizes this symbol into a concrete [`Chord`] rooted at `octave`.
    pub fn to_chord(&self, octave: i16) -> Chord {
        let root = Pitch {
            class: self.root,
            octave,
        };
        let mut chord = Chord::from_root_intervals(root, self.quality.intervals());
        if let Some(bass) = self.slash_bass {
            chord = chord.with_slash_bass(Pitch {
                class: bass,
                octave,
            });
        }
        chord
    }

    /// Returns the chord's pitch classes as a [`PitchClassMask`].
    pub fn mask(&self) -> PitchClassMask {
        self.to_chord(4).pitch_classes()
    }
}

impl fmt::Display for JazzChordSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}",
            self.root.canonical_name(),
            self.quality.suffix()
        )?;
        if let Some(bass) = self.slash_bass {
            write!(f, "/{}", bass.canonical_name())?;
        }
        Ok(())
    }
}
