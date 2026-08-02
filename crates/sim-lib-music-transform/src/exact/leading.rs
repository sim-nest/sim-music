//! Certified voice leading over exact score identities.

use sim_lib_discrete_graph::{
    Assignment, AssignmentOperation, AssignmentPolicy, CostMatrix, GraphError, verify_assignment,
};
pub use sim_lib_discrete_graph::{AssignmentCertificate, VoiceCrossingPolicy};
use sim_lib_music_core::{ObjectId, Staff, Time};
use sim_lib_pitch_core::Pitch;

use crate::TransformError;

/// One sounding note with the three exact score identities needed to trace it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactVoiceNote {
    /// Identity of the containing voice.
    pub voice_id: ObjectId,
    /// Identity of the logical note.
    pub note_id: ObjectId,
    /// Identity of this event.
    pub event_id: ObjectId,
    /// Sounding pitch, including register.
    pub pitch: Pitch,
}

/// Exact sounding notes at one score boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactVoicing {
    /// Boundary in exact whole-note time.
    pub at: Time,
    /// Notes sorted by pitch, then voice and event identity.
    pub notes: Vec<ExactVoiceNote>,
}

impl ExactVoicing {
    /// Reads all notes sounding at `at` from an identity-bearing staff.
    ///
    /// Half-open note spans are used: an event is sounding exactly when
    /// `onset <= at < end`.
    pub fn from_staff(staff: &Staff, at: Time) -> Result<Self, TransformError> {
        if at < Time::from_integer(0) || at > staff.duration() {
            return Err(TransformError::InvalidTransformOutput {
                transform: "exact-voicing",
                reason: "voicing boundary lies outside the staff",
            });
        }
        let mut notes = staff
            .notes()
            .filter(|note| note.onset <= at && at < note.end())
            .map(|note| ExactVoiceNote {
                voice_id: note.voice_id.clone(),
                note_id: note.note_id.clone(),
                event_id: note.event_id.clone(),
                pitch: note.note.pitch,
            })
            .collect::<Vec<_>>();
        notes.sort_by(|left, right| {
            left.pitch
                .cmp(&right.pitch)
                .then_with(|| left.voice_id.cmp(&right.voice_id))
                .then_with(|| left.event_id.cmp(&right.event_id))
        });
        Ok(Self { at, notes })
    }
}

/// Norm used to score semitone motion.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VoiceLeadingMetric {
    /// Sum literal absolute semitone distances.
    AbsoluteSemitones,
    /// Sum squared semitone distances, making large leaps disproportionately
    /// expensive.
    SquaredSemitones,
}

/// Explicit costs and structural policy for exact voice leading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceLeadingPolicy {
    /// Cost of a target voice entering without a source.
    pub entrance_cost: i64,
    /// Cost of a source voice leaving without a target.
    pub departure_cost: i64,
    /// Incremental cost for a source supplying an additional target.
    pub doubling_cost: Option<i64>,
    /// Whether the pitch-sorted voices may cross.
    pub voice_crossing: VoiceCrossingPolicy,
    /// Motion norm.
    pub metric: VoiceLeadingMetric,
}

impl VoiceLeadingPolicy {
    /// Builds a squared-distance policy with no doubling and crossings allowed.
    pub fn new(entrance_cost: i64, departure_cost: i64) -> Self {
        Self {
            entrance_cost,
            departure_cost,
            doubling_cost: None,
            voice_crossing: VoiceCrossingPolicy::Allow,
            metric: VoiceLeadingMetric::SquaredSemitones,
        }
    }

    /// Enables source doubling at the supplied incremental cost.
    pub fn with_doubling(mut self, cost: i64) -> Self {
        self.doubling_cost = Some(cost);
        self
    }

    /// Sets the voice-crossing policy.
    pub fn with_voice_crossing(mut self, policy: VoiceCrossingPolicy) -> Self {
        self.voice_crossing = policy;
        self
    }

    /// Sets the motion norm.
    pub fn with_metric(mut self, metric: VoiceLeadingMetric) -> Self {
        self.metric = metric;
        self
    }
}

/// Identity-resolved interpretation of one generic assignment operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoiceLeadingMotion {
    /// One source voice moves to one target voice.
    Move {
        /// Exact source note.
        source: ExactVoiceNote,
        /// Exact target note.
        target: ExactVoiceNote,
        /// Signed target-minus-source semitone motion.
        semitones: i64,
        /// Cost charged by the selected metric.
        cost: i64,
    },
    /// One source voice also supplies another target.
    Double {
        /// Exact reused source note.
        source: ExactVoiceNote,
        /// Exact additional target note.
        target: ExactVoiceNote,
        /// Signed target-minus-source semitone motion.
        semitones: i64,
        /// Pair motion plus configured doubling cost.
        cost: i64,
    },
    /// A target enters without a source.
    Enter {
        /// Exact target note.
        target: ExactVoiceNote,
        /// Configured entrance cost.
        cost: i64,
    },
    /// A source leaves without a target.
    Leave {
        /// Exact source note.
        source: ExactVoiceNote,
        /// Configured departure cost.
        cost: i64,
    },
}

/// Certified minimum-cost transition between two exact voicings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceLeading {
    /// Exact source voicing.
    pub source: ExactVoicing,
    /// Exact target voicing.
    pub target: ExactVoicing,
    /// Generic optimal assignment and certificate.
    pub assignment: Assignment<i64>,
    /// Assignment operations resolved back to exact score identities.
    pub motions: Vec<VoiceLeadingMotion>,
}

/// Finds certified minimum-cost voice leading without factorial permutation
/// search.
pub fn voice_leading(
    source: &ExactVoicing,
    target: &ExactVoicing,
    policy: &VoiceLeadingPolicy,
) -> Result<VoiceLeading, TransformError> {
    let costs = voice_costs(source, target, policy.metric)?;
    let assignment_policy = assignment_policy(source, target, policy);
    let assignment =
        sim_lib_discrete_graph::min_cost_assignment(&costs, assignment_policy.clone())?;
    let motions = resolve_motions(source, target, &assignment);
    let leading = VoiceLeading {
        source: source.clone(),
        target: target.clone(),
        assignment,
        motions,
    };
    verify_voice_leading(&leading, policy)?;
    Ok(leading)
}

/// Re-checks exact endpoints, operation projection, and the discrete optimality
/// certificate.
pub fn verify_voice_leading(
    leading: &VoiceLeading,
    policy: &VoiceLeadingPolicy,
) -> Result<(), TransformError> {
    let costs = voice_costs(&leading.source, &leading.target, policy.metric)?;
    let assignment_policy = assignment_policy(&leading.source, &leading.target, policy);
    verify_assignment(&costs, &assignment_policy, &leading.assignment)?;
    if leading.motions != resolve_motions(&leading.source, &leading.target, &leading.assignment) {
        return Err(TransformError::InvalidTransformOutput {
            transform: "voice-leading",
            reason: "identity-resolved motions disagree with the assignment",
        });
    }
    Ok(())
}

/// Aggregate certificate for a sequence of independently certified legs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceLeadingPathCertificate {
    /// Certified optimum for each adjacent leg.
    pub leg_costs: Vec<i64>,
    /// Checked sum of all leg costs.
    pub total_cost: i64,
}

/// Certified adjacent voice-leading path through an exact progression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceLeadingPath {
    /// Input voicings in exact time order.
    pub voicings: Vec<ExactVoicing>,
    /// One transition for every adjacent pair.
    pub legs: Vec<VoiceLeading>,
    /// Aggregate path certificate.
    pub certificate: VoiceLeadingPathCertificate,
}

/// Finds every adjacent optimum and returns their checked path certificate.
pub fn voice_leading_path(
    voicings: &[ExactVoicing],
    policy: &VoiceLeadingPolicy,
) -> Result<VoiceLeadingPath, TransformError> {
    if voicings.windows(2).any(|pair| pair[0].at > pair[1].at) {
        return Err(TransformError::InvalidTransformOutput {
            transform: "voice-leading-path",
            reason: "voicings must be in non-decreasing exact time order",
        });
    }
    let mut legs = Vec::with_capacity(voicings.len().saturating_sub(1));
    let mut leg_costs = Vec::with_capacity(voicings.len().saturating_sub(1));
    let mut total_cost = 0_i64;
    for pair in voicings.windows(2) {
        let leg = voice_leading(&pair[0], &pair[1], policy)?;
        total_cost = total_cost
            .checked_add(leg.assignment.total_cost)
            .ok_or_else(|| GraphError::WeightOverflow("voice-leading path total".to_owned()))?;
        leg_costs.push(leg.assignment.total_cost);
        legs.push(leg);
    }
    let path = VoiceLeadingPath {
        voicings: voicings.to_vec(),
        legs,
        certificate: VoiceLeadingPathCertificate {
            leg_costs,
            total_cost,
        },
    };
    verify_voice_leading_path(&path, policy)?;
    Ok(path)
}

/// Re-checks all leg certificates, adjacency, and the aggregate path total.
pub fn verify_voice_leading_path(
    path: &VoiceLeadingPath,
    policy: &VoiceLeadingPolicy,
) -> Result<(), TransformError> {
    if path.legs.len() != path.voicings.len().saturating_sub(1)
        || path.certificate.leg_costs.len() != path.legs.len()
    {
        return Err(TransformError::InvalidTransformOutput {
            transform: "voice-leading-path",
            reason: "path dimensions do not agree",
        });
    }
    let mut total = 0_i64;
    for (index, leg) in path.legs.iter().enumerate() {
        if leg.source != path.voicings[index] || leg.target != path.voicings[index + 1] {
            return Err(TransformError::InvalidTransformOutput {
                transform: "voice-leading-path",
                reason: "path leg endpoints do not join",
            });
        }
        verify_voice_leading(leg, policy)?;
        if path.certificate.leg_costs[index] != leg.assignment.total_cost {
            return Err(TransformError::InvalidTransformOutput {
                transform: "voice-leading-path",
                reason: "path leg cost disagrees with its assignment",
            });
        }
        total = total
            .checked_add(leg.assignment.total_cost)
            .ok_or_else(|| GraphError::WeightOverflow("voice-leading path total".to_owned()))?;
    }
    if total != path.certificate.total_cost {
        return Err(TransformError::InvalidTransformOutput {
            transform: "voice-leading-path",
            reason: "path total disagrees with its certified legs",
        });
    }
    Ok(())
}

/// One directed transition in an exact voicing-change palette.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoicingChange {
    /// Source palette index.
    pub source: usize,
    /// Target palette index.
    pub target: usize,
    /// Certified exact-identity transition.
    pub leading: VoiceLeading,
}

/// All directed transitions among a finite exact voicing palette.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoicingChangePalette {
    /// Palette voicings.
    pub voicings: Vec<ExactVoicing>,
    /// Unique directed changes in `(source, target)` order.
    pub changes: Vec<VoicingChange>,
}

impl VoicingChangePalette {
    /// Returns all changes leaving `source`, or an empty slice iterator for a
    /// dead end.
    pub fn outgoing(&self, source: usize) -> impl Iterator<Item = &VoicingChange> {
        self.changes
            .iter()
            .filter(move |change| change.source == source)
    }
}

/// Builds a duplicate-free, deterministic palette of every directed transition
/// between distinct exact voicings.
pub fn voicing_change_palette(
    voicings: &[ExactVoicing],
    policy: &VoiceLeadingPolicy,
) -> Result<VoicingChangePalette, TransformError> {
    let mut changes = Vec::new();
    for source in 0..voicings.len() {
        for target in 0..voicings.len() {
            if source == target {
                continue;
            }
            changes.push(VoicingChange {
                source,
                target,
                leading: voice_leading(&voicings[source], &voicings[target], policy)?,
            });
        }
    }
    Ok(VoicingChangePalette {
        voicings: voicings.to_vec(),
        changes,
    })
}

fn voice_costs(
    source: &ExactVoicing,
    target: &ExactVoicing,
    metric: VoiceLeadingMetric,
) -> Result<CostMatrix<i64>, TransformError> {
    let mut values = Vec::with_capacity(source.notes.len() * target.notes.len());
    for from in &source.notes {
        for to in &target.notes {
            let distance = i64::from(to.pitch.semitone()) - i64::from(from.pitch.semitone());
            let absolute = distance.abs();
            values.push(match metric {
                VoiceLeadingMetric::AbsoluteSemitones => absolute,
                VoiceLeadingMetric::SquaredSemitones => {
                    absolute.checked_mul(absolute).ok_or_else(|| {
                        GraphError::WeightOverflow("squared voice-leading distance".to_owned())
                    })?
                }
            });
        }
    }
    Ok(CostMatrix::new(
        source.notes.len(),
        target.notes.len(),
        values,
    )?)
}

fn assignment_policy(
    source: &ExactVoicing,
    target: &ExactVoicing,
    policy: &VoiceLeadingPolicy,
) -> AssignmentPolicy<i64> {
    let assignment = AssignmentPolicy::new(
        vec![policy.entrance_cost; target.notes.len()],
        vec![policy.departure_cost; source.notes.len()],
    )
    .with_voice_crossing(policy.voice_crossing);
    match policy.doubling_cost {
        Some(cost) => assignment.with_doubling(vec![cost; source.notes.len()]),
        None => assignment,
    }
}

fn resolve_motions(
    source: &ExactVoicing,
    target: &ExactVoicing,
    assignment: &Assignment<i64>,
) -> Vec<VoiceLeadingMotion> {
    assignment
        .operations
        .iter()
        .map(|operation| match operation {
            AssignmentOperation::Match {
                source: from,
                target: to,
                cost,
            } => VoiceLeadingMotion::Move {
                source: source.notes[*from].clone(),
                target: target.notes[*to].clone(),
                semitones: i64::from(target.notes[*to].pitch.semitone())
                    - i64::from(source.notes[*from].pitch.semitone()),
                cost: *cost,
            },
            AssignmentOperation::Double {
                source: from,
                target: to,
                cost,
            } => VoiceLeadingMotion::Double {
                source: source.notes[*from].clone(),
                target: target.notes[*to].clone(),
                semitones: i64::from(target.notes[*to].pitch.semitone())
                    - i64::from(source.notes[*from].pitch.semitone()),
                cost: *cost,
            },
            AssignmentOperation::Insert { target: to, cost } => VoiceLeadingMotion::Enter {
                target: target.notes[*to].clone(),
                cost: *cost,
            },
            AssignmentOperation::Delete { source: from, cost } => VoiceLeadingMotion::Leave {
                source: source.notes[*from].clone(),
                cost: *cost,
            },
        })
        .collect()
}
