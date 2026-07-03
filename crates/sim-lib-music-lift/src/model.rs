use num_rational::Ratio;
use thiserror::Error;

use sim_kernel::Diagnostic;
use sim_lib_midi_smf::SmfFile;
use sim_lib_music_analysis::{ChordWindowMode, DiffRoll};
use sim_lib_music_core::{Counterpoint, PianoRoll, Progression, Time};
use sim_lib_pitch_scale::Key;

use crate::collect::collect_midi;
use crate::counterpoint::lift_counterpoint_impl;
use crate::progression::lift_progression_impl;

/// Error raised while lifting MIDI into a higher-level music representation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LiftError {
    /// The progression grid duration was not positive.
    #[error("grid must be positive")]
    InvalidGrid,
    /// The minimum-notes-per-chord threshold was not positive.
    #[error("minimum notes must be positive")]
    InvalidMinNotes,
    /// The minimum rest-to-close threshold was negative.
    #[error("minimum rest threshold must be non-negative")]
    InvalidRestThreshold,
    /// The per-track voice cap was not positive.
    #[error("max voices per track must be positive")]
    InvalidVoiceLimit,
    /// A construction error surfaced from `sim-lib-music-core`.
    #[error(transparent)]
    Music(#[from] sim_lib_music_core::MusicError),
}

/// A lifted value paired with diagnostics describing lossy or ambiguous choices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiftReport<T> {
    /// The lifted music value.
    pub value: T,
    /// Diagnostics emitted while producing [`value`](Self::value).
    pub diagnostics: Vec<Diagnostic>,
}

impl<T> LiftReport<T> {
    /// Maps the lifted value through `f`, preserving the diagnostics.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> LiftReport<U> {
        LiftReport {
            value: f(self.value),
            diagnostics: self.diagnostics,
        }
    }
}

/// A lifter that raises a parsed MIDI file into a higher-level music value.
pub trait MidiLifter {
    /// The higher-level representation produced by this lifter.
    type Out;

    /// Returns the stable lifter symbol used for registration and tracing.
    fn symbol(&self) -> &'static str;

    /// Lifts `file`, returning the value together with its diagnostics.
    fn lift_report(&self, file: &SmfFile) -> Result<LiftReport<Self::Out>, LiftError>;

    /// Lifts `file` and returns only the value, discarding diagnostics.
    fn lift(&self, file: &SmfFile) -> Result<Self::Out, LiftError> {
        Ok(self.lift_report(file)?.value)
    }
}

/// Chord-label selection policy for the progression lifter.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LabelStrategy {
    /// Roman-numeral functional labels relative to the key.
    Functional,
    /// Jazz chord-symbol labels.
    JazzChord,
    /// Pitch-class set-class labels.
    SetClass,
}

/// Options controlling the MIDI-to-progression lift.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressionLiftOpts {
    /// Quantization grid duration for chord windows.
    pub grid: Time,
    /// Minimum simultaneous notes required to emit a chord.
    pub min_notes: usize,
    /// Optional key hint guiding functional labeling.
    pub key_hint: Option<Key>,
    /// Strategy used to label each detected chord.
    pub label_strategy: LabelStrategy,
    /// Window mode selecting sounding vs starting notes.
    pub window_mode: ChordWindowMode,
}

impl Default for ProgressionLiftOpts {
    fn default() -> Self {
        Self {
            grid: Ratio::new(1, 16),
            min_notes: 2,
            key_hint: None,
            label_strategy: LabelStrategy::JazzChord,
            window_mode: ChordWindowMode::SoundingNotes,
        }
    }
}

/// Voice-splitting policy for the counterpoint lifter.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VoiceAssignment {
    /// Separate voices by MIDI channel only.
    ChannelOnly,
    /// Separate voices by track first, then channel.
    TrackThenChannel,
    /// Assign overlapping notes highest pitch first.
    HighestFirst,
    /// Assign overlapping notes lowest pitch first.
    LowestFirst,
}

/// Options controlling the MIDI-to-counterpoint lift.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CounterpointLiftOpts {
    /// Minimum rest duration that closes an active voice.
    pub min_rest_to_close: Time,
    /// Maximum number of voices extracted per track.
    pub max_voices_per_track: usize,
    /// Policy used to assign notes to voices.
    pub voice_assignment: VoiceAssignment,
}

impl Default for CounterpointLiftOpts {
    fn default() -> Self {
        Self {
            min_rest_to_close: Ratio::new(1, 64),
            max_voices_per_track: 8,
            voice_assignment: VoiceAssignment::HighestFirst,
        }
    }
}

/// Lifter producing a `PianoRoll` of timed notes from a MIDI file.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct MidiToPianoRoll;

impl MidiLifter for MidiToPianoRoll {
    type Out = PianoRoll;

    fn symbol(&self) -> &'static str {
        "music:MidiToPianoRoll"
    }

    fn lift_report(&self, file: &SmfFile) -> Result<LiftReport<Self::Out>, LiftError> {
        let collected = collect_midi(file);
        Ok(LiftReport {
            value: collected.to_piano_roll()?,
            diagnostics: collected.diagnostics,
        })
    }
}

/// Lifter producing a `DiffRoll` note-boundary analysis view from a MIDI file.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct MidiToDiffRoll;

impl MidiLifter for MidiToDiffRoll {
    type Out = DiffRoll;

    fn symbol(&self) -> &'static str {
        "music:MidiToDiffRoll"
    }

    fn lift_report(&self, file: &SmfFile) -> Result<LiftReport<Self::Out>, LiftError> {
        let report = MidiToPianoRoll.lift_report(file)?;
        Ok(report.map(|roll| DiffRoll::from_piano_roll(&roll)))
    }
}

/// Lifter producing a chord `Progression` from a MIDI file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MidiToProgression {
    /// Options controlling chord detection and labeling.
    pub opts: ProgressionLiftOpts,
}

impl MidiLifter for MidiToProgression {
    type Out = Progression;

    fn symbol(&self) -> &'static str {
        "music:MidiToProgression"
    }

    fn lift_report(&self, file: &SmfFile) -> Result<LiftReport<Self::Out>, LiftError> {
        lift_progression_impl(file, &self.opts)
    }
}

/// Lifter producing a `Counterpoint` of separated voices from a MIDI file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MidiToCounterpoint {
    /// Options controlling voice splitting.
    pub opts: CounterpointLiftOpts,
}

impl MidiLifter for MidiToCounterpoint {
    type Out = Counterpoint;

    fn symbol(&self) -> &'static str {
        "music:MidiToCounterpoint"
    }

    fn lift_report(&self, file: &SmfFile) -> Result<LiftReport<Self::Out>, LiftError> {
        lift_counterpoint_impl(file, &self.opts)
    }
}

/// Lifts a MIDI file to a `PianoRoll`, discarding diagnostics.
pub fn lift_to_piano_roll(file: &SmfFile) -> Result<PianoRoll, LiftError> {
    MidiToPianoRoll.lift(file)
}

/// Lifts a MIDI file to a `PianoRoll` with diagnostics.
pub fn lift_to_piano_roll_report(file: &SmfFile) -> Result<LiftReport<PianoRoll>, LiftError> {
    MidiToPianoRoll.lift_report(file)
}

/// Lifts a MIDI file to a `DiffRoll`, discarding diagnostics.
pub fn lift_to_diff_roll(file: &SmfFile) -> Result<DiffRoll, LiftError> {
    MidiToDiffRoll.lift(file)
}

/// Lifts a MIDI file to a `DiffRoll` with diagnostics.
pub fn lift_to_diff_roll_report(file: &SmfFile) -> Result<LiftReport<DiffRoll>, LiftError> {
    MidiToDiffRoll.lift_report(file)
}

/// Lifts a MIDI file to a chord `Progression`, discarding diagnostics.
pub fn lift_to_progression(
    file: &SmfFile,
    opts: ProgressionLiftOpts,
) -> Result<Progression, LiftError> {
    MidiToProgression { opts }.lift(file)
}

/// Lifts a MIDI file to a chord `Progression` with diagnostics.
pub fn lift_to_progression_report(
    file: &SmfFile,
    opts: ProgressionLiftOpts,
) -> Result<LiftReport<Progression>, LiftError> {
    MidiToProgression { opts }.lift_report(file)
}

/// Lifts a MIDI file to a `Counterpoint`, discarding diagnostics.
pub fn lift_to_counterpoint(
    file: &SmfFile,
    opts: CounterpointLiftOpts,
) -> Result<Counterpoint, LiftError> {
    MidiToCounterpoint { opts }.lift(file)
}

/// Lifts a MIDI file to a `Counterpoint` with diagnostics.
pub fn lift_to_counterpoint_report(
    file: &SmfFile,
    opts: CounterpointLiftOpts,
) -> Result<LiftReport<Counterpoint>, LiftError> {
    MidiToCounterpoint { opts }.lift_report(file)
}
