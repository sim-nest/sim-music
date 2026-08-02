//! Finite-domain counterpoint compilation and bounded generation.

use std::cmp::Ordering;

use sim_lib_discrete_search::{SearchControl, SearchInterrupt, SearchProblem, SearchStep, solve};
use sim_lib_music_core::{Counterpoint, Melody, MelodyItem, Pitch, Time};

use crate::generator_materialization::{
    cantus_staff, counterpoint_from_assignment, materialize_result, retain_diverse,
};

use crate::{
    CadencePolicy, CounterpointCsp, CounterpointDomain, CounterpointGeneration,
    CounterpointGenerationPolicy, CounterpointGenerationReceipt, CounterpointVariable,
    GenerationError, RuleSet, Species, analyze_counterpoint,
};

const MAX_GENERATED_VOICES: usize = 8;
const MAX_CSP_VARIABLES: usize = 4_096;

/// Compiles musical rule data to finite variables and pitch domains.
///
/// Variables are ordered by exact time and then voice. Their durations are
/// fixed by the selected species rhythm; only pitch is searched. Range,
/// sonance, vertical order, and cadence restrictions are reflected directly in
/// each finite domain. Prefix relationships such as melodic motion, overlap,
/// and parallel-perfect avoidance are propagated by the generic search
/// problem.
pub fn compile_counterpoint_csp(
    cantus: &Melody,
    rules: &RuleSet,
    policy: &CounterpointGenerationPolicy,
) -> Result<CounterpointCsp, GenerationError> {
    rules.validate()?;
    validate_policy(policy)?;
    let duration = cantus.total_duration();
    if duration <= Time::from_integer(0) {
        return invalid("cantus must have positive duration");
    }
    let rhythm = generation_rhythm(rules);
    if rhythm <= Time::from_integer(0) {
        return invalid("generated rhythm must have positive duration");
    }
    let slot_ratio = duration / rhythm;
    if *slot_ratio.denom() != 1 || *slot_ratio.numer() <= 0 {
        return invalid(format!(
            "cantus duration {}/{} is not an exact multiple of generated rhythm {}/{}",
            duration.numer(),
            duration.denom(),
            rhythm.numer(),
            rhythm.denom()
        ));
    }
    let slots = usize::try_from(*slot_ratio.numer())
        .map_err(|_| GenerationError::InvalidPolicy("slot count does not fit usize".to_owned()))?;
    let variable_count = slots.checked_mul(policy.voices).ok_or_else(|| {
        GenerationError::InvalidPolicy("CSP variable count overflowed".to_owned())
    })?;
    if variable_count > MAX_CSP_VARIABLES {
        return invalid(format!(
            "CSP variable count {variable_count} exceeds {MAX_CSP_VARIABLES}"
        ));
    }

    let mut variables = Vec::with_capacity(variable_count);
    let mut domains = Vec::with_capacity(variable_count);
    for slot in 0..slots {
        let onset = rhythm
            * Time::from_integer(i64::try_from(slot).map_err(|_| {
                GenerationError::InvalidPolicy("slot index does not fit exact time".to_owned())
            })?);
        let cantus_pitch = sounding_pitch(cantus, onset);
        for voice in 0..policy.voices {
            let variable = CounterpointVariable {
                index: variables.len(),
                voice,
                slot,
                onset,
                duration: rhythm,
            };
            let range = rules.range_for_voice(voice + 1);
            let mut pitches = (range.low.semitone().max(0)..=range.high.semitone().min(127))
                .filter_map(|semitone| u8::try_from(semitone).ok())
                .filter(|semitone| {
                    let pitch = Pitch::from_midi(*semitone);
                    cantus_pitch.is_none_or(|fixed| {
                        ordered(fixed, pitch, rules)
                            && consonant(fixed, pitch, rules)
                            && cadence_accepts(policy.cadence, slot, slots, fixed, pitch, rules)
                    })
                })
                .collect::<Vec<_>>();
            pitches.sort_unstable();
            pitches.dedup();
            if pitches.is_empty() {
                return invalid(format!(
                    "voice {voice} slot {slot} has an empty pitch domain"
                ));
            }
            variables.push(variable.clone());
            domains.push(CounterpointDomain { variable, pitches });
        }
    }

    if !matches!(policy.cadence, CadencePolicy::Open) {
        let final_onset = rhythm
            * Time::from_integer(i64::try_from(slots - 1).map_err(|_| {
                GenerationError::InvalidPolicy("final slot does not fit exact time".to_owned())
            })?);
        if sounding_pitch(cantus, Time::from_integer(0)).is_none()
            || sounding_pitch(cantus, final_onset).is_none()
        {
            return invalid("a perfect cadence requires sounding cantus endpoints");
        }
    }

    Ok(CounterpointCsp {
        variables,
        domains,
        rhythm,
        rule_set: rules.id.clone(),
        facts: vec![
            "engine=sim-lib-discrete-search/SearchProblem".to_owned(),
            "variables=generated-voice-slot-pitch".to_owned(),
            "domains=finite-range-sonance-cadence-filtered".to_owned(),
            "propagation=melody-vertical-order-motion".to_owned(),
            "scoring=melodic-motion-plus-perfect-interval-cost".to_owned(),
        ],
    })
}

/// Generates legal contrapuntal additions under explicit search controls.
///
/// The fixed cantus is borrowed and never modified. Every returned result owns
/// a content-bound additive patch; removing that patch from its completed staff
/// is verified to reproduce the source staff exactly. Empty, partial, and
/// cancelled runs are returned with their original search receipt.
pub fn generate_counterpoint(
    cantus: &Melody,
    rules: &RuleSet,
    policy: &CounterpointGenerationPolicy,
    control: SearchControl,
    interrupt: &dyn SearchInterrupt,
) -> Result<CounterpointGeneration, GenerationError> {
    validate_control(&control)?;
    let csp = compile_counterpoint_csp(cantus, rules, policy)?;
    let problem = CounterpointProblem {
        cantus,
        rules,
        policy,
        csp: &csp,
        seed: control.seed,
    };
    let run = solve(&problem, control, interrupt);
    let raw_result_count = run.outputs.len();
    let mut assignments = run.outputs;
    assignments.sort_by(compare_assignments);
    let (assignments, diversity_rejected) = retain_diverse(assignments, &policy.diversity);
    let source = cantus_staff(cantus)?;
    let mut results = Vec::with_capacity(assignments.len());
    for assignment in assignments {
        results.push(materialize_result(
            cantus,
            &source,
            rules,
            policy,
            &csp,
            run.receipt.seed,
            assignment,
        )?);
    }
    let facts = vec![
        "fixed-material=borrowed-and-content-bound".to_owned(),
        "termination-status=unmodified-generic-search-receipt".to_owned(),
        format!("raw-results={raw_result_count}"),
        format!("selected-results={}", results.len()),
        format!("diversity-rejected={diversity_rejected}"),
        format!("cadence={}", cadence_label(policy.cadence)),
        format!(
            "diversity=minimum-pitch-changes:{}",
            policy.diversity.minimum_pitch_changes
        ),
    ];
    Ok(CounterpointGeneration {
        csp,
        results,
        receipt: CounterpointGenerationReceipt {
            search: run.receipt,
            raw_result_count,
            selected_result_count: raw_result_count.saturating_sub(diversity_rejected),
            diversity_rejected,
            facts,
        },
    })
}

fn validate_policy(policy: &CounterpointGenerationPolicy) -> Result<(), GenerationError> {
    if policy.voices == 0 || policy.voices > MAX_GENERATED_VOICES {
        return invalid(format!(
            "generated voice count must be in 1..={MAX_GENERATED_VOICES}"
        ));
    }
    if policy.velocity > 127 {
        return invalid("generated velocity must be in 0..=127");
    }
    Ok(())
}

fn validate_control(control: &SearchControl) -> Result<(), GenerationError> {
    if control.max_work.is_none() || control.max_frontier.is_none() || control.max_results.is_none()
    {
        return invalid("counterpoint generation requires work, frontier, and result bounds");
    }
    Ok(())
}

fn generation_rhythm(rules: &RuleSet) -> Time {
    match rules.species {
        Species::First => rules.durations.pulse,
        Species::Second | Species::Fourth => rules.durations.pulse * Time::new(1, 2),
        Species::Third => rules.durations.pulse * Time::new(1, 4),
        Species::Open => rules
            .durations
            .allowed_pulse_ratios
            .iter()
            .copied()
            .min()
            .map_or(rules.durations.pulse, |ratio| rules.durations.pulse * ratio),
    }
}

fn cadence_accepts(
    cadence: CadencePolicy,
    slot: usize,
    slots: usize,
    fixed: Pitch,
    generated: Pitch,
    rules: &RuleSet,
) -> bool {
    let endpoint = match cadence {
        CadencePolicy::Open => false,
        CadencePolicy::PerfectFinal => slot + 1 == slots,
        CadencePolicy::PerfectEndpoints => slot == 0 || slot + 1 == slots,
    };
    !endpoint
        || rules
            .intervals
            .perfect_harmonic_classes
            .contains(&interval_class(fixed, generated))
}

fn sounding_pitch(melody: &Melody, at: Time) -> Option<Pitch> {
    let mut onset = Time::from_integer(0);
    for item in &melody.items {
        let end = onset + item.duration();
        if onset <= at && at < end {
            return match item {
                MelodyItem::Note(note) => Some(note.pitch),
                MelodyItem::Rest(_) => None,
            };
        }
        onset = end;
    }
    None
}

struct CounterpointProblem<'a> {
    cantus: &'a Melody,
    rules: &'a RuleSet,
    policy: &'a CounterpointGenerationPolicy,
    csp: &'a CounterpointCsp,
    seed: u64,
}

#[derive(Clone, Debug)]
struct CounterpointState {
    pitches: Vec<u8>,
    score: i64,
    legal: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PitchChoice {
    rank: u64,
    pitch: u8,
}

#[derive(Clone, Debug)]
pub(crate) struct CounterpointAssignment {
    pub(crate) pitches: Vec<u8>,
    pub(crate) score: i64,
}

impl SearchProblem for CounterpointProblem<'_> {
    type State = CounterpointState;
    type Choice = PitchChoice;
    type Output = CounterpointAssignment;

    fn initial_state(&self) -> Self::State {
        CounterpointState {
            pitches: Vec::new(),
            score: 0,
            legal: false,
        }
    }

    fn expand(&self, state: &Self::State, out: &mut Vec<Self::Choice>) {
        let Some(domain) = self.csp.domains.get(state.pitches.len()) else {
            return;
        };
        out.extend(domain.pitches.iter().copied().map(|pitch| PitchChoice {
            rank: seeded_rank(self.seed, domain.variable.index, pitch),
            pitch,
        }));
    }

    fn apply(&self, state: &Self::State, choice: &Self::Choice) -> SearchStep<Self::State> {
        let Some(domain) = self.csp.domains.get(state.pitches.len()) else {
            return SearchStep::infeasible("assignment exceeds compiled variables");
        };
        if !domain.pitches.contains(&choice.pitch) {
            return SearchStep::infeasible("pitch lies outside compiled domain");
        }
        let mut pitches = state.pitches.clone();
        pitches.push(choice.pitch);
        let score = state.score.saturating_add(self.incremental_score(&pitches));
        SearchStep::Continue(CounterpointState {
            pitches,
            score,
            legal: false,
        })
    }

    fn propagate(&self, mut state: Self::State) -> SearchStep<Self::State> {
        if state.pitches.is_empty() {
            return SearchStep::Continue(state);
        }
        let index = state.pitches.len() - 1;
        if !self.accepts_prefix(&state.pitches, index) {
            return SearchStep::pruned("counterpoint CSP propagation rejected assignment");
        }
        if state.pitches.len() == self.csp.variables.len() {
            let counterpoint = self.counterpoint(&state.pitches);
            state.legal = counterpoint
                .as_ref()
                .is_ok_and(|value| analyze_counterpoint(value, self.rules).is_legal());
            if !state.legal {
                return SearchStep::pruned("complete assignment failed counterpoint analysis");
            }
        }
        SearchStep::Continue(state)
    }

    fn finish(&self, state: &Self::State) -> Option<Self::Output> {
        state.legal.then(|| CounterpointAssignment {
            pitches: state.pitches.clone(),
            score: state.score,
        })
    }

    fn score_state(&self, state: &Self::State) -> i64 {
        state.score
    }

    fn bound(&self, state: &Self::State) -> Option<i64> {
        Some(state.score)
    }

    fn output_score(&self, output: &Self::Output) -> Option<i64> {
        Some(output.score)
    }
}

impl CounterpointProblem<'_> {
    fn accepts_prefix(&self, pitches: &[u8], index: usize) -> bool {
        let variable = &self.csp.variables[index];
        let pitch = Pitch::from_midi(pitches[index]);
        if variable.slot > 0 {
            let previous_index = (variable.slot - 1) * self.policy.voices + variable.voice;
            let previous = Pitch::from_midi(pitches[previous_index]);
            let distance = absolute_interval(previous, pitch);
            if distance > i32::from(self.rules.intervals.max_melodic_semitones)
                || self
                    .rules
                    .intervals
                    .forbidden_melodic_semitones
                    .contains(&u8::try_from(distance).unwrap_or(u8::MAX))
            {
                return false;
            }
        }

        let mut prior_lines = Vec::new();
        if let Some(fixed) = sounding_pitch(self.cantus, variable.onset) {
            prior_lines.push((0usize, fixed));
        }
        for voice in 0..variable.voice {
            let other_index = variable.slot * self.policy.voices + voice;
            prior_lines.push((voice + 1, Pitch::from_midi(pitches[other_index])));
        }
        for (line, other) in prior_lines {
            if !ordered(other, pitch, self.rules) || !consonant(other, pitch, self.rules) {
                return false;
            }
            if variable.slot > 0 && !self.accepts_motion(pitches, variable, line, other, pitch) {
                return false;
            }
        }
        true
    }

    fn accepts_motion(
        &self,
        pitches: &[u8],
        variable: &CounterpointVariable,
        other_line: usize,
        other_now: Pitch,
        pitch_now: Pitch,
    ) -> bool {
        let previous_onset = variable.onset - variable.duration;
        let other_before = if other_line == 0 {
            sounding_pitch(self.cantus, previous_onset)
        } else {
            let index = (variable.slot - 1) * self.policy.voices + (other_line - 1);
            pitches.get(index).copied().map(Pitch::from_midi)
        };
        let Some(other_before) = other_before else {
            return true;
        };
        let this_before_index = (variable.slot - 1) * self.policy.voices + variable.voice;
        let this_before = Pitch::from_midi(pitches[this_before_index]);
        let other_motion = other_now.semitone() - other_before.semitone();
        let this_motion = pitch_now.semitone() - this_before.semitone();
        let similar =
            other_motion != 0 && this_motion != 0 && other_motion.signum() == this_motion.signum();
        let before_class = interval_class(other_before, this_before);
        let now_class = interval_class(other_now, pitch_now);
        let perfect_before = self
            .rules
            .intervals
            .perfect_harmonic_classes
            .contains(&before_class);
        let perfect_now = self
            .rules
            .intervals
            .perfect_harmonic_classes
            .contains(&now_class);
        if self.rules.motion.forbid_parallel_perfects
            && similar
            && perfect_before
            && perfect_now
            && before_class == now_class
        {
            return false;
        }
        if self.rules.motion.forbid_direct_perfects
            && similar
            && perfect_now
            && (other_motion.unsigned_abs() > u32::from(self.rules.motion.leap_threshold)
                || this_motion.unsigned_abs() > u32::from(self.rules.motion.leap_threshold))
        {
            return false;
        }
        if !self.rules.voices.allow_overlap {
            let other_was_higher = other_before >= this_before;
            if (other_was_higher && (other_now < this_before || pitch_now > other_before))
                || (!other_was_higher && (other_now > this_before || pitch_now < other_before))
            {
                return false;
            }
        }
        true
    }

    fn incremental_score(&self, pitches: &[u8]) -> i64 {
        let index = pitches.len() - 1;
        let variable = &self.csp.variables[index];
        let pitch = Pitch::from_midi(pitches[index]);
        let melodic = if variable.slot == 0 {
            0
        } else {
            let previous = pitches[(variable.slot - 1) * self.policy.voices + variable.voice];
            i64::from(pitches[index].abs_diff(previous))
        };
        let perfect_cost = sounding_pitch(self.cantus, variable.onset).map_or(0, |fixed| {
            i64::from(
                self.rules
                    .intervals
                    .perfect_harmonic_classes
                    .contains(&interval_class(fixed, pitch)),
            ) * 2
        });
        melodic.saturating_add(perfect_cost)
    }

    fn counterpoint(&self, pitches: &[u8]) -> Result<Counterpoint, GenerationError> {
        counterpoint_from_assignment(self.cantus, self.policy, self.csp, pitches)
    }
}

fn compare_assignments(left: &CounterpointAssignment, right: &CounterpointAssignment) -> Ordering {
    left.score
        .cmp(&right.score)
        .then_with(|| left.pitches.cmp(&right.pitches))
}

fn seeded_rank(seed: u64, variable: usize, pitch: u8) -> u64 {
    let mut value = seed
        ^ u64::try_from(variable)
            .unwrap_or(u64::MAX)
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ u64::from(pitch).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn cadence_label(value: CadencePolicy) -> &'static str {
    match value {
        CadencePolicy::Open => "open",
        CadencePolicy::PerfectFinal => "perfect-final",
        CadencePolicy::PerfectEndpoints => "perfect-endpoints",
    }
}

fn ordered(upper: Pitch, lower: Pitch, rules: &RuleSet) -> bool {
    rules.voices.allow_crossing
        || if rules.voices.highest_voice_first {
            upper.semitone() >= lower.semitone()
        } else {
            upper.semitone() <= lower.semitone()
        }
}

fn consonant(first: Pitch, second: Pitch, rules: &RuleSet) -> bool {
    rules
        .intervals
        .consonant_harmonic_classes
        .contains(&interval_class(first, second))
}

fn interval_class(first: Pitch, second: Pitch) -> u8 {
    let class = absolute_interval(first, second).rem_euclid(12) as u8;
    class.min(12 - class)
}

fn absolute_interval(first: Pitch, second: Pitch) -> i32 {
    (first.semitone() - second.semitone()).abs()
}

fn invalid<T>(reason: impl Into<String>) -> Result<T, GenerationError> {
    Err(GenerationError::InvalidPolicy(reason.into()))
}
