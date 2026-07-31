//! Bounded hash-candidate and exact-verified repeated-pattern discovery.

mod union;

use std::collections::BTreeMap;

use sim_lib_discrete_search::{
    NeverInterrupt, SearchControl, SearchProblem, SearchReceipt, SearchStep, solve,
};
use sim_lib_music_core::{ObjectId, Time};

use crate::{
    AnalysisError, AnalysisEvent, AnalysisTransform, SimilarityInvariances, TimeSpan, event_span,
    sequence_extent,
};
use union::UnionFind;

/// Whether occurrences of one reported pattern may share source events.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PatternOverlapPolicy {
    /// Retain every exact-verified occurrence, including overlaps.
    #[default]
    Allow,
    /// Greedily retain stable, earliest occurrences with disjoint event indexes.
    DisallowSharedEvents,
}

/// Structural, support, overlap, and deterministic resource policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternDiscoveryPolicy {
    /// Minimum events in one pattern occurrence.
    pub min_events: usize,
    /// Maximum events in one pattern occurrence.
    pub max_events: usize,
    /// Minimum retained occurrences required for a report row.
    pub min_support: usize,
    /// Pitch/time invariances used by canonical hash keys and exact verification.
    pub invariances: SimilarityInvariances,
    /// Whether occurrences may share source events.
    pub overlap: PatternOverlapPolicy,
    /// Maximum number of materialized windows.
    pub max_windows: usize,
    /// Maximum number of same-length, same-hash pairs admitted for verification.
    pub max_candidate_pairs: usize,
    /// Maximum estimated canonical-key bytes.
    pub max_hash_bytes: usize,
    /// Generic bounded-search order, work, result, frontier, memory, and seed policy.
    pub search: SearchControl,
}

/// Deterministic preflight facts for candidate memory and work shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternResourceReceipt {
    /// Canonical windows materialized.
    pub windows: usize,
    /// Same-hash pairs handed to exact verification.
    pub candidate_pairs: usize,
    /// Conservative canonical-key byte estimate.
    pub hash_bytes: usize,
}

/// One exact-verified occurrence and its identity-preserving transform.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternOccurrence {
    /// Stable identity within this report.
    pub occurrence_id: String,
    /// Exact half-open source span.
    pub span: TimeSpan,
    /// Exact affine transform from the pattern prototype to this occurrence.
    pub transform: AnalysisTransform,
    /// Verification mismatch cost; exact occurrences have zero cost.
    pub cost: f64,
    /// Stable event identities covered by this occurrence.
    pub event_ids: Vec<ObjectId>,
}

/// One repeated exact canonical pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    /// Stable content/position identity.
    pub id: String,
    /// Number of events per occurrence.
    pub event_count: usize,
    /// Prototype event identities.
    pub prototype_event_ids: Vec<ObjectId>,
    /// Exact-verified occurrences after overlap policy.
    pub occurrences: Vec<PatternOccurrence>,
}

/// Repeated patterns plus bounded search and candidate-resource evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternReport {
    /// Complete support, invariance, overlap, memory, and search policy.
    pub policy: PatternDiscoveryPolicy,
    /// Stable pattern rows, longest then earliest.
    pub patterns: Vec<Pattern>,
    /// Unmodified generic search receipt for exact pair verification.
    pub search: SearchReceipt,
    /// Candidate count and memory preflight.
    pub resources: PatternResourceReceipt,
}

/// Discovers repeated contiguous event patterns through hash candidates followed
/// by exact canonical verification under generic bounded search.
pub fn discover_patterns(
    events: &[AnalysisEvent],
    policy: &PatternDiscoveryPolicy,
) -> Result<PatternReport, AnalysisError> {
    validate_policy(events, policy)?;
    let events = ordered(events);
    let mut windows = Vec::new();
    let mut hash_bytes = 0usize;
    for length in policy.min_events..=policy.max_events.min(events.len()) {
        for start in 0..=events.len() - length {
            if windows.len() == policy.max_windows {
                return resource_error("pattern windows", windows.len() + 1, policy.max_windows);
            }
            let canonical = canonicalize(&events[start..start + length], policy.invariances)?;
            let bytes = canonical
                .len()
                .checked_mul(std::mem::size_of::<CanonicalEvent>())
                .ok_or(AnalysisError::ResourceLimit {
                    resource: "pattern hash bytes",
                    required: u64::MAX,
                    maximum: policy.max_hash_bytes as u64,
                })?;
            hash_bytes = hash_bytes
                .checked_add(bytes)
                .ok_or(AnalysisError::ResourceLimit {
                    resource: "pattern hash bytes",
                    required: u64::MAX,
                    maximum: policy.max_hash_bytes as u64,
                })?;
            if hash_bytes > policy.max_hash_bytes {
                return resource_error("pattern hash bytes", hash_bytes, policy.max_hash_bytes);
            }
            windows.push(Window {
                start,
                length,
                hash: canonical_hash(&canonical),
                canonical,
            });
        }
    }

    let mut groups = BTreeMap::<(usize, u64), Vec<usize>>::new();
    for (index, window) in windows.iter().enumerate() {
        groups
            .entry((window.length, window.hash))
            .or_default()
            .push(index);
    }
    let mut pairs = Vec::new();
    for indexes in groups
        .values()
        .filter(|indexes| indexes.len() >= policy.min_support)
    {
        for left in 0..indexes.len() {
            for right in left + 1..indexes.len() {
                if pairs.len() == policy.max_candidate_pairs {
                    return resource_error(
                        "pattern candidate pairs",
                        pairs.len() + 1,
                        policy.max_candidate_pairs,
                    );
                }
                pairs.push((indexes[left], indexes[right]));
            }
        }
    }

    let problem = PairVerificationProblem {
        windows: &windows,
        pairs: &pairs,
    };
    let run = solve(&problem, policy.search.clone(), &NeverInterrupt);
    let mut union = UnionFind::new(windows.len());
    for pair in &run.outputs {
        union.join(pair.left, pair.right);
    }
    let mut components = BTreeMap::<usize, Vec<usize>>::new();
    for pair in &run.outputs {
        for index in [pair.left, pair.right] {
            components.entry(union.root(index)).or_default().push(index);
        }
    }
    for component in components.values_mut() {
        component.sort_by_key(|index| windows[*index].start);
        component.dedup();
    }

    let mut patterns = Vec::new();
    for indexes in components.into_values() {
        let retained = apply_overlap(indexes, &windows, policy.overlap);
        if retained.len() < policy.min_support {
            continue;
        }
        let prototype_index = retained[0];
        let prototype = &windows[prototype_index];
        let prototype_events = &events[prototype.start..prototype.start + prototype.length];
        let id = format!(
            "pattern/{:016x}/{}/{}",
            prototype.hash, prototype.length, prototype.start
        );
        let occurrences = retained
            .iter()
            .enumerate()
            .map(|(ordinal, index)| {
                let window = &windows[*index];
                let occurrence = &events[window.start..window.start + window.length];
                Ok(PatternOccurrence {
                    occurrence_id: format!("{id}/occurrence/{ordinal}"),
                    span: event_span(occurrence)?,
                    transform: occurrence_transform(
                        prototype_events,
                        occurrence,
                        policy.invariances,
                    ),
                    cost: 0.0,
                    event_ids: occurrence
                        .iter()
                        .map(|event| event.event_id.clone())
                        .collect(),
                })
            })
            .collect::<Result<Vec<_>, AnalysisError>>()?;
        patterns.push(Pattern {
            id,
            event_count: prototype.length,
            prototype_event_ids: prototype_events
                .iter()
                .map(|event| event.event_id.clone())
                .collect(),
            occurrences,
        });
    }
    patterns.sort_by(|left, right| {
        right
            .event_count
            .cmp(&left.event_count)
            .then_with(|| right.occurrences.len().cmp(&left.occurrences.len()))
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(PatternReport {
        policy: policy.clone(),
        patterns,
        search: run.receipt,
        resources: PatternResourceReceipt {
            windows: windows.len(),
            candidate_pairs: pairs.len(),
            hash_bytes,
        },
    })
}

fn validate_policy(
    events: &[AnalysisEvent],
    policy: &PatternDiscoveryPolicy,
) -> Result<(), AnalysisError> {
    if events.is_empty() {
        return Err(AnalysisError::InvalidInput {
            field: "pattern events",
            reason: "at least one event is required".to_owned(),
        });
    }
    if policy.min_events == 0 || policy.max_events < policy.min_events {
        return Err(AnalysisError::InvalidPolicy {
            field: "pattern length",
            reason: "lengths must satisfy 1 <= min_events <= max_events".to_owned(),
        });
    }
    if policy.min_support < 2 {
        return Err(AnalysisError::InvalidPolicy {
            field: "min_support",
            reason: "repeated patterns require support of at least two".to_owned(),
        });
    }
    for (field, value) in [
        ("max_windows", policy.max_windows),
        ("max_candidate_pairs", policy.max_candidate_pairs),
        ("max_hash_bytes", policy.max_hash_bytes),
    ] {
        if value == 0 {
            return Err(AnalysisError::InvalidPolicy {
                field,
                reason: "resource ceiling must be positive".to_owned(),
            });
        }
    }
    Ok(())
}

fn ordered(events: &[AnalysisEvent]) -> Vec<AnalysisEvent> {
    let mut events = events.to_vec();
    events.sort_by(|left, right| {
        left.onset
            .cmp(&right.onset)
            .then_with(|| left.pitch.cmp(&right.pitch))
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    events
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalEvent {
    pitch: i32,
    onset: Time,
    duration: Time,
}

#[derive(Clone, Debug)]
struct Window {
    start: usize,
    length: usize,
    hash: u64,
    canonical: Vec<CanonicalEvent>,
}

fn canonicalize(
    events: &[AnalysisEvent],
    invariances: SimilarityInvariances,
) -> Result<Vec<CanonicalEvent>, AnalysisError> {
    let first_pitch = if invariances.transposition {
        events[0].pitch.semitone()
    } else {
        0
    };
    let first_onset = events[0].onset;
    let basis = if invariances.time_scale {
        sequence_extent(events)
    } else {
        Time::from_integer(1)
    };
    if basis <= Time::from_integer(0) {
        return Err(AnalysisError::InvalidInput {
            field: "pattern time scale",
            reason: "time-scale-invariant windows need a positive extent or duration".to_owned(),
        });
    }
    Ok(events
        .iter()
        .map(|event| CanonicalEvent {
            pitch: event.pitch.semitone() - first_pitch,
            onset: (event.onset - first_onset) / basis,
            duration: event.duration / basis,
        })
        .collect())
}

fn canonical_hash(events: &[CanonicalEvent]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for event in events {
        for value in [
            i64::from(event.pitch),
            *event.onset.numer(),
            *event.onset.denom(),
            *event.duration.numer(),
            *event.duration.denom(),
        ] {
            for byte in value.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[derive(Clone, Debug)]
struct PairVerificationProblem<'a> {
    windows: &'a [Window],
    pairs: &'a [(usize, usize)],
}

#[derive(Clone, Debug)]
struct PairState {
    cursor: usize,
    verify: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PairChoice {
    Verify,
    Advance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct VerifiedPair {
    left: usize,
    right: usize,
}

impl SearchProblem for PairVerificationProblem<'_> {
    type State = PairState;
    type Choice = PairChoice;
    type Output = VerifiedPair;

    fn initial_state(&self) -> Self::State {
        PairState {
            cursor: 0,
            verify: false,
        }
    }

    fn expand(&self, state: &Self::State, out: &mut Vec<Self::Choice>) {
        if !state.verify && state.cursor < self.pairs.len() {
            out.extend([PairChoice::Verify, PairChoice::Advance]);
        }
    }

    fn apply(&self, state: &Self::State, choice: &Self::Choice) -> SearchStep<Self::State> {
        match choice {
            PairChoice::Verify => SearchStep::Continue(PairState {
                cursor: state.cursor,
                verify: true,
            }),
            PairChoice::Advance => SearchStep::Continue(PairState {
                cursor: state.cursor + 1,
                verify: false,
            }),
        }
    }

    fn finish(&self, state: &Self::State) -> Option<Self::Output> {
        if !state.verify {
            return None;
        }
        let (left, right) = self.pairs[state.cursor];
        (self.windows[left].canonical == self.windows[right].canonical)
            .then_some(VerifiedPair { left, right })
    }
}

fn apply_overlap(
    mut indexes: Vec<usize>,
    windows: &[Window],
    policy: PatternOverlapPolicy,
) -> Vec<usize> {
    indexes.sort_by_key(|index| windows[*index].start);
    if policy == PatternOverlapPolicy::Allow {
        return indexes;
    }
    let mut retained = Vec::new();
    let mut next_start = 0usize;
    for index in indexes {
        let window = &windows[index];
        if retained.is_empty() || window.start >= next_start {
            next_start = window.start + window.length;
            retained.push(index);
        }
    }
    retained
}

fn occurrence_transform(
    prototype: &[AnalysisEvent],
    occurrence: &[AnalysisEvent],
    invariances: SimilarityInvariances,
) -> AnalysisTransform {
    let transposition = if invariances.transposition {
        occurrence[0].pitch.semitone() - prototype[0].pitch.semitone()
    } else {
        0
    };
    let prototype_extent = sequence_extent(prototype);
    let occurrence_extent = sequence_extent(occurrence);
    let time_scale = if invariances.time_scale && prototype_extent > Time::from_integer(0) {
        occurrence_extent / prototype_extent
    } else {
        Time::from_integer(1)
    };
    AnalysisTransform {
        transposition,
        time_scale,
        time_shift: occurrence[0].onset - prototype[0].onset * time_scale,
    }
}

fn resource_error<T>(
    resource: &'static str,
    required: usize,
    maximum: usize,
) -> Result<T, AnalysisError> {
    Err(AnalysisError::ResourceLimit {
        resource,
        required: u64::try_from(required).unwrap_or(u64::MAX),
        maximum: u64::try_from(maximum).unwrap_or(u64::MAX),
    })
}
