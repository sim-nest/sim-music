//! Ranked extraction of serial-row hypotheses from exact music attacks.

use std::collections::{BTreeMap, BTreeSet};

use sim_lib_discrete_search::{
    NeverInterrupt, SearchControl, SearchInterrupt, SearchOrder, SearchProblem, SearchRun,
    SearchStatus, SearchStep, solve,
};
use sim_lib_music_core::{
    AmbiguousConversionPolicy, AtomRef, Music, MusicObject, Note, ObjectId, Score, ScoreForm,
    ScoreFormKind, Staff, StaffNote, StaffVoice, Time, convert_score,
};
use sim_lib_pitch_serial::{RowClassAlias, RowLabelConvention, ToneRow, analyze_row_class};
use thiserror::Error;

use crate::{
    ExtractionEvidence, ExtractionOutcome, RankedSerialHypothesis, SerialAliasEvidence,
    SerialObservation, SerialObservationBlock, SerialReadingOrder, SerialStableRank,
    SerialTimeSpan,
};

/// Request policy for serial-row extraction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialExtractionRequest {
    /// Generic bounded-search control reused from the discrete owner.
    pub search: SearchControl,
    /// Convention used when rendering alias labels.
    pub label_convention: RowLabelConvention,
}

impl Default for SerialExtractionRequest {
    fn default() -> Self {
        Self {
            search: SearchControl::default()
                .with_order(SearchOrder::DepthFirst)
                .with_max_results(128),
            label_convention: RowLabelConvention::FirstLastPitch,
        }
    }
}

/// Auxiliary services for extraction.
#[derive(Default)]
pub struct SerialExtractionServices<'a> {
    /// Optional external interrupt source checked by the generic search loop.
    pub interrupt: Option<&'a dyn SearchInterrupt>,
}

/// Failure while extracting serial-row hypotheses.
#[derive(Debug, Error)]
pub enum SerialExtractionError {
    /// Existing score conversion rejected the source.
    #[error("serial extraction score conversion failed: {0}")]
    ScoreConversion(String),
    /// The exact window owner rejected the source staff.
    #[error("serial extraction window construction failed: {0}")]
    WindowConstruction(String),
    /// A score-form conversion or derived identity was invalid.
    #[error("serial extraction identity failure: {0}")]
    Identity(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AttackCandidate {
    voice_id: ObjectId,
    note_id: ObjectId,
    event_id: ObjectId,
    midi: u8,
    onset: Time,
    release: Time,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AttackGroup {
    span: SerialTimeSpan,
    notes: Vec<AttackCandidate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ChosenBlock {
    span: SerialTimeSpan,
    order: SerialReadingOrder,
    notes: Vec<AttackCandidate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExtractionState {
    next_group: usize,
    blocks: Vec<ChosenBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BlockChoice {
    group_index: usize,
    order: SerialReadingOrder,
    event_key: String,
}

struct ExtractionProblem {
    groups: Vec<AttackGroup>,
}

impl SearchProblem for ExtractionProblem {
    type State = ExtractionState;
    type Choice = BlockChoice;
    type Output = RankedSerialHypothesis;

    fn initial_state(&self) -> Self::State {
        ExtractionState {
            next_group: 0,
            blocks: Vec::new(),
        }
    }

    fn expand(&self, state: &Self::State, out: &mut Vec<Self::Choice>) {
        if state.next_group >= self.groups.len() {
            return;
        }
        let group = &self.groups[state.next_group];
        for order in candidate_orders(group.notes.len()) {
            let sorted = sorted_candidates(&group.notes, order);
            out.push(BlockChoice {
                group_index: state.next_group,
                order,
                event_key: stable_event_key(&sorted),
            });
        }
    }

    fn apply(&self, state: &Self::State, choice: &Self::Choice) -> SearchStep<Self::State> {
        let group = &self.groups[choice.group_index];
        let mut blocks = state.blocks.clone();
        blocks.push(ChosenBlock {
            span: group.span.clone(),
            order: choice.order,
            notes: sorted_candidates(&group.notes, choice.order),
        });
        SearchStep::Continue(ExtractionState {
            next_group: state.next_group + 1,
            blocks,
        })
    }

    fn finish(&self, state: &Self::State) -> Option<Self::Output> {
        (state.next_group == self.groups.len()).then(|| hypothesis_from_blocks(&state.blocks))
    }

    fn score_state(&self, state: &Self::State) -> i64 {
        -(state.blocks.len() as i64)
    }

    fn output_score(&self, output: &Self::Output) -> Option<i64> {
        Some(
            output.stable_rank.omissions as i64 * 1_000_000
                + output.stable_rank.duplicates_before_completion as i64 * 10_000
                + output.stable_rank.order_errors as i64 * 100
                + *output.stable_rank.occupied_span.numer(),
        )
    }
}

/// Extracts ranked serial-row hypotheses from a musical score.
pub fn extract_serial_hypotheses(
    score: &Score,
    request: &SerialExtractionRequest,
    services: &SerialExtractionServices<'_>,
) -> Result<ExtractionOutcome, SerialExtractionError> {
    let staff = canonical_staff(score)?;
    let groups = attack_groups(&staff)?;
    let source_summary = vec![
        format!("voices={}", staff.voices.len()),
        format!("attack-groups={}", groups.len()),
        format!(
            "attacks={}",
            groups.iter().map(|group| group.notes.len()).sum::<usize>()
        ),
    ];
    let interrupt = services
        .interrupt
        .map_or(&NeverInterrupt as &dyn SearchInterrupt, |interrupt| {
            interrupt
        });
    let run = solve(
        &ExtractionProblem { groups },
        request.search.clone(),
        interrupt,
    );
    let receipt = run.receipt.clone();
    let ranked = dedupe_and_rank(run, request.label_convention);
    let evidence = ExtractionEvidence {
        search: receipt,
        source_summary,
    };
    match evidence.search.status {
        SearchStatus::Partial => Ok(ExtractionOutcome::BudgetExhausted { ranked, evidence }),
        SearchStatus::Cancelled | SearchStatus::Infeasible | SearchStatus::Complete => {
            if ranked.len() <= 1 {
                let hypothesis = ranked.first().cloned().unwrap_or_else(empty_hypothesis);
                Ok(ExtractionOutcome::Complete {
                    hypothesis: Box::new(hypothesis),
                    ranked,
                    evidence,
                })
            } else {
                Ok(ExtractionOutcome::Ambiguous { ranked, evidence })
            }
        }
    }
}

fn canonical_staff(score: &Score) -> Result<Staff, SerialExtractionError> {
    if let Some(form) = score_form(&score.body) {
        let report = convert_score(
            &form,
            ScoreFormKind::Staff,
            AmbiguousConversionPolicy::Reject,
        )
        .map_err(|error| SerialExtractionError::ScoreConversion(error.to_string()))?;
        let ScoreForm::Staff(staff) = report.value else {
            unreachable!("staff conversion must return a staff");
        };
        Ok(staff)
    } else {
        flattened_staff(score)
    }
}

fn attack_groups(staff: &Staff) -> Result<Vec<AttackGroup>, SerialExtractionError> {
    let duration = staff.duration();
    if duration == Time::from_integer(0) {
        return Ok(Vec::new());
    }
    let notes = staff
        .notes()
        .map(|note| {
            let midi = note.note.pitch.to_midi().ok_or_else(|| {
                SerialExtractionError::WindowConstruction(format!(
                    "non-MIDI pitch in event {}",
                    note.event_id
                ))
            })?;
            Ok(AttackCandidate {
                voice_id: note.voice_id.clone(),
                note_id: note.note_id.clone(),
                event_id: note.event_id.clone(),
                midi,
                onset: note.onset,
                release: note.end(),
            })
        })
        .collect::<Result<Vec<_>, SerialExtractionError>>()?;
    let mut boundaries = vec![Time::from_integer(0), duration];
    for note in &notes {
        boundaries.push(note.onset);
        boundaries.push(note.release);
    }
    boundaries.sort();
    boundaries.dedup();
    Ok(boundaries
        .windows(2)
        .filter_map(|pair| {
            let span = SerialTimeSpan::new(pair[0], pair[1]);
            let attacks = notes
                .iter()
                .filter(|note| note.onset == span.start)
                .cloned()
                .collect::<Vec<_>>();
            (!attacks.is_empty()).then_some(AttackGroup {
                span,
                notes: attacks,
            })
        })
        .collect())
}

fn hypothesis_from_blocks(blocks: &[ChosenBlock]) -> RankedSerialHypothesis {
    let mut seen = BTreeMap::<u8, usize>::new();
    let mut row_classes = Vec::new();
    let mut duplicates_before_completion = 0usize;
    let mut order_errors = 0usize;
    let mut observations = Vec::new();
    let mut all_event_ids = Vec::new();
    for block in blocks {
        let mut block_observations = Vec::new();
        for note in &block.notes {
            let class = note.midi % 12;
            let ordinal = if let Some(&ordinal) = seen.get(&class) {
                if row_classes.len() < 12 {
                    duplicates_before_completion += 1;
                } else {
                    order_errors += 1;
                }
                ordinal
            } else {
                let ordinal = row_classes.len();
                seen.insert(class, ordinal);
                row_classes.push(class);
                ordinal
            };
            block_observations.push(SerialObservation {
                voice_id: note.voice_id.clone(),
                note_id: note.note_id.clone(),
                event_id: note.event_id.clone(),
                ordinal,
                span: block.span.clone(),
            });
            all_event_ids.push(note.event_id.to_string());
        }
        observations.push(SerialObservationBlock {
            span: block.span.clone(),
            order: block.order,
            observations: block_observations,
        });
    }

    let omissions = 12usize.saturating_sub(row_classes.len());
    let row = tone_row_from_classes(&row_classes);
    let row_report = analyze_row_class(&row);
    let aliases = alias_evidence(
        &row_report.aliases,
        &row,
        RowLabelConvention::FirstLastPitch,
    );
    let start = blocks
        .first()
        .map(|block| block.span.start)
        .unwrap_or_else(|| Time::from_integer(0));
    let end = blocks
        .iter()
        .flat_map(|block| block.notes.iter().map(|note| note.release))
        .max()
        .unwrap_or(start);
    let span = SerialTimeSpan::new(start, end);
    let stable_key = all_event_ids.join("|");
    let stable_rank = SerialStableRank {
        omissions,
        duplicates_before_completion,
        order_errors,
        occupied_span: span.duration(),
        stable_key,
    };
    RankedSerialHypothesis {
        stable_rank,
        row,
        blocks: observations,
        duplicates_before_completion,
        order_errors,
        omissions,
        span,
        aliases,
    }
}

fn alias_evidence(
    aliases: &[RowClassAlias],
    row: &ToneRow,
    convention: RowLabelConvention,
) -> Vec<SerialAliasEvidence> {
    aliases
        .iter()
        .copied()
        .map(|alias| {
            let label = row.apply(alias.operation).label(convention).to_string();
            SerialAliasEvidence { alias, label }
        })
        .collect()
}

fn dedupe_and_rank(
    run: SearchRun<RankedSerialHypothesis>,
    convention: RowLabelConvention,
) -> Vec<RankedSerialHypothesis> {
    let mut by_key = BTreeMap::<String, RankedSerialHypothesis>::new();
    for mut hypothesis in run.outputs {
        hypothesis.aliases = alias_evidence(
            &analyze_row_class(&hypothesis.row).aliases,
            &hypothesis.row,
            convention,
        );
        let key = format!(
            "{:?}|{:?}|{:?}",
            hypothesis.row.classes(),
            hypothesis.stable_rank,
            hypothesis
                .blocks
                .iter()
                .map(|block| block.order.as_str())
                .collect::<Vec<_>>()
        );
        by_key.entry(key).or_insert(hypothesis);
    }
    let mut ranked = by_key.into_values().collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        left.stable_rank
            .cmp(&right.stable_rank)
            .then_with(|| left.aliases.len().cmp(&right.aliases.len()))
    });
    ranked
}

fn candidate_orders(group_len: usize) -> Vec<SerialReadingOrder> {
    let mut orders = BTreeSet::from([
        SerialReadingOrder::WindowOrder,
        SerialReadingOrder::PitchAscending,
        SerialReadingOrder::PitchDescending,
        SerialReadingOrder::VoiceAscending,
        SerialReadingOrder::VoiceDescending,
    ]);
    if group_len <= 1 {
        orders.retain(|order| *order == SerialReadingOrder::WindowOrder);
    }
    orders.into_iter().collect()
}

fn sorted_candidates(notes: &[AttackCandidate], order: SerialReadingOrder) -> Vec<AttackCandidate> {
    let mut sorted = notes.to_vec();
    match order {
        SerialReadingOrder::WindowOrder => {}
        SerialReadingOrder::PitchAscending => sorted.sort_by(|left, right| {
            left.midi
                .cmp(&right.midi)
                .then_with(|| left.voice_id.cmp(&right.voice_id))
                .then_with(|| left.event_id.cmp(&right.event_id))
        }),
        SerialReadingOrder::PitchDescending => sorted.sort_by(|left, right| {
            right
                .midi
                .cmp(&left.midi)
                .then_with(|| left.voice_id.cmp(&right.voice_id))
                .then_with(|| left.event_id.cmp(&right.event_id))
        }),
        SerialReadingOrder::VoiceAscending => sorted.sort_by(|left, right| {
            left.voice_id
                .cmp(&right.voice_id)
                .then_with(|| left.midi.cmp(&right.midi))
                .then_with(|| left.event_id.cmp(&right.event_id))
        }),
        SerialReadingOrder::VoiceDescending => sorted.sort_by(|left, right| {
            right
                .voice_id
                .cmp(&left.voice_id)
                .then_with(|| left.midi.cmp(&right.midi))
                .then_with(|| left.event_id.cmp(&right.event_id))
        }),
    }
    sorted
}

fn stable_event_key(notes: &[AttackCandidate]) -> String {
    notes
        .iter()
        .map(|note| note.event_id.to_string())
        .collect::<Vec<_>>()
        .join("|")
}

fn tone_row_from_classes(classes: &[u8]) -> ToneRow {
    use sim_lib_music_core::PitchClass;

    let mut ordered = classes
        .iter()
        .copied()
        .map(|value| PitchClass::new(value).expect("pitch class"))
        .collect::<Vec<_>>();
    for value in 0..12u8 {
        if !classes.contains(&value) {
            ordered.push(PitchClass::new(value).expect("pitch class"));
        }
    }
    let classes = std::array::from_fn(|index| ordered[index]);
    ToneRow::try_from_classes(classes).expect("padded class order is exhaustive")
}

fn empty_hypothesis() -> RankedSerialHypothesis {
    let row = tone_row_from_classes(&[]);
    RankedSerialHypothesis {
        stable_rank: SerialStableRank {
            omissions: 12,
            duplicates_before_completion: 0,
            order_errors: 0,
            occupied_span: Time::from_integer(0),
            stable_key: "empty".to_owned(),
        },
        row,
        blocks: Vec::new(),
        duplicates_before_completion: 0,
        order_errors: 0,
        omissions: 12,
        span: SerialTimeSpan::new(Time::from_integer(0), Time::from_integer(0)),
        aliases: Vec::new(),
    }
}

fn score_form(music: &Music) -> Option<ScoreForm> {
    match music {
        Music::Chord(value) => Some(ScoreForm::Chord(value.clone())),
        Music::Melody(value) => Some(ScoreForm::Melody(value.clone())),
        Music::Progression(value) => Some(ScoreForm::Progression(value.clone())),
        Music::Counterpoint(value) => Some(ScoreForm::Counterpoint(value.clone())),
        Music::PianoRoll(value) => Some(ScoreForm::PianoRoll(value.clone())),
        Music::Note(_)
        | Music::Rest(_)
        | Music::Par(_)
        | Music::Seq(_)
        | Music::Arranger(_)
        | Music::MidiTrack(_)
        | Music::MidiFile(_) => None,
    }
}

fn flattened_staff(score: &Score) -> Result<Staff, SerialExtractionError> {
    let duration = score.body.duration();
    let mut atoms = Vec::new();
    score.body.voices(Time::from_integer(0), &mut atoms);
    let mut voices = BTreeMap::<u8, StaffVoice>::new();
    for (index, atom) in atoms.into_iter().enumerate() {
        let AtomRef::Note(note) = atom.atom else {
            continue;
        };
        push_derived_note(&mut voices, duration, index, atom.onset, note)?;
    }
    if voices.is_empty() {
        let voice_id = object_id("score/voice/silence")?;
        voices.insert(
            0,
            StaffVoice {
                id: voice_id,
                name: "Silence".to_owned(),
                duration,
                notes: Vec::new(),
            },
        );
    }
    Staff::new(voices.into_values().collect())
        .map_err(|error| SerialExtractionError::ScoreConversion(error.to_string()))
}

fn push_derived_note(
    voices: &mut BTreeMap<u8, StaffVoice>,
    duration: Time,
    index: usize,
    onset: Time,
    note: Note,
) -> Result<(), SerialExtractionError> {
    let channel = note.channel.0;
    let voice_id = object_id(format!("score/voice/channel-{channel}"))?;
    let entry = voices.entry(channel).or_insert_with(|| StaffVoice {
        id: voice_id.clone(),
        name: format!("Derived channel {channel}"),
        duration,
        notes: Vec::new(),
    });
    entry.notes.push(StaffNote {
        voice_id,
        note_id: object_id(format!("score/note/{index}"))?,
        event_id: object_id(format!("score/event/{index}"))?,
        onset,
        note,
    });
    Ok(())
}

fn object_id(value: impl Into<String>) -> Result<ObjectId, SerialExtractionError> {
    ObjectId::new(value).map_err(|error| SerialExtractionError::Identity(error.to_string()))
}
