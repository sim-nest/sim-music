//! Identity-bearing score forms and loss-audited conversion reports.

mod convert;

use std::collections::BTreeSet;
use std::fmt;

use thiserror::Error;

use crate::{
    AtomRef, Melody, MusicError, MusicObject, Note, PianoRoll, Progression, Time, TimedAtom,
};
use crate::{Chord, Counterpoint};

pub use convert::convert_score;

/// Stable identity for a voice, note, or score event.
///
/// Imported forms without native identifiers receive deterministic identifiers
/// derived from their structural position. Once allocated, the identifiers are
/// carried by staff, snapshot, change-stream, and exact-transform operations.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(String);

impl ObjectId {
    /// Creates an identifier from a stable, non-empty string.
    pub fn new(value: impl Into<String>) -> Result<Self, ConversionError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ConversionError::InvalidIdentity(
                "object identity cannot be empty".to_owned(),
            ));
        }
        Ok(Self(value))
    }

    /// Returns the identifier's stable wire value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn derived(kind: &str, path: impl fmt::Display) -> Self {
        Self(format!("{kind}/{path}"))
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A note placed on an identity-bearing staff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaffNote {
    /// Identity of the containing voice.
    pub voice_id: ObjectId,
    /// Identity of the logical note across reversible pitch/time transforms.
    pub note_id: ObjectId,
    /// Identity of this note event across score representations.
    pub event_id: ObjectId,
    /// Exact absolute onset in whole-note units.
    pub onset: Time,
    /// Musical note payload.
    pub note: Note,
}

impl StaffNote {
    /// Returns the exact half-open end of this note.
    pub fn end(&self) -> Time {
        self.onset + self.note.duration
    }
}

/// One named staff voice with an exact notated span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaffVoice {
    /// Stable voice identity.
    pub id: ObjectId,
    /// Human-facing voice name.
    pub name: String,
    /// Exact span, including trailing silence.
    pub duration: Time,
    /// Notes belonging to the voice, in stable time order.
    pub notes: Vec<StaffNote>,
}

/// Identity-bearing canonical score timeline.
///
/// A staff is the lossless interlingua for note-bearing catalog forms. It
/// retains voice boundaries, exact rational timing, trailing silence, and
/// stable note/event identities without requiring those concerns in the
/// lightweight [`Note`] value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staff {
    /// Independent or simultaneous voices.
    pub voices: Vec<StaffVoice>,
}

impl Staff {
    /// Builds and validates a staff, sorting voices and notes canonically.
    pub fn new(mut voices: Vec<StaffVoice>) -> Result<Self, ConversionError> {
        let zero = Time::from_integer(0);
        let mut identities = BTreeSet::new();
        for voice in &mut voices {
            if voice.duration < zero {
                return Err(ConversionError::Music(MusicError::NegativeDuration));
            }
            if !identities.insert(voice.id.clone()) {
                return Err(ConversionError::DuplicateIdentity(voice.id.clone()));
            }
            for note in &voice.notes {
                if note.voice_id != voice.id {
                    return Err(ConversionError::InvalidIdentity(format!(
                        "event {} names voice {} but belongs to {}",
                        note.event_id, note.voice_id, voice.id
                    )));
                }
                if note.onset < zero {
                    return Err(ConversionError::Music(MusicError::NegativeOnset));
                }
                if note.note.duration < zero {
                    return Err(ConversionError::Music(MusicError::NegativeDuration));
                }
                if note.end() > voice.duration {
                    return Err(ConversionError::InvalidIdentity(format!(
                        "event {} ends after voice {}",
                        note.event_id, voice.id
                    )));
                }
                for id in [&note.note_id, &note.event_id] {
                    if !identities.insert(id.clone()) {
                        return Err(ConversionError::DuplicateIdentity(id.clone()));
                    }
                }
            }
            voice.notes.sort_by(staff_note_order);
        }
        voices.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(Self { voices })
    }

    /// Returns the exact parallel span of all voices.
    pub fn duration(&self) -> Time {
        self.voices
            .iter()
            .map(|voice| voice.duration)
            .max()
            .unwrap_or_else(|| Time::from_integer(0))
    }

    /// Iterates over all notes in canonical voice/time order.
    pub fn notes(&self) -> impl Iterator<Item = &StaffNote> {
        self.voices.iter().flat_map(|voice| voice.notes.iter())
    }

    /// Returns every voice, note, and event identity in sorted order.
    pub fn object_ids(&self) -> Vec<ObjectId> {
        let mut ids = self
            .voices
            .iter()
            .flat_map(|voice| {
                std::iter::once(voice.id.clone()).chain(
                    voice
                        .notes
                        .iter()
                        .flat_map(|note| [note.note_id.clone(), note.event_id.clone()]),
                )
            })
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        ids
    }
}

impl MusicObject for Staff {
    fn kind(&self) -> &'static str {
        "Staff"
    }

    fn duration(&self) -> Time {
        Staff::duration(self)
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        for item in self.notes() {
            out.push(TimedAtom {
                onset: offset + item.onset,
                atom: AtomRef::Note(item.note.clone()),
            });
        }
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Complete pitch-activity snapshot at one exact boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicSnapshot {
    /// Exact snapshot time.
    pub at: Time,
    /// Complete set of identity-bearing notes sounding in the half-open
    /// interval at `at`.
    pub sounding: Vec<StaffNote>,
}

/// Voice metadata retained by non-staff event representations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreVoice {
    /// Stable voice identity.
    pub id: ObjectId,
    /// Human-facing voice name.
    pub name: String,
    /// Exact voice span, including trailing silence.
    pub duration: Time,
}

/// Event-boundary snapshot representation of a score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotStream {
    /// Exact total score span, including trailing silence.
    pub duration: Time,
    /// Voice identities, names, and individual spans.
    pub voices: Vec<ScoreVoice>,
    /// Complete event-boundary snapshots in strictly ascending time order.
    pub snapshots: Vec<MusicSnapshot>,
}

/// One identity-bearing change in a score event stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MusicChange {
    /// A note starts; the complete note payload makes the stream self-contained.
    NoteStarted(StaffNote),
    /// A note ends at the given exact time.
    NoteEnded {
        /// Exact release time.
        at: Time,
        /// Voice containing the note.
        voice_id: ObjectId,
        /// Logical note identity.
        note_id: ObjectId,
        /// Event identity.
        event_id: ObjectId,
    },
}

impl MusicChange {
    /// Returns the exact time at which this change occurs.
    pub fn at(&self) -> Time {
        match self {
            Self::NoteStarted(note) => note.onset,
            Self::NoteEnded { at, .. } => *at,
        }
    }
}

/// Chronological note-on/note-off representation of a score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicChangeStream {
    /// Exact total score span, including trailing silence.
    pub duration: Time,
    /// Voice identities, names, and individual spans.
    pub voices: Vec<ScoreVoice>,
    /// Changes in deterministic time, release-before-start identity order.
    pub changes: Vec<MusicChange>,
}

/// Catalog score representations supported by [`convert_score`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScoreForm {
    /// Monophonic melody.
    Melody(Melody),
    /// Simultaneous chord.
    Chord(Chord),
    /// Identity-bearing staff.
    Staff(Staff),
    /// Named independent voices.
    Counterpoint(Counterpoint),
    /// Timed piano-roll lanes.
    PianoRoll(PianoRoll),
    /// Event-boundary snapshots.
    Snapshot(SnapshotStream),
    /// Chronological start/end changes.
    ChangeStream(MusicChangeStream),
    /// Sequential chord progression.
    Progression(Progression),
}

impl ScoreForm {
    /// Returns this representation's catalog kind.
    pub fn kind(&self) -> ScoreFormKind {
        match self {
            Self::Melody(_) => ScoreFormKind::Melody,
            Self::Chord(_) => ScoreFormKind::Chord,
            Self::Staff(_) => ScoreFormKind::Staff,
            Self::Counterpoint(_) => ScoreFormKind::Counterpoint,
            Self::PianoRoll(_) => ScoreFormKind::PianoRoll,
            Self::Snapshot(_) => ScoreFormKind::Snapshot,
            Self::ChangeStream(_) => ScoreFormKind::ChangeStream,
            Self::Progression(_) => ScoreFormKind::Progression,
        }
    }
}

/// Target representation for a catalog conversion.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ScoreFormKind {
    /// [`ScoreForm::Melody`].
    Melody,
    /// [`ScoreForm::Chord`].
    Chord,
    /// [`ScoreForm::Staff`].
    Staff,
    /// [`ScoreForm::Counterpoint`].
    Counterpoint,
    /// [`ScoreForm::PianoRoll`].
    PianoRoll,
    /// [`ScoreForm::Snapshot`].
    Snapshot,
    /// [`ScoreForm::ChangeStream`].
    ChangeStream,
    /// [`ScoreForm::Progression`].
    Progression,
}

/// Explicit choice used when a target cannot represent every simultaneous line.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AmbiguousConversionPolicy {
    /// Reject rather than guess or discard material.
    Reject,
    /// Retain the line with the highest first sounding pitch.
    KeepHighest,
    /// Retain the line with the lowest first sounding pitch.
    KeepLowest,
    /// Retain the first line in canonical identity order.
    KeepFirst,
}

/// Machine-readable kind of information a conversion could not carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConversionLossKind {
    /// An explicit rest boundary became implicit silence.
    ExplicitRest,
    /// A chord or progression label could not be represented.
    HarmonicLabel,
    /// A progression key annotation could not be represented.
    KeyAnnotation,
    /// Piano-roll grid metadata could not be represented.
    PianoRollGrid,
    /// A non-note piano-roll cell could not be represented.
    NonNoteCell,
    /// A voice or note was discarded under the selected ambiguity policy.
    DiscardedVoice,
    /// Distinct voice boundaries collapsed in a target without voices.
    VoiceBoundary,
    /// A voice name or exact silent span could not be represented.
    VoiceMetadata,
    /// Stable note or event identities survive only in the report sidecar.
    IdentityMetadata,
    /// A zero-duration event is invisible to sounding-note snapshots.
    ZeroDurationSnapshot,
    /// Silence could not be represented by the target form.
    Silence,
    /// Change-stream boundaries disagreed with their note payload.
    InconsistentChange,
    /// A target-only label had to be synthesized.
    SynthesizedLabel,
    /// The source music object's structural form is not carried by a semantic event form.
    SourceStructure,
    /// An absolute pitch/time anchor was intentionally omitted from a relative form.
    RelativeAnchor,
}

/// One explicit, identity-addressed conversion loss.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversionLoss {
    /// Stable loss classification.
    pub kind: ConversionLossKind,
    /// Closest affected identity, when one exists.
    pub object: Option<ObjectId>,
    /// Human-readable exact reason.
    pub detail: String,
}

impl ConversionLoss {
    /// Builds one conversion loss with an optional affected score identity.
    ///
    /// Conversion owners outside `sim-lib-music-core` use this constructor when
    /// they reuse [`MusicConversion`] for another exact music representation.
    pub fn new(
        kind: ConversionLossKind,
        object: Option<ObjectId>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            object,
            detail: detail.into(),
        }
    }
}

/// Converted value with identity retention and complete loss evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicConversion<T> {
    /// Converted value.
    pub value: T,
    /// Voice, note, and event identities retained in the result or its report.
    pub preserved: Vec<ObjectId>,
    /// Facts not representable in the target form.
    pub losses: Vec<ConversionLoss>,
}

impl<T> MusicConversion<T> {
    /// Maps the converted value while retaining its audit report.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> MusicConversion<U> {
        MusicConversion {
            value: f(self.value),
            preserved: self.preserved,
            losses: self.losses,
        }
    }

    /// Returns `true` when no conversion loss was recorded.
    pub fn is_lossless(&self) -> bool {
        self.losses.is_empty()
    }
}

/// Error raised when a requested conversion cannot honor its explicit policy.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConversionError {
    /// A source value violated a music-model invariant.
    #[error(transparent)]
    Music(#[from] MusicError),
    /// The explicit reject policy encountered an ambiguous conversion.
    #[error("ambiguous {from:?} to {to:?} conversion: {detail}")]
    Ambiguous {
        /// Source form.
        from: ScoreFormKind,
        /// Requested target form.
        to: ScoreFormKind,
        /// Stable explanation of the ambiguity.
        detail: String,
    },
    /// An identity was empty or inconsistent with its container.
    #[error("invalid score identity: {0}")]
    InvalidIdentity(String),
    /// Two different score objects shared one identity.
    #[error("duplicate score identity {0}")]
    DuplicateIdentity(ObjectId),
}

pub(crate) fn staff_note_order(left: &StaffNote, right: &StaffNote) -> std::cmp::Ordering {
    left.onset
        .cmp(&right.onset)
        .then_with(|| left.note.pitch.cmp(&right.note.pitch))
        .then_with(|| left.event_id.cmp(&right.event_id))
}
