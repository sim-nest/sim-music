use std::collections::BTreeSet;

use sim_kernel::{ContentId, Datum, Symbol};
use sim_lib_music_core::{Articulation, ObjectId, Staff, StaffNote, StaffVoice, Time};
use sim_lib_music_transform::{
    AdditiveStaffPatch, apply_additive_staff_patch, remove_additive_staff_patch,
};
use thiserror::Error;

/// Kernel content identity used to bind a patch to one immutable staff.
pub type ContentKey = ContentId;

/// Semantic class of a consonance addition.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AdditionKind {
    /// One independently proposed note.
    Note,
    /// A bounded figure around an existing event.
    Ornament,
    /// Simultaneous notes proposed as one harmonic unit.
    Chord,
    /// A sustained harmonic pedal point.
    Pedal,
    /// An octave or unison doubling of an existing event.
    Doubling,
    /// A complete new voice.
    Voice,
}

/// One independently proposed note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteAddition {
    /// Identity-bearing note payload to add.
    pub note: StaffNote,
}

/// A bounded note figure anchored to existing material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrnamentAddition {
    /// Existing event that gives the ornament its musical context.
    pub anchor_event_id: ObjectId,
    /// Ordered identity-bearing ornament notes.
    pub notes: Vec<StaffNote>,
}

/// Simultaneous notes introduced as one harmonic choice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChordAddition {
    /// Optional authored harmonic label retained as provenance.
    pub label: Option<String>,
    /// Identity-bearing notes sharing one exact onset and release.
    pub notes: Vec<StaffNote>,
}

/// A sustained harmonic pedal point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PedalAddition {
    /// Optional authored harmonic label retained as provenance.
    pub label: Option<String>,
    /// Long-lived identity-bearing pedal note.
    pub note: StaffNote,
}

/// An octave or unison doubling of an existing event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoublingAddition {
    /// Existing event whose onset, duration, and pitch class are doubled.
    pub source_event_id: ObjectId,
    /// Fresh identity-bearing doubling.
    pub note: StaffNote,
}

/// A complete independent voice introduced by completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceAddition {
    /// New voice, including every fresh note identity.
    pub voice: StaffVoice,
}

/// One typed, strictly additive consonance proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Addition {
    /// One note.
    Note(NoteAddition),
    /// A note figure around an existing event.
    Ornament(OrnamentAddition),
    /// Simultaneous harmonic notes.
    Chord(ChordAddition),
    /// A sustained pedal point.
    Pedal(PedalAddition),
    /// An octave or unison doubling.
    Doubling(DoublingAddition),
    /// A complete independent voice.
    Voice(VoiceAddition),
}

impl Addition {
    /// Returns the semantic addition class.
    pub fn kind(&self) -> AdditionKind {
        match self {
            Self::Note(_) => AdditionKind::Note,
            Self::Ornament(_) => AdditionKind::Ornament,
            Self::Chord(_) => AdditionKind::Chord,
            Self::Pedal(_) => AdditionKind::Pedal,
            Self::Doubling(_) => AdditionKind::Doubling,
            Self::Voice(_) => AdditionKind::Voice,
        }
    }

    /// Iterates over every note introduced by this addition.
    pub fn notes(&self) -> Box<dyn Iterator<Item = &StaffNote> + '_> {
        match self {
            Self::Note(value) => Box::new(std::iter::once(&value.note)),
            Self::Ornament(value) => Box::new(value.notes.iter()),
            Self::Chord(value) => Box::new(value.notes.iter()),
            Self::Pedal(value) => Box::new(std::iter::once(&value.note)),
            Self::Doubling(value) => Box::new(std::iter::once(&value.note)),
            Self::Voice(value) => Box::new(value.voice.notes.iter()),
        }
    }
}

/// A content-bound collection of typed score additions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsonancePatch {
    /// Content identity of the exact staff to which the patch applies.
    pub base: ContentKey,
    /// Typed material introduced by the patch.
    pub additions: Vec<Addition>,
}

impl ConsonancePatch {
    /// Builds and validates a patch against an immutable source staff.
    pub fn new(source: &Staff, additions: Vec<Addition>) -> Result<Self, PatchError> {
        let patch = Self {
            base: staff_content_key(source)?,
            additions,
        };
        patch.compile(source)?;
        Ok(patch)
    }

    pub(crate) fn compile(&self, source: &Staff) -> Result<AdditiveStaffPatch, PatchError> {
        validate_additions(source, &self.additions)?;
        let mut patch = AdditiveStaffPatch::default();
        for addition in &self.additions {
            match addition {
                Addition::Voice(value) => patch.voices.push(value.voice.clone()),
                _ => patch.notes.extend(addition.notes().cloned()),
            }
        }
        apply_additive_staff_patch(source, &patch)
            .map_err(|error| PatchError::InvalidAddition(error.to_string()))?;
        Ok(patch)
    }
}

/// Applies a content-bound consonance patch without mutating its source.
pub fn apply_patch(source: &Staff, patch: &ConsonancePatch) -> Result<Staff, PatchError> {
    require_base(source, &patch.base)?;
    let additions = patch.compile(source)?;
    apply_additive_staff_patch(source, &additions)
        .map(|transform| transform.value)
        .map_err(|error| PatchError::InvalidAddition(error.to_string()))
}

/// Removes exactly a patch's introduced material and verifies its base content.
pub fn remove_patch(completed: &Staff, patch: &ConsonancePatch) -> Result<Staff, PatchError> {
    let additions = compile_without_source(&patch.additions);
    let source = remove_additive_staff_patch(completed, &additions)
        .map(|transform| transform.value)
        .map_err(|error| PatchError::InvalidInverse(error.to_string()))?;
    require_base(&source, &patch.base)?;
    Ok(source)
}

/// Computes the canonical kernel content identity of an exact staff.
pub fn staff_content_key(staff: &Staff) -> Result<ContentKey, PatchError> {
    staff_datum(staff)
        .content_id()
        .map_err(|error| PatchError::ContentIdentity(error.to_string()))
}

/// Failure to construct, apply, or invert a consonance patch.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PatchError {
    /// The patch names a different immutable base staff.
    #[error("consonance patch base does not match the supplied staff")]
    BaseMismatch,
    /// Typed addition invariants were violated.
    #[error("invalid consonance addition: {0}")]
    InvalidAddition(String),
    /// Exact inverse validation failed.
    #[error("invalid consonance patch inverse: {0}")]
    InvalidInverse(String),
    /// Canonical staff hashing failed.
    #[error("staff content identity failed: {0}")]
    ContentIdentity(String),
}

fn validate_additions(source: &Staff, additions: &[Addition]) -> Result<(), PatchError> {
    let duration = source.duration();
    let source_events = source
        .notes()
        .map(|note| (note.event_id.clone(), note))
        .collect::<std::collections::BTreeMap<_, _>>();
    for addition in additions {
        validate_semantics(addition, &source_events)?;
        for note in addition.notes() {
            if note.note.duration <= Time::from_integer(0)
                || note.onset < Time::from_integer(0)
                || note.end() > duration
            {
                return invalid("added notes must have positive spans inside the source duration");
            }
        }
        if let Addition::Voice(value) = addition
            && value.voice.duration != duration
        {
            return invalid("added voices must retain the source staff duration");
        }
    }
    Ok(())
}

fn validate_semantics(
    addition: &Addition,
    source_events: &std::collections::BTreeMap<ObjectId, &StaffNote>,
) -> Result<(), PatchError> {
    match addition {
        Addition::Note(_) => {}
        Addition::Ornament(value) => {
            if !source_events.contains_key(&value.anchor_event_id) || value.notes.is_empty() {
                return invalid("an ornament needs an existing anchor and at least one note");
            }
        }
        Addition::Chord(value) => {
            let Some(first) = value.notes.first() else {
                return invalid("a chord addition must contain notes");
            };
            if value.notes.len() < 2
                || value
                    .notes
                    .iter()
                    .any(|note| note.onset != first.onset || note.end() != first.end())
            {
                return invalid("a chord addition needs at least two notes with one exact span");
            }
        }
        Addition::Pedal(_) => {}
        Addition::Doubling(value) => {
            let Some(source) = source_events.get(&value.source_event_id) else {
                return invalid("a doubling must name an existing source event");
            };
            if value.note.onset != source.onset
                || value.note.note.duration != source.note.duration
                || value.note.note.pitch.class != source.note.pitch.class
            {
                return invalid("a doubling must retain source onset, duration, and pitch class");
            }
        }
        Addition::Voice(value) if value.voice.notes.is_empty() => {
            return invalid("an added voice must contain at least one note");
        }
        Addition::Voice(_) => {}
    }
    Ok(())
}

fn compile_without_source(additions: &[Addition]) -> AdditiveStaffPatch {
    let mut patch = AdditiveStaffPatch::default();
    for addition in additions {
        match addition {
            Addition::Voice(value) => patch.voices.push(value.voice.clone()),
            _ => patch.notes.extend(addition.notes().cloned()),
        }
    }
    patch
}

fn require_base(staff: &Staff, expected: &ContentKey) -> Result<(), PatchError> {
    if staff_content_key(staff)? == *expected {
        Ok(())
    } else {
        Err(PatchError::BaseMismatch)
    }
}

fn staff_datum(staff: &Staff) -> Datum {
    Datum::Node {
        tag: Symbol::qualified("music/consonance", "staff-v1"),
        fields: vec![
            (Symbol::new("duration"), time_datum(staff.duration())),
            (
                Symbol::new("voices"),
                Datum::Vector(staff.voices.iter().map(voice_datum).collect()),
            ),
        ],
    }
}

fn voice_datum(voice: &StaffVoice) -> Datum {
    Datum::Vector(vec![
        Datum::String(voice.id.to_string()),
        Datum::String(voice.name.clone()),
        time_datum(voice.duration),
        Datum::Vector(voice.notes.iter().map(note_datum).collect()),
    ])
}

fn note_datum(note: &StaffNote) -> Datum {
    Datum::Vector(vec![
        Datum::String(note.voice_id.to_string()),
        Datum::String(note.note_id.to_string()),
        Datum::String(note.event_id.to_string()),
        time_datum(note.onset),
        time_datum(note.note.duration),
        Datum::String(note.note.pitch.semitone().to_string()),
        Datum::String(note.note.velocity.to_string()),
        Datum::String(note.note.channel.0.to_string()),
        Datum::String(articulation_name(note.note.articulation).to_owned()),
    ])
}

fn time_datum(value: Time) -> Datum {
    Datum::String(format!("{}/{}", value.numer(), value.denom()))
}

fn articulation_name(value: Articulation) -> &'static str {
    match value {
        Articulation::Normal => "normal",
        Articulation::Staccato => "staccato",
        Articulation::Legato => "legato",
        Articulation::Tenuto => "tenuto",
        Articulation::Accent => "accent",
        Articulation::Marcato => "marcato",
    }
}

fn invalid<T>(reason: impl Into<String>) -> Result<T, PatchError> {
    Err(PatchError::InvalidAddition(reason.into()))
}

pub(crate) fn addition_ids(additions: &[Addition]) -> Vec<ObjectId> {
    let mut ids = BTreeSet::new();
    for addition in additions {
        if let Addition::Voice(value) = addition {
            ids.insert(value.voice.id.clone());
        }
        for note in addition.notes() {
            ids.insert(note.note_id.clone());
            ids.insert(note.event_id.clone());
        }
    }
    ids.into_iter().collect()
}
