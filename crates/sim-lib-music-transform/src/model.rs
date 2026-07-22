use std::collections::BTreeMap;

use num_rational::Ratio;
use thiserror::Error;

use sim_lib_music_core::{
    Articulation, AtomRef, Channel, Melody, MelodyItem, Music, MusicError, MusicObject, Note,
    PianoRoll, Rest, Time, TimedNote,
};
use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_pitch_scale::{Key, Mode, PitchScaleError, Scale};

/// Error returned by transforms that reject invalid scaling factors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransformError {
    /// The supplied time-scaling factor was zero or negative.
    #[error("transform factor must be positive")]
    InvalidFactor,
    /// A music object or transform result violated model invariants.
    #[error(transparent)]
    InvalidMusic(#[from] MusicError),
    /// A transform returned an output shape that the caller cannot use.
    #[error("{transform} transform returned invalid output: {reason}")]
    InvalidTransformOutput {
        /// Name of the transform that produced the invalid output.
        transform: &'static str,
        /// Stable explanation of the invalid condition.
        reason: &'static str,
    },
}

/// Strategy for placing notes when reversing material in time.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RetrogradeMode {
    /// Mirror each note's span about the total duration.
    Cutout,
    /// Keep the original onset grid and reverse only the note order.
    PinnedNoteOn,
}

/// Named diatonic function (mode) used to remap scale degrees onto pitches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FunctionMap {
    /// Major (Ionian) function.
    Major,
    /// Natural minor (Aeolian) function.
    MinorNatural,
    /// Harmonic minor function.
    MinorHarmonic,
    /// Ascending melodic minor function.
    MinorMelodicAsc,
    /// Dorian mode function.
    Dorian,
    /// Phrygian mode function.
    Phrygian,
    /// Lydian mode function.
    Lydian,
    /// Mixolydian mode function.
    Mixolydian,
    /// Locrian mode function.
    Locrian,
    /// User-supplied scale used directly as the function.
    Custom(Scale),
}

impl FunctionMap {
    /// Returns the stable wire/display name for this function.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Major => "Major",
            Self::MinorNatural => "MinorNatural",
            Self::MinorHarmonic => "MinorHarmonic",
            Self::MinorMelodicAsc => "MinorMelodicAsc",
            Self::Dorian => "Dorian",
            Self::Phrygian => "Phrygian",
            Self::Lydian => "Lydian",
            Self::Mixolydian => "Mixolydian",
            Self::Locrian => "Locrian",
            Self::Custom(_) => "Custom",
        }
    }

    /// Builds the concrete `Scale` this function produces for the given key.
    pub fn scale_for_key(&self, key: &Key) -> Scale {
        match self {
            Self::Major => Scale::new(key.tonic, Mode::Major),
            Self::MinorNatural => Scale::new(key.tonic, Mode::MinorNatural),
            Self::MinorHarmonic => Scale::new(key.tonic, Mode::MinorHarmonic),
            Self::MinorMelodicAsc => Scale::new(key.tonic, Mode::MinorMelodic),
            Self::Dorian => Scale::new(key.tonic, Mode::Dorian),
            Self::Phrygian => Scale::new(key.tonic, Mode::Phrygian),
            Self::Lydian => Scale::new(key.tonic, Mode::Lydian),
            Self::Mixolydian => Scale::new(key.tonic, Mode::Mixolydian),
            Self::Locrian => Scale::new(key.tonic, Mode::Locrian),
            Self::Custom(scale) => *scale,
        }
    }

    /// Resolves a scale degree to a concrete pitch at the given octave.
    pub fn degree_to_pitch(
        &self,
        degree: usize,
        key: &Key,
        octave: i16,
    ) -> Result<Pitch, PitchScaleError> {
        Ok(Pitch {
            class: self.scale_for_key(key).pitch_at_degree(degree)?,
            octave,
        })
    }
}

/// Lookup table of [`FunctionMap`] values addressed by name.
#[derive(Clone, Debug, Default)]
pub struct FunctionMapRegistry {
    maps: BTreeMap<String, FunctionMap>,
}

impl FunctionMapRegistry {
    /// Creates a registry preloaded with the built-in diatonic functions.
    pub fn new_with_builtins() -> Self {
        let mut registry = Self::default();
        for map in [
            FunctionMap::Major,
            FunctionMap::MinorNatural,
            FunctionMap::MinorHarmonic,
            FunctionMap::MinorMelodicAsc,
            FunctionMap::Dorian,
            FunctionMap::Phrygian,
            FunctionMap::Lydian,
            FunctionMap::Mixolydian,
            FunctionMap::Locrian,
        ] {
            registry.register(map);
        }
        registry
    }

    /// Inserts a function under its own name, replacing any prior entry.
    pub fn register(&mut self, map: FunctionMap) {
        self.maps.insert(map.name().to_owned(), map);
    }

    /// Looks up a function by name.
    pub fn get(&self, name: &str) -> Option<&FunctionMap> {
        self.maps.get(name)
    }

    /// Returns the names of all registered functions.
    pub fn names(&self) -> Vec<&str> {
        self.maps.keys().map(String::as_str).collect()
    }
}

/// Lengthens every onset and duration by `factor` (time augmentation).
pub fn augment(object: &dyn MusicObject, factor: Time) -> Result<Music, TransformError> {
    scale_time(object, factor)
}

/// Shortens every onset and duration by `factor` (time diminution).
pub fn diminish(object: &dyn MusicObject, factor: Time) -> Result<Music, TransformError> {
    if factor <= Time::from_integer(0) {
        return Err(TransformError::InvalidFactor);
    }
    scale_time(object, factor.recip())
}

/// Reverses the material in time using the default [`RetrogradeMode::Cutout`].
pub fn retrograde(object: &dyn MusicObject) -> Result<Music, TransformError> {
    retrograde_with_mode(object, RetrogradeMode::Cutout)
}

/// Reverses the material in time using the given [`RetrogradeMode`].
pub fn retrograde_with_mode(
    object: &dyn MusicObject,
    mode: RetrogradeMode,
) -> Result<Music, TransformError> {
    let roll = to_piano_roll(object)?;
    let total = object.duration();
    let items = match mode {
        RetrogradeMode::Cutout => roll
            .items
            .into_iter()
            .map(|item| TimedNote {
                onset: total - item.onset - item.note.duration,
                note: item.note,
            })
            .collect(),
        RetrogradeMode::PinnedNoteOn => {
            let mut onsets: Vec<Time> = roll.items.iter().map(|item| item.onset).collect();
            onsets.sort();
            let notes: Vec<Note> = roll.items.into_iter().rev().map(|item| item.note).collect();
            onsets
                .into_iter()
                .zip(notes)
                .map(|(onset, note)| TimedNote { onset, note })
                .collect()
        }
    };
    Ok(Music::PianoRoll(canonical_roll(items)?))
}

/// Mirrors note onsets about the total duration, then rebases to start at zero.
pub fn time_invert(object: &dyn MusicObject) -> Result<Music, TransformError> {
    let roll = to_piano_roll(object)?;
    if roll.items.is_empty() {
        return Ok(Music::PianoRoll(roll));
    }
    let total = object.duration();
    let mut items: Vec<TimedNote> = roll
        .items
        .into_iter()
        .map(|item| TimedNote {
            onset: total - item.onset,
            note: item.note,
        })
        .collect();
    let min_onset = items
        .iter()
        .map(|item| item.onset)
        .min()
        .unwrap_or_else(|| Time::from_integer(0));
    for item in &mut items {
        item.onset -= min_onset;
    }
    Ok(Music::PianoRoll(canonical_roll(items)?))
}

/// Repeats the material `n` times back to back along the time axis.
pub fn loop_n(object: &dyn MusicObject, n: usize) -> Result<Music, TransformError> {
    let roll = to_piano_roll(object)?;
    let span = object.duration();
    let items = (0..n)
        .flat_map(|index| {
            let offset = span * Time::from_integer(index as i64);
            roll.items.iter().cloned().map(move |mut item| {
                item.onset += offset;
                item
            })
        })
        .collect();
    Ok(Music::PianoRoll(canonical_roll(items)?))
}

/// Extracts the `[start, end)` time window, clipping note spans to the bounds.
pub fn slice(object: &dyn MusicObject, start: Time, end: Time) -> Result<Music, TransformError> {
    let roll = to_piano_roll(object)?;
    let items = roll
        .items
        .into_iter()
        .filter_map(|item| {
            let item_start = item.onset;
            let item_end = item.onset + item.note.duration;
            let clipped_start = item_start.max(start);
            let clipped_end = item_end.min(end);
            (clipped_start < clipped_end).then(|| TimedNote {
                onset: clipped_start - start,
                note: Note {
                    duration: clipped_end - clipped_start,
                    ..item.note
                },
            })
        })
        .collect();
    Ok(Music::PianoRoll(canonical_roll(items)?))
}

/// Transposes every note chromatically by the given number of semitones.
pub fn transpose(object: &dyn MusicObject, semitones: i32) -> Result<Music, TransformError> {
    map_notes(object, |note| Note {
        pitch: note.pitch.transpose(semitones),
        ..note
    })
}

/// Transposes every note by `steps` scale degrees within the given scale.
///
/// Notes outside the scale are left unchanged.
pub fn transpose_diatonic(
    object: &dyn MusicObject,
    scale: &Scale,
    steps: i32,
) -> Result<Music, TransformError> {
    map_notes(object, |note| Note {
        pitch: scale
            .transpose_diatonic(note.pitch, steps)
            .unwrap_or(note.pitch),
        ..note
    })
}

/// Inverts every note's pitch about the given axis pitch.
pub fn pitch_invert(object: &dyn MusicObject, axis: Pitch) -> Result<Music, TransformError> {
    map_notes(object, |note| Note {
        pitch: note.pitch.invert(axis),
        ..note
    })
}

/// Applies [`pitch_invert`] then [`retrograde`] (retrograde inversion).
pub fn retrograde_invert(object: &dyn MusicObject, axis: Pitch) -> Result<Music, TransformError> {
    let inverted = pitch_invert(object, axis)?;
    retrograde(&inverted)
}

/// Shifts every note by the given number of whole octaves.
pub fn shift_octave(object: &dyn MusicObject, octaves: i16) -> Result<Music, TransformError> {
    map_notes(object, |note| Note {
        pitch: note.pitch.transpose(i32::from(octaves) * 12),
        ..note
    })
}

/// Snaps every note to the nearest pitch belonging to the given scale.
pub fn chord_tones_in(object: &dyn MusicObject, scale: &Scale) -> Result<Music, TransformError> {
    map_notes(object, |note| Note {
        pitch: nearest_pitch_in_scale(note.pitch, scale),
        ..note
    })
}

/// Remaps each in-key note to the same scale degree under another function.
///
/// Notes whose pitch class is not a degree of `key` are passed through.
pub fn map_to_function(
    object: &dyn MusicObject,
    key: &Key,
    fmap: &FunctionMap,
) -> Result<Music, TransformError> {
    let source_scale = Scale::new(key.tonic, key.mode);
    map_notes(object, |note| {
        match source_scale.degree_of(note.pitch.class) {
            Some(degree) => Note {
                pitch: fmap
                    .degree_to_pitch(degree, key, note.pitch.octave)
                    .unwrap_or(note.pitch),
                ..note
            },
            None => note,
        }
    })
}

fn scale_time(object: &dyn MusicObject, factor: Time) -> Result<Music, TransformError> {
    if factor <= Time::from_integer(0) {
        return Err(TransformError::InvalidFactor);
    }
    map_roll(object, |mut item| {
        item.onset *= factor;
        item.note.duration *= factor;
        item
    })
}

pub(crate) fn map_notes(
    object: &dyn MusicObject,
    f: impl Fn(Note) -> Note,
) -> Result<Music, TransformError> {
    map_roll(object, |mut item| {
        item.note = f(item.note);
        item
    })
}

pub(crate) fn map_roll(
    object: &dyn MusicObject,
    f: impl Fn(TimedNote) -> TimedNote,
) -> Result<Music, TransformError> {
    let roll = to_piano_roll(object)?;
    let items = roll.items.into_iter().map(f).collect();
    Ok(Music::PianoRoll(canonical_roll(items)?))
}

pub(crate) fn to_piano_roll(object: &dyn MusicObject) -> Result<PianoRoll, TransformError> {
    let mut atoms = Vec::new();
    object.voices(Time::from_integer(0), &mut atoms);
    let items = atoms
        .into_iter()
        .filter_map(|atom| match atom.atom {
            AtomRef::Note(note) => Some(TimedNote {
                onset: atom.onset,
                note,
            }),
            AtomRef::Rest(_) | AtomRef::Phantom(_) => None,
        })
        .collect();
    canonical_roll(items)
}

pub(crate) fn canonical_roll(items: Vec<TimedNote>) -> Result<PianoRoll, TransformError> {
    Ok(PianoRoll::new(items)?)
}

pub(crate) fn nearest_pitch_in_scale(pitch: Pitch, scale: &Scale) -> Pitch {
    if scale.degree_of(pitch.class).is_some() {
        return pitch;
    }
    let candidates = scale
        .pitch_classes()
        .into_iter()
        .flat_map(|class| {
            [
                Pitch {
                    class,
                    octave: pitch.octave - 1,
                },
                Pitch {
                    class,
                    octave: pitch.octave,
                },
                Pitch {
                    class,
                    octave: pitch.octave + 1,
                },
            ]
        })
        .collect::<Vec<_>>();
    candidates
        .into_iter()
        .min_by_key(|candidate| {
            (
                (candidate.semitone() - pitch.semitone()).abs(),
                candidate.semitone(),
            )
        })
        .unwrap_or(pitch)
}

/// Builds a single-voice `Melody` from `(MIDI key, duration)` pairs.
///
/// Each note uses velocity 100, channel 0, and normal articulation. Intended
/// as a test and example helper.
pub fn simple_melody(items: &[(u8, Time)]) -> Melody {
    Melody::new(
        items
            .iter()
            .map(|(midi, duration)| {
                MelodyItem::Note(
                    Note::new(
                        *duration,
                        Pitch::from_midi(*midi),
                        100,
                        Channel::new(0).expect("channel"),
                        Articulation::Normal,
                    )
                    .expect("note"),
                )
            })
            .collect(),
    )
    .expect("melody")
}

/// Builds a `Rest` spanning the given duration.
pub fn silence(duration: Time) -> Rest {
    Rest::new(duration).expect("rest")
}

/// Returns the `Time` value for one quarter note (1/4).
pub fn quarter() -> Time {
    Ratio::new(1, 4)
}

/// Returns the canonical name of a pitch class.
pub fn pitch_class_name(class: PitchClass) -> &'static str {
    class.canonical_name()
}
