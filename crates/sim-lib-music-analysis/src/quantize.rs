//! Exact metrical quantization through global sequence alignment.

use std::collections::BTreeMap;

use sim_lib_discrete_graph::{
    AlgorithmControl, Alignment, AlignmentBoundary, AlignmentMemory, AlignmentStep,
    AlignmentWindow, DtwPolicy, GapPolicy, dynamic_time_warp_with_control,
};
use sim_lib_music_core::{ObjectId, Staff, Time};

use crate::{AnalysisError, ratio_to_f64};

/// Declared metrical numerator and beat unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Meter {
    /// Beats in one bar.
    pub beats_per_bar: u16,
    /// Whole-note denominator of one beat; four means a quarter-note beat.
    pub beat_unit: u16,
}

/// Exact within-pair timing policy for the primary subdivision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwingPolicy {
    /// Divide every primary slot evenly.
    #[default]
    Straight,
    /// Place the second slot of each pair after `long / (long + short)`.
    Ratio {
        /// First portion of the pair.
        long: u16,
        /// Second portion of the pair.
        short: u16,
    },
}

/// Origin of one exact lattice point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LatticeKind {
    /// Point from the primary, optionally swung subdivision.
    Primary,
    /// Point from a declared straight tuplet division.
    Tuplet,
}

/// One identified point in a declared tempo/meter lattice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LatticePoint {
    /// Exact metrical time.
    pub at: Time,
    /// Zero-based containing bar.
    pub bar: usize,
    /// Zero-based beat within the bar.
    pub beat: u16,
    /// Number of slots dividing this beat.
    pub division: u16,
    /// Zero-based slot within the beat.
    pub slot: u16,
    /// Primary or tuplet origin.
    pub kind: LatticeKind,
}

/// Tempo, meter, swing, and tuplet declaration for quantization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetricalLattice {
    /// Exact quarter-note beats per minute, used to state movement cost in seconds.
    pub tempo_bpm: Time,
    /// Declared bar meter.
    pub meter: Meter,
    /// Primary slots per beat.
    pub subdivision: u16,
    /// Primary-subdivision swing policy.
    pub swing: SwingPolicy,
    /// Additional straight tuplet divisions per beat.
    pub tuplets: Vec<u16>,
}

/// Global alignment, tolerance, and report policy for metrical quantization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantizationPlan {
    /// Complete declared lattice.
    pub lattice: MetricalLattice,
    /// Maximum exact movement admitted for one onset.
    pub tolerance: Time,
    /// Maximum nearest lattice alternatives retained per onset.
    pub max_alternatives: usize,
    /// Maximum exact lattice points materialized before DTW admission.
    pub max_lattice_points: usize,
    /// Work and memory bounds delegated to generic DTW.
    pub control: AlgorithmControl,
}

/// One possible target for a source onset.
#[derive(Clone, Debug, PartialEq)]
pub struct QuantizationTarget {
    /// Exact target lattice point.
    pub point: LatticePoint,
    /// Absolute movement in whole-note units.
    pub distance: Time,
    /// Movement cost in seconds under the declared tempo.
    pub cost: f64,
}

/// Global selection and nearby alternatives for one shared source onset.
#[derive(Clone, Debug, PartialEq)]
pub struct QuantizationDecision {
    /// Stable event identities sharing this onset.
    pub event_ids: Vec<ObjectId>,
    /// Exact source onset.
    pub source: Time,
    /// Globally selected target, absent when tolerance preserved the source.
    pub selected: Option<QuantizationTarget>,
    /// Nearest independently ranked lattice choices.
    pub alternatives: Vec<QuantizationTarget>,
}

/// One explicit exact-time edit made by quantization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantizationEdit {
    /// Changed event identity.
    pub event_id: ObjectId,
    /// Original exact onset.
    pub before: Time,
    /// Quantized exact onset.
    pub after: Time,
}

/// Complete non-silent transformation evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantizationTransform {
    /// Every onset edit; an empty list proves the result was unchanged.
    pub edits: Vec<QuantizationEdit>,
}

/// Exact output plus alternatives, costs, and global-alignment proof.
#[derive(Clone, Debug, PartialEq)]
pub struct QuantizationReport {
    /// Complete lattice, tolerance, alternative, and resource policy.
    pub plan: QuantizationPlan,
    /// Quantized staff with all original identities and exact durations.
    pub output: Staff,
    /// One decision for every distinct source onset.
    pub decisions: Vec<QuantizationDecision>,
    /// Explicit list of every applied edit.
    pub transform: QuantizationTransform,
    /// Sum of selected movement costs in seconds.
    pub cost: f64,
    /// Generic DTW optimum, path, certificate, and resource receipt.
    pub alignment: Alignment<f64>,
}

/// Quantizes staff onsets by globally aligning distinct source times to a lattice.
///
/// Notes beyond [`QuantizationPlan::tolerance`] retain their exact source time.
/// Every accepted movement is listed in [`QuantizationTransform::edits`].
pub fn quantize_staff(
    source: &Staff,
    plan: &QuantizationPlan,
) -> Result<QuantizationReport, AnalysisError> {
    validate_plan(plan)?;
    let mut onset_events = BTreeMap::<Time, Vec<ObjectId>>::new();
    for note in source.notes() {
        onset_events
            .entry(note.onset)
            .or_default()
            .push(note.event_id.clone());
    }
    if onset_events.is_empty() {
        return Err(AnalysisError::InvalidInput {
            field: "staff",
            reason: "quantization requires at least one note event".to_owned(),
        });
    }
    let source_times = onset_events.keys().copied().collect::<Vec<_>>();
    let lattice = lattice_points(&plan.lattice, source.duration(), plan.max_lattice_points)?;
    let lattice_times = lattice.iter().map(|point| point.at).collect::<Vec<_>>();
    let max_pair_cost = pair_cost(
        *source_times.last().expect("non-empty source times"),
        lattice.first().expect("validated lattice").at,
        &plan.lattice,
    )?
    .max(pair_cost(
        source_times[0],
        lattice.last().expect("validated lattice").at,
        &plan.lattice,
    )?);
    let delete_cost = (max_pair_cost + 1.0) * (source_times.len() as f64 + 1.0);
    if !delete_cost.is_finite() {
        return Err(AnalysisError::InvalidInput {
            field: "staff",
            reason: "quantization alignment cost exceeded finite range".to_owned(),
        });
    }
    let policy = DtwPolicy::new(GapPolicy::new(delete_cost, 0.0))
        .with_boundary(AlignmentBoundary::Global)
        .with_window(AlignmentWindow::Unbounded)
        .with_memory(AlignmentMemory::Full);
    let alignment = dynamic_time_warp_with_control(
        &source_times,
        &lattice_times,
        |source, target| {
            pair_cost(*source, *target, &plan.lattice).expect("validated exact times remain finite")
        },
        policy,
        &plan.control,
        &sim_lib_discrete_graph::NeverInterrupt,
    )
    .map_err(|error| AnalysisError::Alignment(error.to_string()))?;

    let mut aligned = BTreeMap::<usize, usize>::new();
    for step in alignment.steps.as_deref().unwrap_or_default() {
        if let AlignmentStep::Match { left, right, .. } = step {
            aligned.insert(*left, *right);
        }
    }
    let mut selected_times = BTreeMap::<Time, Time>::new();
    let mut decisions = Vec::with_capacity(source_times.len());
    let mut cost = 0.0;
    for (index, source_time) in source_times.iter().copied().enumerate() {
        let mut alternatives = lattice
            .iter()
            .cloned()
            .map(|point| target(source_time, point, &plan.lattice))
            .collect::<Result<Vec<_>, _>>()?;
        alternatives.sort_by(|left, right| {
            left.cost
                .total_cmp(&right.cost)
                .then_with(|| left.point.at.cmp(&right.point.at))
        });
        alternatives.truncate(plan.max_alternatives);
        let selected = aligned
            .get(&index)
            .map(|target_index| target(source_time, lattice[*target_index].clone(), &plan.lattice))
            .transpose()?
            .filter(|target| target.distance <= plan.tolerance);
        if let Some(target) = &selected {
            selected_times.insert(source_time, target.point.at);
            cost += target.cost;
        }
        decisions.push(QuantizationDecision {
            event_ids: onset_events[&source_time].clone(),
            source: source_time,
            selected,
            alternatives,
        });
    }

    let mut voices = source.voices.clone();
    let mut edits = Vec::new();
    for voice in &mut voices {
        for note in &mut voice.notes {
            let after = selected_times
                .get(&note.onset)
                .copied()
                .unwrap_or(note.onset);
            if after != note.onset {
                edits.push(QuantizationEdit {
                    event_id: note.event_id.clone(),
                    before: note.onset,
                    after,
                });
                note.onset = after;
            }
        }
        if let Some(end) = voice.notes.iter().map(|note| note.end()).max() {
            voice.duration = voice.duration.max(end);
        }
    }
    edits.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    let output = Staff::new(voices).map_err(|error| AnalysisError::Staff(error.to_string()))?;
    Ok(QuantizationReport {
        plan: plan.clone(),
        output,
        decisions,
        transform: QuantizationTransform { edits },
        cost,
        alignment,
    })
}

fn validate_plan(plan: &QuantizationPlan) -> Result<(), AnalysisError> {
    let lattice = &plan.lattice;
    if lattice.tempo_bpm <= Time::from_integer(0) {
        return policy_error("tempo_bpm", "tempo must be positive");
    }
    if lattice.meter.beats_per_bar == 0 {
        return policy_error("meter", "beats per bar must be positive");
    }
    if lattice.meter.beat_unit == 0 || !lattice.meter.beat_unit.is_power_of_two() {
        return policy_error("meter", "beat unit must be a positive power of two");
    }
    if lattice.subdivision == 0 {
        return policy_error("subdivision", "primary subdivision must be positive");
    }
    if matches!(
        lattice.swing,
        SwingPolicy::Ratio { long: 0, .. } | SwingPolicy::Ratio { short: 0, .. }
    ) {
        return policy_error("swing", "swing ratio terms must be positive");
    }
    if lattice
        .tuplets
        .iter()
        .any(|division| *division < 2 || *division == lattice.subdivision)
    {
        return policy_error(
            "tuplets",
            "tuplet divisions must be at least two and distinct from the primary division",
        );
    }
    if plan.tolerance < Time::from_integer(0) {
        return policy_error("tolerance", "tolerance cannot be negative");
    }
    if plan.max_alternatives == 0 {
        return policy_error("max_alternatives", "at least one alternative is required");
    }
    if plan.max_lattice_points == 0 {
        return policy_error(
            "max_lattice_points",
            "lattice-point ceiling must be positive",
        );
    }
    Ok(())
}

fn policy_error<T>(field: &'static str, reason: &str) -> Result<T, AnalysisError> {
    Err(AnalysisError::InvalidPolicy {
        field,
        reason: reason.to_owned(),
    })
}

fn lattice_points(
    lattice: &MetricalLattice,
    end: Time,
    maximum: usize,
) -> Result<Vec<LatticePoint>, AnalysisError> {
    let beat_duration = Time::new(1, i64::from(lattice.meter.beat_unit));
    let beats_ratio = end / beat_duration;
    let beats = beats_ratio
        .numer()
        .div_euclid(*beats_ratio.denom())
        .saturating_add(2);
    let beats = usize::try_from(beats).map_err(|_| AnalysisError::InvalidInput {
        field: "staff duration",
        reason: "lattice beat count does not fit memory indexes".to_owned(),
    })?;
    let points_per_beat = usize::from(lattice.subdivision)
        .checked_add(
            lattice
                .tuplets
                .iter()
                .map(|division| usize::from(*division))
                .sum::<usize>(),
        )
        .ok_or(AnalysisError::ResourceLimit {
            resource: "quantization lattice points",
            required: u64::MAX,
            maximum: maximum as u64,
        })?;
    let required = beats
        .checked_mul(points_per_beat)
        .ok_or(AnalysisError::ResourceLimit {
            resource: "quantization lattice points",
            required: u64::MAX,
            maximum: maximum as u64,
        })?;
    if required > maximum {
        return Err(AnalysisError::ResourceLimit {
            resource: "quantization lattice points",
            required: u64::try_from(required).unwrap_or(u64::MAX),
            maximum: u64::try_from(maximum).unwrap_or(u64::MAX),
        });
    }
    let mut points = BTreeMap::<Time, LatticePoint>::new();
    for absolute_beat in 0..beats {
        add_division(
            &mut points,
            lattice,
            absolute_beat,
            lattice.subdivision,
            LatticeKind::Primary,
            true,
        );
        for division in &lattice.tuplets {
            add_division(
                &mut points,
                lattice,
                absolute_beat,
                *division,
                LatticeKind::Tuplet,
                false,
            );
        }
    }
    Ok(points.into_values().collect())
}

fn add_division(
    points: &mut BTreeMap<Time, LatticePoint>,
    lattice: &MetricalLattice,
    absolute_beat: usize,
    division: u16,
    kind: LatticeKind,
    apply_swing: bool,
) {
    let beat_duration = Time::new(1, i64::from(lattice.meter.beat_unit));
    let beat_start = beat_duration * Time::from_integer(absolute_beat as i64);
    for slot in 0..division {
        let fraction = if apply_swing {
            swung_fraction(slot, division, lattice.swing)
        } else {
            Time::new(i64::from(slot), i64::from(division))
        };
        let at = beat_start + beat_duration * fraction;
        points.entry(at).or_insert_with(|| LatticePoint {
            at,
            bar: absolute_beat / usize::from(lattice.meter.beats_per_bar),
            beat: (absolute_beat % usize::from(lattice.meter.beats_per_bar)) as u16,
            division,
            slot,
            kind,
        });
    }
}

fn swung_fraction(slot: u16, division: u16, swing: SwingPolicy) -> Time {
    let SwingPolicy::Ratio { long, short } = swing else {
        return Time::new(i64::from(slot), i64::from(division));
    };
    if division < 2 || !division.is_multiple_of(2) {
        return Time::new(i64::from(slot), i64::from(division));
    }
    let pair = slot / 2;
    let pair_start = Time::new(i64::from(pair * 2), i64::from(division));
    if slot.is_multiple_of(2) {
        pair_start
    } else {
        let pair_width = Time::new(2, i64::from(division));
        pair_start
            + pair_width
                * Time::new(
                    i64::from(long),
                    i64::from(long)
                        .checked_add(i64::from(short))
                        .expect("validated u16 swing terms fit i64"),
                )
    }
}

fn target(
    source: Time,
    point: LatticePoint,
    lattice: &MetricalLattice,
) -> Result<QuantizationTarget, AnalysisError> {
    let distance = if source >= point.at {
        source - point.at
    } else {
        point.at - source
    };
    Ok(QuantizationTarget {
        cost: pair_cost(source, point.at, lattice)?,
        point,
        distance,
    })
}

fn pair_cost(source: Time, target: Time, lattice: &MetricalLattice) -> Result<f64, AnalysisError> {
    let distance = if source >= target {
        source - target
    } else {
        target - source
    };
    let quarter_notes = distance * Time::from_integer(4);
    let minutes = quarter_notes / lattice.tempo_bpm;
    ratio_to_f64(minutes * Time::from_integer(60))
}
