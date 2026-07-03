use num_rational::Ratio;
use std::any::Any;
use thiserror::Error;

pub use sim_lib_midi_core::{Channel, ChannelMessage, MidiEvent, MidiPayload, TickTime};
pub use sim_lib_midi_smf::SmfFile;
pub use sim_lib_pitch_core::{Pitch, PitchClass, PitchError, parse_pitch};

use crate::{arranger::Arranger, piano_roll::PianoRoll};

/// Exact musical time measured in whole notes as a rational number of beats.
///
/// Durations and onsets throughout the model are expressed in this type so that
/// tuplets and subdivisions stay exact rather than accumulating float drift.
pub type Time = Ratio<i64>;

/// Error returned when a musical value violates a model invariant.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MusicError {
    /// A duration was negative.
    #[error("duration cannot be negative")]
    NegativeDuration,
    /// An onset position was negative.
    #[error("onset cannot be negative")]
    NegativeOnset,
    /// A tempo was zero or otherwise non-positive.
    #[error("tempo must be positive")]
    InvalidTempo,
    /// A time signature had a zero denominator.
    #[error("time signature denominator must be non-zero")]
    InvalidTimeSignature,
    /// A melody contained overlapping voices instead of a single line.
    #[error("melody items must be monophonic")]
    NonMonophonicMelody,
    /// A play range ended before it started.
    #[error("play range end cannot precede start")]
    InvalidTimeRange,
    /// A play pulses-per-quarter resolution was zero.
    #[error("play PPQ must be greater than zero")]
    InvalidPpq,
    /// A lane targeted something incompatible with its event kind.
    #[error("lane {lane} target {target} is invalid for its event kind")]
    InvalidLaneTarget {
        /// Name of the offending lane.
        lane: String,
        /// The invalid target the lane referenced.
        target: String,
    },
    /// A piano-roll grid had a non-positive ticks-per-quarter or step.
    #[error("piano-roll time grid must have positive TPQ and step")]
    InvalidPianoRollGrid,
    /// A piano-roll lane held a cell kind it cannot accept.
    #[error("piano-roll lane {lane} of kind {lane_kind} cannot contain {cell_kind} cells")]
    PianoRollLaneCellMismatch {
        /// Name of the offending lane.
        lane: String,
        /// Kind of the lane.
        lane_kind: String,
        /// Kind of the cell that did not fit the lane.
        cell_kind: String,
    },
}

/// A playable musical structure that can report its span and flatten to atoms.
///
/// Implementors are the concrete node types ([Note], [Rest], [Par], [Seq], and
/// the larger forms) that compose a [Music] tree.
pub trait MusicObject: Send + Sync + Any {
    /// Returns a stable, human-readable tag for this object kind.
    fn kind(&self) -> &'static str;
    /// Returns the total duration this object occupies.
    fn duration(&self) -> Time;
    /// Appends the timed atoms produced by this object, shifted by `offset`.
    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>);
    /// Clones this object into a fresh boxed trait object.
    fn clone_box(&self) -> Box<dyn MusicObject>;
    /// Returns this object as a `&dyn Any` for downcasting.
    fn as_any(&self) -> &dyn Any;
}

impl Clone for Box<dyn MusicObject> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// A single flattened atom together with its absolute onset time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimedAtom<'a> {
    /// Absolute time at which the atom begins.
    pub onset: Time,
    /// The atom sounding (or resting) at this onset.
    pub atom: AtomRef<'a>,
}

/// A leaf event produced when a [MusicObject] is flattened into voices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AtomRef<'a> {
    /// A sounding note.
    Note(Note),
    /// A silent rest.
    Rest(Rest),
    /// Carries the borrow lifetime when no owned atom is present.
    Phantom(std::marker::PhantomData<&'a ()>),
}

/// Performance articulation applied to a note.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Articulation {
    /// Default articulation with no modification.
    Normal,
    /// Shortened, detached note.
    Staccato,
    /// Smoothly connected to the following note.
    Legato,
    /// Held to its full notated length.
    Tenuto,
    /// Emphasized with extra attack.
    Accent,
    /// Strongly accented and detached.
    Marcato,
}

/// A single pitched note with timing, dynamics, and articulation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// Sounding length of the note.
    pub duration: Time,
    /// Pitch the note sounds.
    pub pitch: Pitch,
    /// MIDI velocity (0-127).
    pub velocity: u8,
    /// MIDI channel the note plays on.
    pub channel: Channel,
    /// Articulation applied to the note.
    pub articulation: Articulation,
}

impl Note {
    /// Builds a note, validating that `duration` is non-negative.
    pub fn new(
        duration: Time,
        pitch: Pitch,
        velocity: u8,
        channel: Channel,
        articulation: Articulation,
    ) -> Result<Self, MusicError> {
        ensure_non_negative(duration)?;
        Ok(Self {
            duration,
            pitch,
            velocity,
            channel,
            articulation,
        })
    }
}

/// A span of silence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rest {
    /// Length of the silence.
    pub duration: Time,
}

impl Rest {
    /// Builds a rest, validating that `duration` is non-negative.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_music_core::{Rest, Time};
    ///
    /// let rest = Rest::new(Time::from_integer(1)).unwrap();
    /// assert_eq!(rest.duration, Time::from_integer(1));
    /// ```
    pub fn new(duration: Time) -> Result<Self, MusicError> {
        ensure_non_negative(duration)?;
        Ok(Self { duration })
    }
}

/// Parallel composition: children that sound simultaneously.
#[derive(Clone)]
pub struct Par {
    /// The objects played at the same time.
    pub children: Vec<Box<dyn MusicObject>>,
}

impl std::fmt::Debug for Par {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Par")
            .field("children_len", &self.children.len())
            .finish()
    }
}

/// Sequential composition: children that play one after another.
#[derive(Clone)]
pub struct Seq {
    /// The objects played back to back in order.
    pub children: Vec<Box<dyn MusicObject>>,
}

impl std::fmt::Debug for Seq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Seq")
            .field("children_len", &self.children.len())
            .finish()
    }
}

/// A set of pitches sounded together, optionally tagged with a chord symbol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chord {
    /// Sounding length of the chord.
    pub duration: Time,
    /// Chord symbol label (for example `"Cmaj7"`).
    pub symbol: String,
    /// Pitches that make up the chord.
    pub pitches: Vec<Pitch>,
    /// MIDI velocity (0-127) applied to every pitch.
    pub velocity: u8,
    /// MIDI channel the chord plays on.
    pub channel: Channel,
}

impl Chord {
    /// Builds a chord, validating that `duration` is non-negative.
    pub fn new(
        duration: Time,
        symbol: impl Into<String>,
        pitches: Vec<Pitch>,
        velocity: u8,
        channel: Channel,
    ) -> Result<Self, MusicError> {
        ensure_non_negative(duration)?;
        Ok(Self {
            duration,
            symbol: symbol.into(),
            pitches,
            velocity,
            channel,
        })
    }
}

/// One element of a monophonic [Melody]: either a note or a rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MelodyItem {
    /// A sounding note.
    Note(Note),
    /// A silent rest.
    Rest(Rest),
}

impl MelodyItem {
    /// Returns the duration of this item, whether note or rest.
    pub fn duration(&self) -> Time {
        match self {
            Self::Note(note) => note.duration,
            Self::Rest(rest) => rest.duration,
        }
    }
}

/// A single monophonic line of notes and rests in sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Melody {
    /// The ordered notes and rests of the line.
    pub items: Vec<MelodyItem>,
}

impl Melody {
    /// Builds a melody, validating that every item duration is non-negative.
    pub fn new(items: Vec<MelodyItem>) -> Result<Self, MusicError> {
        for item in &items {
            ensure_non_negative(item.duration())?;
        }
        Ok(Self { items })
    }

    /// Returns the summed duration of all items in the line.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_music_core::{Melody, MelodyItem, Rest, Time};
    ///
    /// let melody = Melody::new(vec![
    ///     MelodyItem::Rest(Rest::new(Time::from_integer(1)).unwrap()),
    ///     MelodyItem::Rest(Rest::new(Time::from_integer(2)).unwrap()),
    /// ])
    /// .unwrap();
    /// assert_eq!(melody.total_duration(), Time::from_integer(3));
    /// ```
    pub fn total_duration(&self) -> Time {
        self.items
            .iter()
            .fold(Time::from_integer(0), |sum, item| sum + item.duration())
    }
}

/// An ordered sequence of chords, optionally in a named key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progression {
    /// Key the progression is heard in, if specified.
    pub key: Option<String>,
    /// The chords in playing order.
    pub chords: Vec<Chord>,
}

impl Progression {
    /// Builds a progression, validating that every chord duration is non-negative.
    pub fn new(key: Option<String>, chords: Vec<Chord>) -> Result<Self, MusicError> {
        for chord in &chords {
            ensure_non_negative(chord.duration)?;
        }
        Ok(Self { key, chords })
    }
}

/// Several independent melodic lines sounding together with labels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Counterpoint {
    /// The independent melodic voices.
    pub voices: Vec<Melody>,
    /// Display names for each voice, parallel to `voices`.
    pub voice_names: Vec<String>,
}

impl Counterpoint {
    /// Builds counterpoint, supplying default voice names when the counts mismatch.
    pub fn new(voices: Vec<Melody>, voice_names: Vec<String>) -> Result<Self, MusicError> {
        let voice_names = normalize_voice_names(voices.len(), voice_names);
        Ok(Self {
            voices,
            voice_names,
        })
    }

    /// Returns voice names, filling in defaults when the stored list is wrong-length.
    pub fn normalized_voice_names(&self) -> Vec<String> {
        normalize_voice_names(self.voices.len(), self.voice_names.clone())
    }
}

/// A raw MIDI track wrapped as a music object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiTrackObj {
    /// The MIDI events of the track.
    pub events: Vec<MidiEvent>,
    /// Preferred channel for events that do not carry one.
    pub channel_hint: Option<Channel>,
}

impl MidiTrackObj {
    /// Wraps a list of MIDI events with an optional channel hint.
    pub fn new(events: Vec<MidiEvent>, channel_hint: Option<Channel>) -> Self {
        Self {
            events,
            channel_hint,
        }
    }
}

/// A complete Standard MIDI File wrapped as a music object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiFileObj {
    /// The parsed SMF contents.
    pub file: SmfFile,
}

impl MidiFileObj {
    /// Wraps a parsed Standard MIDI File.
    pub fn new(file: SmfFile) -> Self {
        Self { file }
    }
}

/// A top-level score: global settings plus a musical body.
#[derive(Clone, Debug)]
pub struct Score {
    /// Tempo in beats per minute.
    pub tempo_bpm: u32,
    /// Time signature as a (numerator, denominator) pair.
    pub time_signature: (u8, u8),
    /// Key the score is in, if specified.
    pub key: Option<String>,
    /// The musical content of the score.
    pub body: Music,
}

impl Score {
    /// Builds a score, validating positive tempo and a non-zero time signature.
    pub fn new(
        tempo_bpm: u32,
        time_signature: (u8, u8),
        key: Option<String>,
        body: Music,
    ) -> Result<Self, MusicError> {
        if tempo_bpm == 0 {
            return Err(MusicError::InvalidTempo);
        }
        if time_signature.1 == 0 {
            return Err(MusicError::InvalidTimeSignature);
        }
        Ok(Self {
            tempo_bpm,
            time_signature,
            key,
            body,
        })
    }
}

/// The unified musical value: any node that can appear in a [Score] body.
#[derive(Clone)]
pub enum Music {
    /// A single note.
    Note(Note),
    /// A rest.
    Rest(Rest),
    /// Parallel composition of children.
    Par(Par),
    /// Sequential composition of children.
    Seq(Seq),
    /// A chord.
    Chord(Chord),
    /// A monophonic melody line.
    Melody(Melody),
    /// A chord progression.
    Progression(Progression),
    /// Multiple voices in counterpoint.
    Counterpoint(Counterpoint),
    /// A piano-roll grid.
    PianoRoll(PianoRoll),
    /// An arranger timeline.
    Arranger(Arranger),
    /// A raw MIDI track.
    MidiTrack(MidiTrackObj),
    /// A complete MIDI file.
    MidiFile(MidiFileObj),
}

impl std::fmt::Debug for Music {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Note(_) => f.write_str("Music::Note(..)"),
            Self::Rest(_) => f.write_str("Music::Rest(..)"),
            Self::Par(par) => f.debug_tuple("Music::Par").field(par).finish(),
            Self::Seq(seq) => f.debug_tuple("Music::Seq").field(seq).finish(),
            Self::Chord(chord) => f.debug_tuple("Music::Chord").field(chord).finish(),
            Self::Melody(melody) => f.debug_tuple("Music::Melody").field(melody).finish(),
            Self::Progression(progression) => f
                .debug_tuple("Music::Progression")
                .field(progression)
                .finish(),
            Self::Counterpoint(counterpoint) => f
                .debug_tuple("Music::Counterpoint")
                .field(counterpoint)
                .finish(),
            Self::PianoRoll(roll) => f.debug_tuple("Music::PianoRoll").field(roll).finish(),
            Self::Arranger(arranger) => f.debug_tuple("Music::Arranger").field(arranger).finish(),
            Self::MidiTrack(track) => f.debug_tuple("Music::MidiTrack").field(track).finish(),
            Self::MidiFile(file) => f.debug_tuple("Music::MidiFile").field(file).finish(),
        }
    }
}

pub(crate) fn ensure_non_negative(value: Time) -> Result<(), MusicError> {
    if value < Time::from_integer(0) {
        Err(MusicError::NegativeDuration)
    } else {
        Ok(())
    }
}

fn normalize_voice_names(count: usize, voice_names: Vec<String>) -> Vec<String> {
    if voice_names.len() == count {
        voice_names
    } else {
        (0..count)
            .map(|index| format!("Voice {}", index + 1))
            .collect()
    }
}
