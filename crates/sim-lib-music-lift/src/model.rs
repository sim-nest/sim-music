use num_rational::Ratio;
use thiserror::Error;

use sim_kernel::Diagnostic;
use sim_lib_midi_core::MidiTempoMap;
use sim_lib_midi_smf::SmfFile;
use sim_lib_music_analysis::{ChordWindowMode, DiffRoll};
use sim_lib_music_core::{Counterpoint, Note, PianoRoll, Progression, Time};
use sim_lib_pitch_scale::Key;

use crate::collect::collect_midi;
use crate::counterpoint::lift_counterpoint_impl;
use crate::progression::lift_progression_impl;
use crate::realize::realize_midi_impl;

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
    /// A MIDI timing or tempo-map conversion failed.
    #[error(transparent)]
    Midi(#[from] sim_lib_midi_core::MidiError),
    /// MIDI-to-music lifting requires metrical timing; SMPTE input needs an
    /// explicit real-time-to-metrical adapter.
    #[error("MIDI music realization requires metrical timing")]
    MetricalTimingRequired,
    /// A format-2 file has independent patterns, but the single-timeline lift
    /// entry point was used.
    #[error("SMF format 2 realization requires explicit pattern selection")]
    IndependentPatternsRequireSelection,
    /// A requested format-2 pattern index did not exist.
    #[error("SMF format 2 pattern {pattern} is outside 0..{patterns}")]
    PatternOutOfRange {
        /// Requested track-local pattern index.
        pattern: usize,
        /// Available pattern count.
        patterns: usize,
    },
    /// Reject-overlap policy encountered two held note-ons for one channel and
    /// key.
    #[error("overlapping MIDI note rejected at tick {tick}: channel {channel}, key {key}")]
    OverlappingNote {
        /// Tick of the overlapping attack.
        tick: i64,
        /// MIDI channel.
        channel: u8,
        /// MIDI key.
        key: u8,
    },
    /// Reject-dangling policy found held notes at the end of a timeline.
    #[error("MIDI timeline ended with {count} unmatched note-on event(s)")]
    DanglingNotes {
        /// Number of still-sounding note instances.
        count: usize,
    },
}

/// How note-offs pair with overlapping note-ons sharing channel and key.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum OverlapPolicy {
    /// Close the earliest unmatched key-down note.
    #[default]
    Fifo,
    /// Close the latest unmatched key-down note.
    Lifo,
    /// Reject a second key-down note-on for the same channel and key.
    Reject,
}

/// How events encoded at one exact tick are ordered before realization.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum SameTickPolicy {
    /// Preserve track/event order from the parsed file.
    Encoded,
    /// Process note-offs, then controllers/metadata, then note-ons.
    #[default]
    NoteOffsFirst,
    /// Process note-ons, then controllers/metadata, then note-offs.
    NoteOnsFirst,
}

/// What to do with note-ons still sounding at the end of a timeline.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum DanglingNotePolicy {
    /// Close notes at the timeline end and emit one diagnostic per note.
    #[default]
    CloseAtEnd,
    /// Reject the realization.
    Reject,
}

/// Which hold-pedal controllers extend note sounding time.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PedalPolicy {
    /// Ignore sustain and sostenuto for note duration.
    Ignore,
    /// Realize sustain but leave sostenuto as an uninterpreted control cell.
    Sustain,
    /// Realize both sustain and sostenuto according to MIDI channel state.
    #[default]
    SustainAndSostenuto,
}

/// Policy bundle controlling deterministic MIDI note realization.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct MidiRealizationPolicy {
    /// Same-pitch note-on/note-off pairing policy.
    pub overlap: OverlapPolicy,
    /// Ordering policy for events sharing one exact tick.
    pub same_tick: SameTickPolicy,
    /// End-of-timeline unmatched-note policy.
    pub dangling_notes: DanglingNotePolicy,
    /// Hold-pedal interpretation policy.
    pub pedals: PedalPolicy,
}

/// Identity of one shared or independent MIDI performance timeline.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MidiTimelineId {
    /// Formats 0 and 1 share one timeline.
    Shared,
    /// Format-2 track-local pattern at this source track index.
    Pattern(usize),
}

/// Stable identity assigned to one note-on event during MIDI realization.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MidiNoteId {
    /// Timeline containing the note.
    pub timeline: MidiTimelineId,
    /// Source SMF track index.
    pub track: usize,
    /// Source event index within that track.
    pub event_index: usize,
}

/// Event that finally ended a realized MIDI note's sounding interval.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MidiNoteEnd {
    /// An ordinary note-off (including velocity-zero note-on).
    NoteOff,
    /// MIDI All Notes Off released the note with no active hold pedal.
    AllNotesOff,
    /// MIDI All Sound Off silenced the note immediately.
    AllSoundOff,
    /// Sustain release ended a previously key-released note.
    SustainRelease,
    /// Sostenuto release ended a captured, key-released note.
    SostenutoRelease,
    /// Reset All Controllers released a pedal-held note.
    ResetControllers,
    /// The configured dangling-note policy closed the note at timeline end.
    EndOfTimeline,
}

/// One identity-bearing note after overlap and pedal realization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealizedMidiNote {
    /// Stable identity derived from the source note-on.
    pub id: MidiNoteId,
    /// Exact onset in whole-note units.
    pub onset: Time,
    /// Exact key-release time, when a note-off or All Notes Off was observed.
    pub key_release: Option<Time>,
    /// Exact half-open end of the sounding interval.
    pub sounding_until: Time,
    /// Velocity from the matching note-off, or zero for channel-mode endings.
    pub release_velocity: u8,
    /// Musical note payload whose duration equals the sounding interval.
    pub note: Note,
    /// Event that finally ended the sounding interval.
    pub ended_by: MidiNoteEnd,
}

/// One realized shared timeline or independent format-2 pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiTimelineRealization {
    /// Shared/independent timeline identity.
    pub id: MidiTimelineId,
    /// Source SMF track indices contributing to this timeline.
    pub source_tracks: Vec<usize>,
    /// Exact metrical tempo map for this timeline.
    pub tempo_map: MidiTempoMap,
    /// Identity-bearing notes in stable onset/source order.
    pub notes: Vec<RealizedMidiNote>,
    /// Editable piano-roll projection including note and controller lanes.
    pub piano_roll: PianoRoll,
    /// Unmatched-note and policy diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

/// Exact half-open window over identity-bearing realized MIDI notes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiNoteSlice {
    /// Inclusive start of the sounding window.
    pub at: Time,
    /// Exclusive end of the sounding window.
    pub until: Time,
    /// Notes sounding throughout the window, with source identities intact.
    pub notes: Vec<RealizedMidiNote>,
}

impl MidiTimelineRealization {
    /// Splits this timeline at every realized note onset and sounding release.
    ///
    /// Silent spans are omitted and equal pitches remain separate notes.
    pub fn note_slices(&self) -> Vec<MidiNoteSlice> {
        let mut boundaries = self
            .notes
            .iter()
            .flat_map(|note| [note.onset, note.sounding_until])
            .collect::<Vec<_>>();
        boundaries.sort();
        boundaries.dedup();
        boundaries
            .windows(2)
            .filter_map(|pair| {
                let at = pair[0];
                let until = pair[1];
                let notes = self
                    .notes
                    .iter()
                    .filter(|note| note.onset <= at && at < note.sounding_until)
                    .cloned()
                    .collect::<Vec<_>>();
                (!notes.is_empty() && at < until).then_some(MidiNoteSlice { at, until, notes })
            })
            .collect()
    }
}

/// Complete MIDI realization without flattening independent format-2 patterns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiRealization {
    /// One shared timeline for formats 0/1 or one timeline per format-2 track.
    pub timelines: Vec<MidiTimelineRealization>,
}

/// Realizes MIDI tempo, overlap, pedal, and channel-mode semantics.
pub fn realize_midi(
    file: &SmfFile,
    policy: MidiRealizationPolicy,
) -> Result<MidiRealization, LiftError> {
    realize_midi_impl(file, policy)
}

/// Realizes and selects one independent format-2 pattern.
///
/// For shared-timeline formats, pattern zero selects the sole timeline.
pub fn realize_midi_pattern(
    file: &SmfFile,
    pattern: usize,
    policy: MidiRealizationPolicy,
) -> Result<MidiTimelineRealization, LiftError> {
    let realization = realize_midi_impl(file, policy)?;
    let patterns = realization.timelines.len();
    realization
        .timelines
        .into_iter()
        .nth(pattern)
        .ok_or(LiftError::PatternOutOfRange { pattern, patterns })
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
        let collected = collect_midi(file)?;
        Ok(LiftReport {
            value: collected.to_piano_roll(),
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
