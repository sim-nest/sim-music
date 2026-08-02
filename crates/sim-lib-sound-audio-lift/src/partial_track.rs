//! Bounded polyphonic partial tracking over harmonic-comb candidates.

use sim_lib_discrete_graph::{
    AlgorithmControl, AlgorithmReceipt, AlignmentMemory, AlignmentWindow, AssignmentOperation,
    AssignmentPolicy, CostMatrix, DtwPolicy, GapPolicy, NeverInterrupt, VoiceCrossingPolicy,
    dynamic_time_warp_with_control, min_cost_assignment_with_control,
};
use sim_lib_pitch_core::Pitch;
use sim_lib_sound_core::{Amplitude, Frequency};
use sim_lib_sound_tuning::Tuning;

use crate::{
    AudioLiftError, AudioLiftFrame, AudioLiftOptions, AudioLiftReport, PitchCandidate,
    pipeline::analyze,
};

/// Whether assignment may swap the frequency order of two continuing partials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartialCrossingPolicy {
    /// Continue the closest trajectories even when their identities cross.
    Allow,
    /// Preserve low-to-high track order across each assignment.
    Forbid,
}

/// Bounded birth, death, crossing, and continuity policy for partial tracks.
#[derive(Clone, Debug, PartialEq)]
pub struct PartialTrackPolicy {
    /// Maximum number of tracks alive at once.
    pub max_tracks: usize,
    /// Missing frames tolerated before a track dies.
    pub max_gap_frames: usize,
    /// Greatest admitted continuation distance in cents.
    pub max_jump_cents: f64,
    /// Assignment cost for starting a new partial.
    pub birth_cost: f64,
    /// Assignment cost for leaving a partial unmatched for one frame.
    pub death_cost: f64,
    /// Voice-order policy for simultaneous partials.
    pub crossing: PartialCrossingPolicy,
    /// Minimum number of points retained as a completed track.
    pub min_points: usize,
    /// Maximum aggregate graph work across assignment and DTW proofs.
    pub max_work: u64,
    /// Maximum retained graph cells in any one proof.
    pub max_memory_cells: usize,
    /// Radius of the DTW continuity certificate around the diagonal.
    pub dtw_radius: usize,
}

impl Default for PartialTrackPolicy {
    fn default() -> Self {
        Self {
            max_tracks: 8,
            max_gap_frames: 1,
            max_jump_cents: 180.0,
            birth_cost: 120.0,
            death_cost: 120.0,
            crossing: PartialCrossingPolicy::Allow,
            min_points: 2,
            max_work: 500_000,
            max_memory_cells: 65_536,
            dtw_radius: 2,
        }
    }
}

impl PartialTrackPolicy {
    /// Validates every structural and resource bound.
    pub fn validate(&self) -> Result<(), AudioLiftError> {
        if self.max_tracks == 0
            || self.min_points == 0
            || self.max_work == 0
            || self.max_memory_cells == 0
        {
            return Err(AudioLiftError::InvalidPitchBound);
        }
        if !self.max_jump_cents.is_finite()
            || self.max_jump_cents <= 0.0
            || !self.birth_cost.is_finite()
            || self.birth_cost < 0.0
            || !self.death_cost.is_finite()
            || self.death_cost < 0.0
        {
            return Err(AudioLiftError::InvalidPitchThreshold);
        }
        Ok(())
    }
}

/// Exact frame location retained by spectral candidates and links.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartialFrameProvenance {
    /// Existing audio-lift frame index.
    pub frame_index: usize,
    /// Onset in source PCM samples.
    pub onset_sample: usize,
    /// Source samples represented by the frame.
    pub duration_samples: usize,
    /// PCM sample rate.
    pub sample_rate: u32,
}

/// One harmonic-comb candidate before temporal assignment.
#[derive(Clone, Debug, PartialEq)]
pub struct PartialCandidate {
    /// Stable candidate index within the frame's frequency-sorted row.
    pub candidate_index: usize,
    /// Nearest tuned pitch.
    pub pitch: Pitch,
    /// Interpolated spectral frequency.
    pub frequency: Frequency,
    /// Lower half-bin uncertainty bound.
    pub lower_frequency: Frequency,
    /// Upper half-bin uncertainty bound.
    pub upper_frequency: Frequency,
    /// Spectral amplitude.
    pub amplitude: Amplitude,
    /// Harmonic-comb confidence.
    pub confidence: f64,
    /// Number of supporting comb teeth.
    pub harmonic_count: usize,
    /// Difference from the nearest tuned pitch.
    pub cents_error: f64,
    /// Exact source-frame provenance.
    pub provenance: PartialFrameProvenance,
}

/// Why an otherwise available temporal link was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartialLinkRejectionReason {
    /// Frequency displacement exceeded the admitted continuation jump.
    JumpLimit,
    /// A birth could not be retained because the live-track cap was full.
    TrackLimit,
}

/// Candidate temporal hypothesis excluded by explicit policy.
#[derive(Clone, Debug, PartialEq)]
pub struct RejectedPartialLink {
    /// Existing track considered as source, absent for a rejected birth.
    pub track: Option<usize>,
    /// Frame candidate considered as target.
    pub candidate: usize,
    /// Absolute displacement when a source track existed.
    pub cents_distance: Option<f64>,
    /// Policy that rejected the link.
    pub reason: PartialLinkRejectionReason,
}

/// Accepted assignment between one track and one frame candidate.
#[derive(Clone, Debug, PartialEq)]
pub struct PartialLink {
    /// Stable track id.
    pub track: usize,
    /// Candidate index within [`PartialTrackFrame::candidates`].
    pub candidate: usize,
    /// Assignment displacement in cents; zero for births.
    pub cents_distance: f64,
    /// Whether this link created the track.
    pub birth: bool,
}

/// Auditable candidate, link, and rejection evidence for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct PartialTrackFrame {
    /// Exact source-frame provenance.
    pub provenance: PartialFrameProvenance,
    /// Frequency-sorted harmonic-comb candidates.
    pub candidates: Vec<PartialCandidate>,
    /// Accepted temporal assignments.
    pub links: Vec<PartialLink>,
    /// Candidate links rejected before assignment or birth.
    pub rejected_links: Vec<RejectedPartialLink>,
    /// Tracks whose gap policy expired at this frame.
    pub deaths: Vec<usize>,
    /// Shared assignment algorithm receipt, absent when one side was empty.
    pub assignment_receipt: Option<AlgorithmReceipt>,
}

/// Why a completed partial stopped receiving points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartialDeathReason {
    /// Missing-frame tolerance was exceeded.
    GapLimit,
    /// Source PCM ended while the track remained alive.
    StreamEnd,
}

/// One point in an identity-bearing partial trajectory.
#[derive(Clone, Debug, PartialEq)]
pub struct PartialTrackPoint {
    /// Assigned spectral candidate.
    pub candidate: PartialCandidate,
    /// Number of missing frames since the preceding point.
    pub preceding_gap_frames: usize,
}

/// Compact DTW evidence comparing a trajectory with its endpoint trend.
#[derive(Clone, Debug, PartialEq)]
pub struct PartialContinuityEvidence {
    /// Minimum DTW score in cents.
    pub score: f64,
    /// Stable number of edit/match steps.
    pub steps: usize,
    /// Shared graph receipt.
    pub receipt: AlgorithmReceipt,
}

/// A bounded multi-frame partial trajectory.
#[derive(Clone, Debug, PartialEq)]
pub struct PartialTrack {
    /// Stable birth-order id.
    pub id: usize,
    /// Assigned points.
    pub points: Vec<PartialTrackPoint>,
    /// Mean candidate confidence.
    pub confidence: f64,
    /// Minimum frequency admitted by point uncertainty.
    pub lower_frequency: Frequency,
    /// Maximum frequency admitted by point uncertainty.
    pub upper_frequency: Frequency,
    /// Explicit termination reason.
    pub death: PartialDeathReason,
    /// Certified DTW continuity against the endpoint trend.
    pub continuity: PartialContinuityEvidence,
}

/// Polyphonic tracking result extending existing harmonic-comb frames.
#[derive(Clone, Debug, PartialEq)]
pub struct PolyphonicPitchTrack {
    /// Exact policy applied.
    pub policy: PartialTrackPolicy,
    /// Per-frame candidates, accepted links, and rejected hypotheses.
    pub frames: Vec<PartialTrackFrame>,
    /// Completed tracks that met `min_points`.
    pub tracks: Vec<PartialTrack>,
    /// Short-lived tracks rejected at completion.
    pub rejected_tracks: Vec<PartialTrack>,
    /// Aggregate assignment and DTW work.
    pub work_used: u64,
}

/// Runs the existing harmonic-comb analyzer, then extends its candidates into
/// bounded multi-partial trajectories.
pub fn polyphonic_pitch_track(
    samples: &[f32],
    sample_rate: u32,
    tuning: &dyn Tuning,
    lift: &AudioLiftOptions,
    policy: &PartialTrackPolicy,
) -> Result<AudioLiftReport<PolyphonicPitchTrack>, AudioLiftError> {
    let lifted = analyze(samples, sample_rate, tuning, lift, true)?;
    let tracked = track_partials(&lifted.value.frames, sample_rate, policy)?;
    Ok(AudioLiftReport {
        value: tracked,
        diagnostics: lifted.diagnostics,
    })
}

/// Assigns existing audio-lift frame candidates into bounded partial tracks.
pub fn track_partials(
    frames: &[AudioLiftFrame],
    sample_rate: u32,
    policy: &PartialTrackPolicy,
) -> Result<PolyphonicPitchTrack, AudioLiftError> {
    if sample_rate == 0 {
        return Err(AudioLiftError::InvalidSampleRate);
    }
    policy.validate()?;
    let mut work_used = 0u64;
    let mut next_id = 0usize;
    let mut active = Vec::<ActivePartial>::new();
    let mut completed = Vec::<RawPartial>::new();
    let mut evidence = Vec::with_capacity(frames.len());

    for frame in frames {
        let provenance = PartialFrameProvenance {
            frame_index: frame.index,
            onset_sample: frame.onset_sample,
            duration_samples: frame.duration_samples,
            sample_rate,
        };
        let mut candidates = frame
            .pitch_candidates
            .iter()
            .map(|candidate| partial_candidate(candidate, provenance.clone(), frame))
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.frequency.0.total_cmp(&right.frequency.0));
        for (index, candidate) in candidates.iter_mut().enumerate() {
            candidate.candidate_index = index;
        }
        active.sort_by(|left, right| {
            left.last_frequency()
                .total_cmp(&right.last_frequency())
                .then_with(|| left.raw.id.cmp(&right.raw.id))
        });
        let mut frame_evidence = PartialTrackFrame {
            provenance,
            candidates,
            links: Vec::new(),
            rejected_links: Vec::new(),
            deaths: Vec::new(),
            assignment_receipt: None,
        };
        assign_frame(
            &mut active,
            &mut completed,
            &mut next_id,
            &mut frame_evidence,
            policy,
            &mut work_used,
        )?;
        evidence.push(frame_evidence);
    }
    completed.extend(active.into_iter().map(|active| RawPartial {
        death: PartialDeathReason::StreamEnd,
        ..active.raw
    }));

    let mut tracks = Vec::new();
    let mut rejected_tracks = Vec::new();
    for raw in completed {
        let track = finish_track(raw, policy, &mut work_used)?;
        if track.points.len() >= policy.min_points {
            tracks.push(track);
        } else {
            rejected_tracks.push(track);
        }
    }
    tracks.sort_by_key(|track| track.id);
    rejected_tracks.sort_by_key(|track| track.id);
    Ok(PolyphonicPitchTrack {
        policy: policy.clone(),
        frames: evidence,
        tracks,
        rejected_tracks,
        work_used,
    })
}

#[derive(Clone, Debug)]
struct RawPartial {
    id: usize,
    points: Vec<PartialTrackPoint>,
    death: PartialDeathReason,
}

#[derive(Clone, Debug)]
struct ActivePartial {
    raw: RawPartial,
    gap_frames: usize,
}

impl ActivePartial {
    fn last_frequency(&self) -> f64 {
        self.raw
            .points
            .last()
            .expect("active tracks have points")
            .candidate
            .frequency
            .0
    }

    fn predicted_frequency(&self) -> f64 {
        let mut points = self.raw.points.iter().rev();
        let last = points
            .next()
            .expect("active tracks have points")
            .candidate
            .frequency
            .0;
        let Some(previous) = points.next() else {
            return last;
        };
        let previous = previous.candidate.frequency.0;
        2.0_f64.powf(2.0 * last.log2() - previous.log2())
    }
}

fn partial_candidate(
    candidate: &PitchCandidate,
    provenance: PartialFrameProvenance,
    frame: &AudioLiftFrame,
) -> PartialCandidate {
    let resolution = if frame.duration_samples == 0 {
        0.0
    } else {
        f64::from(provenance.sample_rate) / frame.duration_samples as f64
    };
    PartialCandidate {
        candidate_index: 0,
        pitch: candidate.pitch,
        frequency: candidate.frequency,
        lower_frequency: Frequency((candidate.frequency.0 - resolution / 2.0).max(0.0)),
        upper_frequency: Frequency(candidate.frequency.0 + resolution / 2.0),
        amplitude: candidate.amplitude,
        confidence: candidate.confidence,
        harmonic_count: candidate.harmonic_count,
        cents_error: candidate.cents_error,
        provenance,
    }
}

fn assign_frame(
    active: &mut Vec<ActivePartial>,
    completed: &mut Vec<RawPartial>,
    next_id: &mut usize,
    frame: &mut PartialTrackFrame,
    policy: &PartialTrackPolicy,
    work_used: &mut u64,
) -> Result<(), AudioLiftError> {
    if active.is_empty() {
        for candidate in 0..frame.candidates.len() {
            birth(active, next_id, frame, candidate, policy);
        }
        return Ok(());
    }
    if frame.candidates.is_empty() {
        expire_unmatched(active, completed, frame, policy, &vec![false; active.len()]);
        return Ok(());
    }

    let mut costs = Vec::with_capacity(active.len() * frame.candidates.len());
    for track in active.iter() {
        for candidate in &frame.candidates {
            let cents = cents_distance(track.predicted_frequency(), candidate.frequency.0);
            if cents > policy.max_jump_cents {
                costs.push(None);
                frame.rejected_links.push(RejectedPartialLink {
                    track: Some(track.raw.id),
                    candidate: candidate.candidate_index,
                    cents_distance: Some(cents),
                    reason: PartialLinkRejectionReason::JumpLimit,
                });
            } else {
                costs.push(Some(millicents(cents)));
            }
        }
    }
    let matrix = CostMatrix::from_optional(active.len(), frame.candidates.len(), costs)
        .map_err(graph_error)?;
    let crossing = match policy.crossing {
        PartialCrossingPolicy::Allow => VoiceCrossingPolicy::Allow,
        PartialCrossingPolicy::Forbid => VoiceCrossingPolicy::Forbid,
    };
    let assignment_policy = AssignmentPolicy::new(
        vec![millicents(policy.birth_cost); frame.candidates.len()],
        vec![millicents(policy.death_cost); active.len()],
    )
    .with_voice_crossing(crossing);
    let control = graph_control(policy, *work_used);
    let assignment =
        min_cost_assignment_with_control(&matrix, assignment_policy, &control, &NeverInterrupt)
            .map_err(graph_error)?;
    add_work(work_used, assignment.receipt.work_used, policy.max_work)?;
    frame.assignment_receipt = Some(assignment.receipt.clone());
    let mut matched = vec![false; active.len()];
    let mut births = Vec::new();
    for operation in assignment.operations {
        match operation {
            AssignmentOperation::Match {
                source,
                target,
                cost: _,
            } => {
                let cents_distance = cents_distance(
                    active[source].predicted_frequency(),
                    frame.candidates[target].frequency.0,
                );
                matched[source] = true;
                let gap = active[source].gap_frames;
                active[source].gap_frames = 0;
                active[source].raw.points.push(PartialTrackPoint {
                    candidate: frame.candidates[target].clone(),
                    preceding_gap_frames: gap,
                });
                frame.links.push(PartialLink {
                    track: active[source].raw.id,
                    candidate: target,
                    cents_distance,
                    birth: false,
                });
            }
            AssignmentOperation::Insert { target, .. } => births.push(target),
            AssignmentOperation::Delete { .. } => {}
            AssignmentOperation::Double { .. } => unreachable!("doubling is disabled"),
        }
    }
    expire_unmatched(active, completed, frame, policy, &matched);
    for candidate in births {
        birth(active, next_id, frame, candidate, policy);
    }
    Ok(())
}

fn birth(
    active: &mut Vec<ActivePartial>,
    next_id: &mut usize,
    frame: &mut PartialTrackFrame,
    candidate: usize,
    policy: &PartialTrackPolicy,
) {
    if active.len() >= policy.max_tracks {
        frame.rejected_links.push(RejectedPartialLink {
            track: None,
            candidate,
            cents_distance: None,
            reason: PartialLinkRejectionReason::TrackLimit,
        });
        return;
    }
    let id = *next_id;
    *next_id += 1;
    active.push(ActivePartial {
        raw: RawPartial {
            id,
            points: vec![PartialTrackPoint {
                candidate: frame.candidates[candidate].clone(),
                preceding_gap_frames: 0,
            }],
            death: PartialDeathReason::StreamEnd,
        },
        gap_frames: 0,
    });
    frame.links.push(PartialLink {
        track: id,
        candidate,
        cents_distance: 0.0,
        birth: true,
    });
}

fn expire_unmatched(
    active: &mut Vec<ActivePartial>,
    completed: &mut Vec<RawPartial>,
    frame: &mut PartialTrackFrame,
    policy: &PartialTrackPolicy,
    matched: &[bool],
) {
    for (index, track) in active.iter_mut().enumerate() {
        if !matched.get(index).copied().unwrap_or(false) {
            track.gap_frames += 1;
        }
    }
    let mut index = 0;
    while index < active.len() {
        if active[index].gap_frames > policy.max_gap_frames {
            let mut expired = active.remove(index).raw;
            expired.death = PartialDeathReason::GapLimit;
            frame.deaths.push(expired.id);
            completed.push(expired);
        } else {
            index += 1;
        }
    }
}

fn finish_track(
    raw: RawPartial,
    policy: &PartialTrackPolicy,
    work_used: &mut u64,
) -> Result<PartialTrack, AudioLiftError> {
    let frequencies = raw
        .points
        .iter()
        .map(|point| hz_to_cents(point.candidate.frequency.0))
        .collect::<Vec<_>>();
    let reference = endpoint_trend(&frequencies);
    let dtw_policy = DtwPolicy::new(GapPolicy::new(policy.max_jump_cents, policy.max_jump_cents))
        .with_window(AlignmentWindow::Radius(policy.dtw_radius))
        .with_memory(AlignmentMemory::Full);
    let control = graph_control(policy, *work_used);
    let alignment = dynamic_time_warp_with_control(
        &frequencies,
        &reference,
        |left, right| (left - right).abs(),
        dtw_policy,
        &control,
        &NeverInterrupt,
    )
    .map_err(graph_error)?;
    add_work(work_used, alignment.receipt.work_used, policy.max_work)?;
    let confidence = raw
        .points
        .iter()
        .map(|point| point.candidate.confidence)
        .sum::<f64>()
        / raw.points.len() as f64;
    let lower_frequency = Frequency(
        raw.points
            .iter()
            .map(|point| point.candidate.lower_frequency.0)
            .fold(f64::INFINITY, f64::min),
    );
    let upper_frequency = Frequency(
        raw.points
            .iter()
            .map(|point| point.candidate.upper_frequency.0)
            .fold(0.0_f64, f64::max),
    );
    Ok(PartialTrack {
        id: raw.id,
        points: raw.points,
        confidence,
        lower_frequency,
        upper_frequency,
        death: raw.death,
        continuity: PartialContinuityEvidence {
            score: alignment.score,
            steps: alignment.steps.as_ref().map_or(0, Vec::len),
            receipt: alignment.receipt,
        },
    })
}

fn graph_control(policy: &PartialTrackPolicy, work_used: u64) -> AlgorithmControl {
    AlgorithmControl::default()
        .with_max_work(policy.max_work.saturating_sub(work_used))
        .with_max_memory_cells(policy.max_memory_cells)
}

fn add_work(total: &mut u64, amount: u64, limit: u64) -> Result<(), AudioLiftError> {
    *total = total
        .checked_add(amount)
        .filter(|value| *value <= limit)
        .ok_or(AudioLiftError::PitchWorkLimit { limit })?;
    Ok(())
}

fn graph_error(error: sim_lib_discrete_graph::GraphError) -> AudioLiftError {
    AudioLiftError::TrackingGraph(error.to_string())
}

fn cents_distance(left_hz: f64, right_hz: f64) -> f64 {
    (1_200.0 * (right_hz / left_hz).log2()).abs()
}

fn hz_to_cents(frequency: f64) -> f64 {
    1_200.0 * (frequency / 440.0).log2()
}

fn millicents(cents: f64) -> i64 {
    (cents * 1_000.0).round() as i64
}

fn endpoint_trend(values: &[f64]) -> Vec<f64> {
    let Some((&first, rest)) = values.split_first() else {
        return Vec::new();
    };
    let last = rest.last().copied().unwrap_or(first);
    if values.len() == 1 {
        return vec![first];
    }
    (0..values.len())
        .map(|index| first + (last - first) * index as f64 / (values.len() - 1) as f64)
        .collect()
}
