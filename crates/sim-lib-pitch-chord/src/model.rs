use thiserror::Error;

use sim_lib_pitch_core::{Pitch, PitchClass, parse_pitch};
use sim_lib_pitch_scale::Scale;
use sim_lib_pitch_set::{BitChord, PitchClassMask};

/// Error returned by chord construction, parsing, and sequencer operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PitchChordError {
    /// A chord symbol used a root or quality that the parser does not support.
    #[error("unsupported chord symbol")]
    UnsupportedChordSymbol,
    /// A chord was requested with a note count of zero.
    #[error("note count must be greater than zero")]
    InvalidNoteCount,
    /// A scale degree outside the scale's valid range was supplied.
    #[error("scale degree {0} is outside this scale")]
    InvalidScaleDegree(usize),
    /// A degree-to-chord map was supplied with no entries.
    #[error("degree chord map must contain at least one entry")]
    EmptyDegreeChordMap,
    /// A chord progression was built with no slots.
    #[error("progression must contain at least one slot")]
    EmptyProgression,
    /// A sequencer slot was given a duration of zero ticks.
    #[error("slot duration must be greater than zero")]
    InvalidSlotDuration,
    /// A wire string could not be parsed back into a chord sequencer configuration.
    #[error("invalid chord sequencer wire data")]
    InvalidProgressionWire,
}

/// A concrete chord: an ordered list of sounding pitches with an optional slash bass.
///
/// The first note is treated as the chord root for analyses such as
/// [`Chord::bit_chord`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    /// The chord's notes, with the root conventionally first.
    pub notes: Vec<Pitch>,
    /// An optional bass note sounding below the chord (a slash chord).
    pub slash_bass: Option<Pitch>,
}

impl Chord {
    /// Constructs a chord from an explicit list of notes and no slash bass.
    pub fn new(notes: Vec<Pitch>) -> Self {
        Self {
            notes,
            slash_bass: None,
        }
    }

    /// Builds a chord from a `root` pitch and a list of semitone `intervals` above
    /// it; the root itself is always included.
    pub fn from_root_intervals(root: Pitch, intervals: &[i32]) -> Self {
        let mut notes = Vec::with_capacity(intervals.len() + 1);
        notes.push(root);
        notes.extend(intervals.iter().map(|interval| root.transpose(*interval)));
        Self::new(notes)
    }

    /// Returns all sounding pitches, including any slash bass, sorted ascending.
    pub fn pitches(&self) -> Vec<Pitch> {
        let mut notes = self.notes.clone();
        if let Some(bass) = self.slash_bass {
            notes.push(bass);
        }
        notes.sort_unstable();
        notes
    }

    /// Returns the chord's pitch classes as a [`PitchClassMask`].
    pub fn pitch_classes(&self) -> PitchClassMask {
        let classes: Vec<_> = self
            .pitches()
            .into_iter()
            .map(|pitch| pitch.class)
            .collect();
        PitchClassMask::from_pitch_classes(&classes)
    }

    /// Returns the chord as a rooted [`BitChord`], using the first note as the root.
    pub fn bit_chord(&self) -> BitChord {
        BitChord {
            mask: self.pitch_classes(),
            root: self.notes.first().map(|pitch| pitch.class),
        }
    }

    /// Returns the chord with `count` inversions applied, each raising the lowest
    /// note by an octave.
    pub fn invert(&self, count: usize) -> Self {
        let mut notes = self.notes.clone();
        if notes.is_empty() {
            return self.clone();
        }
        for _ in 0..count {
            notes.sort_unstable();
            let lowest = notes.remove(0).transpose(12);
            notes.push(lowest);
        }
        Self {
            notes,
            slash_bass: self.slash_bass,
        }
    }

    /// Returns the chord transposed by `semitones`, including any slash bass.
    pub fn transpose(&self, semitones: i32) -> Self {
        Self {
            notes: self
                .notes
                .iter()
                .map(|pitch| pitch.transpose(semitones))
                .collect(),
            slash_bass: self.slash_bass.map(|pitch| pitch.transpose(semitones)),
        }
    }

    /// Returns a closed voicing, collapsing any gaps larger than an octave between
    /// adjacent notes.
    pub fn closed_voicing(&self) -> Self {
        let mut notes = self.notes.clone();
        notes.sort_unstable();
        for index in 1..notes.len() {
            while notes[index].semitone() - notes[index - 1].semitone() > 12 {
                notes[index] = notes[index].transpose(-12);
            }
        }
        Self {
            notes,
            slash_bass: self.slash_bass,
        }
    }

    /// Returns the chord with `bass` set as its slash bass note.
    pub fn with_slash_bass(mut self, bass: Pitch) -> Self {
        self.slash_bass = Some(bass);
        self
    }

    /// Builds the diatonic triad rooted on `degree` of `scale`, at `root_octave`,
    /// by stacking diatonic thirds.
    pub fn chord_tones_in(
        scale: Scale,
        degree: usize,
        root_octave: i16,
    ) -> Result<Self, PitchChordError> {
        let class = scale
            .pitch_at_degree(degree)
            .map_err(|_| PitchChordError::InvalidScaleDegree(degree))?;
        let root = Pitch {
            class,
            octave: root_octave,
        };
        let third = scale
            .transpose_diatonic(root, 2)
            .unwrap_or(root.transpose(4));
        let fifth = scale
            .transpose_diatonic(root, 4)
            .unwrap_or(root.transpose(7));
        Ok(Self::new(vec![root, third, fifth]))
    }
}

/// A parsed chord symbol: a root pitch class, a quality token, and an optional
/// slash bass.
///
/// The supported qualities are the empty string / `maj`, `m`, `7`, `m7`, `maj7`,
/// and `dim`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChordSymbol {
    /// The chord root.
    pub root: PitchClass,
    /// The chord quality token (for example `"maj"`, `"m7"`, `"dim"`).
    pub quality: &'static str,
    /// An optional slash bass pitch class.
    pub slash_bass: Option<PitchClass>,
}

impl ChordSymbol {
    /// Parses a chord symbol such as `"C"`, `"Am7"`, or `"G7/B"`.
    ///
    /// Returns [`PitchChordError::UnsupportedChordSymbol`] for an unrecognized root
    /// or quality.
    pub fn parse(value: &str) -> Result<Self, PitchChordError> {
        let (head, slash_bass) = match value.split_once('/') {
            Some((head, bass)) => (
                head,
                Some(
                    parse_pitch(&format!("{bass}4"))
                        .map_err(|_| PitchChordError::UnsupportedChordSymbol)?
                        .class,
                ),
            ),
            None => (value, None),
        };
        if head.is_empty() {
            return Err(PitchChordError::UnsupportedChordSymbol);
        }
        let root_len = if head
            .as_bytes()
            .get(1)
            .is_some_and(|byte| matches!(*byte, b'#' | b'b' | b's'))
        {
            2
        } else {
            1
        };
        let root = parse_pitch(&format!("{}4", &head[..root_len]))
            .map_err(|_| PitchChordError::UnsupportedChordSymbol)?
            .class;
        let quality = match &head[root_len..] {
            "" => "maj",
            "m" => "m",
            "7" => "7",
            "m7" => "m7",
            "maj7" => "maj7",
            "dim" => "dim",
            _ => return Err(PitchChordError::UnsupportedChordSymbol),
        };
        Ok(Self {
            root,
            quality,
            slash_bass,
        })
    }

    /// Realizes this symbol into a concrete [`Chord`] rooted at `octave`.
    pub fn to_chord(&self, octave: i16) -> Chord {
        let root = Pitch {
            class: self.root,
            octave,
        };
        let intervals: &[i32] = match self.quality {
            "maj" => &[4, 7],
            "m" => &[3, 7],
            "7" => &[4, 7, 10],
            "m7" => &[3, 7, 10],
            "maj7" => &[4, 7, 11],
            "dim" => &[3, 6],
            _ => &[],
        };
        let mut chord = Chord::from_root_intervals(root, intervals);
        if let Some(bass) = self.slash_bass {
            chord = chord.with_slash_bass(Pitch {
                class: bass,
                octave,
            });
        }
        chord
    }

    /// Returns the canonical text label for this symbol (for example `"Am7/C"`).
    pub fn wire_label(&self) -> String {
        let quality = match self.quality {
            "maj" => "",
            "m" => "m",
            "7" => "7",
            "m7" => "m7",
            "maj7" => "maj7",
            "dim" => "dim",
            _ => "?",
        };
        let mut label = format!("{}{}", self.root.canonical_name(), quality);
        if let Some(bass) = self.slash_bass {
            label.push('/');
            label.push_str(bass.canonical_name());
        }
        label
    }
}
