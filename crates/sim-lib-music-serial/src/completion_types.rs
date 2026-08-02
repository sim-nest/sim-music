use sim_lib_discrete_search::SearchReceipt;
use sim_lib_music_core::{ObjectId, Pitch, Staff, StaffNote, StaffVoice};
use thiserror::Error;

use crate::additive::AdditiveStaffPatch;
use crate::allowance::{SerialAllowanceMatch, SerialCompletionAllowances};
use crate::{
    InvariantLedger, PracticeRuleId, SerialPlan, SerialPlanError, StrictRealizationError, WaiverId,
};

/// Semantic class of a serial completion addition.
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

/// One typed, strictly additive completion candidate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionCandidate {
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

impl CompletionCandidate {
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

    /// Returns every note introduced by this candidate.
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

    pub(crate) fn compile_into(&self, patch: &mut AdditiveStaffPatch) {
        match self {
            Self::Voice(value) => patch.voices.push(value.voice.clone()),
            _ => patch.notes.extend(self.notes().cloned()),
        }
    }
}

/// One voice-specific or global pitch-range guard for added notes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchRangeConstraint {
    /// `None` applies to every added note.
    pub voice_id: Option<ObjectId>,
    /// Inclusive lower pitch.
    pub lowest: Pitch,
    /// Inclusive upper pitch.
    pub highest: Pitch,
}

/// Generic additive completion request used by the serial adapter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompletionRequest {
    /// Deterministically ordered candidates considered by the bounded search.
    pub candidates: Vec<CompletionCandidate>,
    /// Minimum number of selected candidates required for success.
    pub min_candidates: usize,
    /// Maximum number of selected candidates admitted by the search.
    pub max_candidates: Option<usize>,
    /// Optional pitch-range guards for introduced notes.
    pub pitch_ranges: Vec<PitchRangeConstraint>,
}

/// Provenance retained for one generic additive completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionProvenance {
    /// Candidate indexes selected in caller order after serial filtering.
    pub selected_candidates: Vec<usize>,
    /// Original source identities preserved by the additive transform.
    pub preserved_ids: Vec<ObjectId>,
    /// Every new identity introduced by the patch.
    pub added_ids: Vec<ObjectId>,
    /// Stable contract facts preserved by the generic adapter.
    pub facts: Vec<String>,
}

/// Generic reversible completion output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionResult {
    /// Exact additive patch selected by bounded search.
    pub patch: AdditiveStaffPatch,
    /// Unchanged source staff.
    pub before: Staff,
    /// Completed staff after applying the selected patch.
    pub after: Staff,
    /// Exact bounded-search termination receipt.
    pub search: SearchReceipt,
    /// Reversible patch provenance and identity evidence.
    pub provenance: CompletionProvenance,
}

/// Generic completion failure retained by the serial adapter.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CompletionError {
    /// The requested candidates could not define a valid additive patch.
    #[error("invalid additive completion candidate: {0}")]
    InvalidCandidate(String),
    /// Search terminated without a feasible completion.
    #[error("bounded completion produced no feasible patch")]
    NoCompletion {
        /// Original source staff retained for diagnosis.
        before: Box<Staff>,
        /// Honest search receipt.
        search: Box<SearchReceipt>,
    },
}

/// One accepted serial completion note and the category that licensed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedSerialNote {
    /// Added note identity.
    pub event_id: ObjectId,
    /// Serial category assigned to the accepted note.
    pub category: AcceptedSerialCategory,
    /// Exact allowance evidence that admitted the note.
    pub allowance: SerialAllowanceMatch,
}

/// Final serial provenance class assigned to one accepted note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcceptedSerialCategory {
    /// Direct row material reuse admitted by the structural plan itself.
    RowNative,
    /// Reuse of already declared derived material.
    RowDerived,
    /// Reuse of a landed pitch through a modal or caller-supplied spine.
    ModalProjected,
    /// Reuse of a caller-declared referential subset.
    Referential {
        /// Stable referential subset identity.
        id: String,
    },
    /// Reuse of explicit foreign material under a declared waiver.
    ForeignWithWaiver {
        /// Stable waiver identity authorizing the foreign reuse.
        waiver: WaiverId,
    },
}

/// One accepted typed addition with exact per-note serial provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedSerialAddition {
    /// Original caller-supplied candidate index.
    pub candidate_index: usize,
    /// Reused typed addition kind.
    pub kind: AdditionKind,
    /// Per-note serial categories and allowance evidence.
    pub notes: Vec<AcceptedSerialNote>,
}

/// Serial request layered over the generic additive completion request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialCompletionRequest {
    /// Exact generic completion request retained unchanged after filtering.
    pub completion: CompletionRequest,
    /// Serial legality categories admitted for candidate notes.
    pub allowances: SerialCompletionAllowances,
}

/// Exact serial completion result that preserves both structural and generic evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialCompletionResult {
    /// Original structural plan, retained byte-for-byte.
    pub structural_plan: SerialPlan,
    /// Unmodified generic reversible completion output with original candidate indexes restored.
    pub generic: CompletionResult,
    /// Accepted typed additions and their serial provenance classes.
    pub accepted_additions: Vec<AcceptedSerialAddition>,
    /// Serial-practice ledger before completion under the structural reading.
    pub structural_before: InvariantLedger<PracticeRuleId>,
    /// Serial-practice ledger after completion under the structural reading.
    pub structural_after: InvariantLedger<PracticeRuleId>,
    /// Serial-practice ledger after completion under the all-sounding reading.
    pub sounding_after: InvariantLedger<PracticeRuleId>,
}

/// Failure to filter, realize, or adapt generic completion into serial evidence.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SerialCompletionError {
    /// Rendering the realized serial plan to canonical staff failed.
    #[error(transparent)]
    Realization(#[from] StrictRealizationError),
    /// Generic completion itself failed.
    #[error(transparent)]
    Completion(#[from] CompletionError),
    /// Building the post-completion serial evidence plan failed.
    #[error(transparent)]
    Plan(#[from] SerialPlanError),
    /// The request admitted no serial-legal candidates.
    #[error("serial completion admitted no legal candidates: {0}")]
    NoLegalCandidates(String),
    /// One generated identity was malformed.
    #[error("serial completion identity error: {0}")]
    Identity(String),
}
