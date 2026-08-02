use std::collections::{BTreeMap, BTreeSet};

use sim_lib_music_consonance::sounding_windows;
use sim_lib_music_core::{
    AmbiguousConversionPolicy, Counterpoint, ObjectId, ScoreForm, ScoreFormKind, Staff,
    convert_score,
};

use crate::{
    AlignmentWindow, AnalysisProvenance, CounterpointReport, DissonanceContext, MetricEvidence,
    Motion, MotionDirection, NoteEvidence, RuleSet, TimeSpan, Violation, VoiceEvidence,
};

/// Analyzes existing counterpoint under an inspectable rule set.
///
/// The source is converted through music-core's identity-bearing [`Staff`], then
/// segmented through the consonance owner's exact sounding-window algorithm.
/// No material is generated or rewritten.
pub fn analyze_counterpoint(cp: &Counterpoint, rules: &RuleSet) -> CounterpointReport {
    let validation = rules.validate();
    let staff = counterpoint_staff(cp);
    let notes = note_evidence(&staff);
    let alignment = exact_alignment(&staff, &notes);
    let motions = motions(&alignment);
    let mut violations = Vec::new();
    analyze_notes(&notes, rules, &mut violations);
    analyze_windows(&alignment, &notes, rules, &mut violations);
    analyze_motions(&motions, rules, &mut violations);
    violations.sort_by(|left, right| {
        left.span
            .cmp(&right.span)
            .then_with(|| left.rule.cmp(&right.rule))
            .then_with(|| event_key(&left.notes).cmp(&event_key(&right.notes)))
    });
    CounterpointReport {
        alignment,
        motions,
        violations,
        provenance: AnalysisProvenance {
            mode: "existing-counterpoint".to_owned(),
            rule_set: rules.id.clone(),
            facts: vec![
                format!("voices={}", staff.voices.len()),
                format!("notes={}", staff.notes().count()),
                "time=exact-rational-half-open".to_owned(),
                "identity=music-core-staff-conversion".to_owned(),
                "alignment=music-consonance-sounding-windows".to_owned(),
                format!(
                    "rule-data={}",
                    validation
                        .map(|()| "validated".to_owned())
                        .unwrap_or_else(|error| format!("invalid:{error}"))
                ),
            ],
        },
    }
}

fn counterpoint_staff(cp: &Counterpoint) -> Staff {
    let converted = convert_score(
        &ScoreForm::Counterpoint(cp.clone()),
        ScoreFormKind::Staff,
        AmbiguousConversionPolicy::Reject,
    )
    .expect("validated Counterpoint always converts to canonical Staff");
    match converted.value {
        ScoreForm::Staff(staff) => staff,
        _ => unreachable!("staff conversion target always returns Staff"),
    }
}

fn note_evidence(staff: &Staff) -> Vec<Vec<NoteEvidence>> {
    staff
        .voices
        .iter()
        .enumerate()
        .map(|(voice_index, voice)| {
            let voice_evidence = VoiceEvidence {
                index: voice_index,
                id: voice.id.clone(),
                name: voice.name.clone(),
            };
            voice
                .notes
                .iter()
                .enumerate()
                .map(|(index, note)| NoteEvidence {
                    voice: voice_evidence.clone(),
                    index,
                    note_id: note.note_id.clone(),
                    event_id: note.event_id.clone(),
                    span: TimeSpan::new(note.onset, note.end()),
                    pitch: note.note.pitch,
                })
                .collect()
        })
        .collect()
}

fn exact_alignment(staff: &Staff, notes: &[Vec<NoteEvidence>]) -> Vec<AlignmentWindow> {
    let by_event = notes
        .iter()
        .flatten()
        .map(|note| (note.event_id.clone(), note.clone()))
        .collect::<BTreeMap<_, _>>();
    sounding_windows(staff)
        .expect("validated Staff always yields exact sounding windows")
        .into_iter()
        .map(|window| {
            let mut sounding = window
                .notes
                .iter()
                .filter_map(|note| by_event.get(&note.event_id).cloned())
                .collect::<Vec<_>>();
            sounding.sort_by_key(|note| (note.voice.index, note.index));
            AlignmentWindow {
                span: TimeSpan::new(window.span.start, window.span.end),
                notes: sounding,
            }
        })
        .collect()
}

fn motions(windows: &[AlignmentWindow]) -> Vec<Motion> {
    let mut motions = Vec::new();
    let mut seen = BTreeSet::new();
    for pair in windows.windows(2) {
        let before = one_note_per_voice(&pair[0].notes);
        let after = one_note_per_voice(&pair[1].notes);
        let voices = before
            .keys()
            .filter(|voice| after.contains_key(voice))
            .copied()
            .collect::<Vec<_>>();
        for first_index in 0..voices.len() {
            for second_index in first_index + 1..voices.len() {
                let first_voice = voices[first_index];
                let second_voice = voices[second_index];
                let before_first = before[&first_voice];
                let before_second = before[&second_voice];
                let after_first = after[&first_voice];
                let after_second = after[&second_voice];
                if before_first.event_id == after_first.event_id
                    && before_second.event_id == after_second.event_id
                {
                    continue;
                }
                let key = (
                    before_first.event_id.clone(),
                    before_second.event_id.clone(),
                    after_first.event_id.clone(),
                    after_second.event_id.clone(),
                );
                if !seen.insert(key) {
                    continue;
                }
                motions.push(Motion {
                    voices: [before_first.voice.clone(), before_second.voice.clone()],
                    notes: [
                        before_first.clone(),
                        before_second.clone(),
                        after_first.clone(),
                        after_second.clone(),
                    ],
                    span: TimeSpan::new(pair[0].span.start, pair[1].span.end),
                    first: direction(before_first.pitch, after_first.pitch),
                    second: direction(before_second.pitch, after_second.pitch),
                    interval_before: absolute_interval(before_first.pitch, before_second.pitch),
                    interval_after: absolute_interval(after_first.pitch, after_second.pitch),
                });
            }
        }
    }
    motions
}

fn analyze_notes(notes: &[Vec<NoteEvidence>], rules: &RuleSet, out: &mut Vec<Violation>) {
    for (voice_index, voice) in notes.iter().enumerate() {
        let range = rules.range_for_voice(voice_index);
        for note in voice {
            if !range.contains(note.pitch) {
                out.push(violation(
                    "range",
                    "note lies outside the declared voice range",
                    vec![note.clone()],
                    note.span.clone(),
                    MetricEvidence {
                        metric: "absolute-pitch".to_owned(),
                        observed: note.pitch.semitone().to_string(),
                        expected: format!("{}..={}", range.low.semitone(), range.high.semitone()),
                        unit: "semitones".to_owned(),
                        facts: vec![format!("pitch={}", pitch_name(note.pitch))],
                    },
                ));
            }
            if !rules.durations.allowed_pulse_ratios.is_empty() {
                let ratio = note.span.duration() / rules.durations.pulse;
                if !rules.durations.allowed_pulse_ratios.contains(&ratio) {
                    out.push(violation(
                        "duration",
                        "note duration is not admitted by the rule set",
                        vec![note.clone()],
                        note.span.clone(),
                        MetricEvidence {
                            metric: "duration/pulse".to_owned(),
                            observed: rational(ratio),
                            expected: rules
                                .durations
                                .allowed_pulse_ratios
                                .iter()
                                .copied()
                                .map(rational)
                                .collect::<Vec<_>>()
                                .join(","),
                            unit: "exact-ratio".to_owned(),
                            facts: vec![format!("pulse={}", rational(rules.durations.pulse))],
                        },
                    ));
                }
            }
        }
        for pair in voice.windows(2) {
            let distance = absolute_interval(pair[0].pitch, pair[1].pitch) as u8;
            if distance > rules.intervals.max_melodic_semitones
                || rules
                    .intervals
                    .forbidden_melodic_semitones
                    .contains(&distance)
            {
                out.push(violation(
                    "melodic-interval",
                    "adjacent notes violate the declared melodic interval policy",
                    pair.to_vec(),
                    TimeSpan::new(pair[0].span.start, pair[1].span.end),
                    MetricEvidence {
                        metric: "absolute-melodic-interval".to_owned(),
                        observed: distance.to_string(),
                        expected: format!(
                            "<={}; forbidden={:?}",
                            rules.intervals.max_melodic_semitones,
                            rules.intervals.forbidden_melodic_semitones
                        ),
                        unit: "semitones".to_owned(),
                        facts: vec![format!(
                            "motion={} -> {}",
                            pitch_name(pair[0].pitch),
                            pitch_name(pair[1].pitch)
                        )],
                    },
                ));
            }
        }
    }
}

fn analyze_windows(
    windows: &[AlignmentWindow],
    notes: &[Vec<NoteEvidence>],
    rules: &RuleSet,
    out: &mut Vec<Violation>,
) {
    for (window_index, window) in windows.iter().enumerate() {
        let sounding = one_note_per_voice(&window.notes);
        let voices = sounding.keys().copied().collect::<Vec<_>>();
        for first in 0..voices.len() {
            for second in first + 1..voices.len() {
                let upper = sounding[&voices[first]];
                let lower = sounding[&voices[second]];
                let crossed = if rules.voices.highest_voice_first {
                    upper.pitch < lower.pitch
                } else {
                    upper.pitch > lower.pitch
                };
                if crossed && !rules.voices.allow_crossing {
                    out.push(violation(
                        "voice-crossing",
                        "simultaneous voices reverse their declared vertical order",
                        vec![upper.clone(), lower.clone()],
                        window.span.clone(),
                        MetricEvidence {
                            metric: "ordered-pitch".to_owned(),
                            observed: format!(
                                "{}:{}",
                                upper.pitch.semitone(),
                                lower.pitch.semitone()
                            ),
                            expected: if rules.voices.highest_voice_first {
                                "first>=second"
                            } else {
                                "first<=second"
                            }
                            .to_owned(),
                            unit: "absolute-pitch".to_owned(),
                            facts: Vec::new(),
                        },
                    ));
                }
                let class = interval_class(upper.pitch, lower.pitch);
                if !rules.intervals.consonant_harmonic_classes.contains(&class) {
                    let contexts =
                        dissonance_contexts(window_index, windows, notes, upper, lower, rules);
                    if !contexts
                        .iter()
                        .any(|context| rules.dissonance.allowed_contexts.contains(context))
                    {
                        out.push(violation(
                            "dissonance-preparation-resolution",
                            "dissonance has no admitted preparation and resolution",
                            vec![upper.clone(), lower.clone()],
                            window.span.clone(),
                            MetricEvidence {
                                metric: "harmonic-interval-class".to_owned(),
                                observed: class.to_string(),
                                expected: format!(
                                    "consonant={:?}; contexts={:?}",
                                    rules.intervals.consonant_harmonic_classes,
                                    rules.dissonance.allowed_contexts
                                ),
                                unit: "interval-class".to_owned(),
                                facts: vec![
                                    format!("recognized-contexts={contexts:?}"),
                                    format!(
                                        "attack={}",
                                        if upper.span.start == window.span.start
                                            || lower.span.start == window.span.start
                                        {
                                            "yes"
                                        } else {
                                            "held"
                                        }
                                    ),
                                ],
                            },
                        ));
                    }
                }
            }
        }
    }
}

fn analyze_motions(motions: &[Motion], rules: &RuleSet, out: &mut Vec<Violation>) {
    for motion in motions {
        let similar = matches!(
            (motion.first, motion.second),
            (MotionDirection::Up, MotionDirection::Up)
                | (MotionDirection::Down, MotionDirection::Down)
        );
        let before_class = folded_class(motion.interval_before);
        let after_class = folded_class(motion.interval_after);
        let perfect_before = rules
            .intervals
            .perfect_harmonic_classes
            .contains(&before_class);
        let perfect_after = rules
            .intervals
            .perfect_harmonic_classes
            .contains(&after_class);
        if rules.motion.forbid_parallel_perfects
            && similar
            && perfect_before
            && perfect_after
            && before_class == after_class
        {
            out.push(motion_violation(
                motion,
                "parallel-perfect",
                "similar motion repeats a perfect interval class",
                before_class,
                after_class,
            ));
        } else if rules.motion.forbid_direct_perfects && similar && perfect_after {
            let first_move = absolute_interval(motion.notes[0].pitch, motion.notes[2].pitch) as u8;
            let second_move = absolute_interval(motion.notes[1].pitch, motion.notes[3].pitch) as u8;
            if first_move > rules.motion.leap_threshold || second_move > rules.motion.leap_threshold
            {
                out.push(motion_violation(
                    motion,
                    "direct-perfect",
                    "similar motion enters a perfect interval with a leap",
                    before_class,
                    after_class,
                ));
            }
        }
        if !rules.voices.allow_overlap {
            let first_was_higher = motion.notes[0].pitch >= motion.notes[1].pitch;
            let overlap = if first_was_higher {
                motion.notes[2].pitch < motion.notes[1].pitch
                    || motion.notes[3].pitch > motion.notes[0].pitch
            } else {
                motion.notes[2].pitch > motion.notes[1].pitch
                    || motion.notes[3].pitch < motion.notes[0].pitch
            };
            if overlap {
                out.push(Violation {
                    rule: "voice-overlap".to_owned(),
                    message: "a moving voice passes the other voice's previous pitch".to_owned(),
                    voices: motion.voices.to_vec(),
                    notes: motion.notes.to_vec(),
                    span: motion.span.clone(),
                    metric: MetricEvidence {
                        metric: "successive-voice-order".to_owned(),
                        observed: format!(
                            "{}:{} -> {}:{}",
                            motion.notes[0].pitch.semitone(),
                            motion.notes[1].pitch.semitone(),
                            motion.notes[2].pitch.semitone(),
                            motion.notes[3].pitch.semitone()
                        ),
                        expected: "each voice remains on its prior side".to_owned(),
                        unit: "absolute-pitch".to_owned(),
                        facts: Vec::new(),
                    },
                });
            }
        }
    }
}

fn dissonance_contexts(
    window_index: usize,
    windows: &[AlignmentWindow],
    voices: &[Vec<NoteEvidence>],
    first: &NoteEvidence,
    second: &NoteEvidence,
    rules: &RuleSet,
) -> Vec<DissonanceContext> {
    let mut contexts = Vec::new();
    let weak_attack = {
        let units = windows[window_index].span.start / rules.durations.pulse;
        *units.denom() != 1
    };
    for note in [first, second] {
        let Some(line) = voices.get(note.voice.index) else {
            continue;
        };
        if note.index > 0 && note.index + 1 < line.len() {
            let previous = &line[note.index - 1];
            let next = &line[note.index + 1];
            let approach = next_signed(previous.pitch, note.pitch);
            let departure = next_signed(note.pitch, next.pitch);
            let step = i32::from(rules.dissonance.max_step_semitones);
            if approach != 0
                && approach.signum() == departure.signum()
                && approach.abs() <= step
                && departure.abs() <= step
                && (!rules.dissonance.require_weak_attack || weak_attack)
            {
                contexts.push(DissonanceContext::Passing);
            }
            if previous.pitch == next.pitch
                && approach.abs() <= step
                && departure.abs() <= step
                && (!rules.dissonance.require_weak_attack || weak_attack)
            {
                contexts.push(DissonanceContext::Neighbor);
            }
        }
        if note.span.start < windows[window_index].span.start
            && note.index + 1 < line.len()
            && next_signed(note.pitch, line[note.index + 1].pitch) < 0
            && next_signed(note.pitch, line[note.index + 1].pitch).abs()
                <= i32::from(rules.dissonance.max_step_semitones)
            && previous_window_is_consonant(window_index, windows, first, second, rules)
        {
            contexts.push(DissonanceContext::Suspension);
        }
    }
    contexts.sort_by_key(|context| match context {
        DissonanceContext::Passing => 0,
        DissonanceContext::Neighbor => 1,
        DissonanceContext::Suspension => 2,
    });
    contexts.dedup();
    contexts
}

fn previous_window_is_consonant(
    index: usize,
    windows: &[AlignmentWindow],
    first: &NoteEvidence,
    second: &NoteEvidence,
    rules: &RuleSet,
) -> bool {
    let Some(previous) = index.checked_sub(1).and_then(|at| windows.get(at)) else {
        return false;
    };
    let sounding = one_note_per_voice(&previous.notes);
    let Some(first) = sounding.get(&first.voice.index) else {
        return false;
    };
    let Some(second) = sounding.get(&second.voice.index) else {
        return false;
    };
    rules
        .intervals
        .consonant_harmonic_classes
        .contains(&interval_class(first.pitch, second.pitch))
}

fn one_note_per_voice(notes: &[NoteEvidence]) -> BTreeMap<usize, &NoteEvidence> {
    notes.iter().map(|note| (note.voice.index, note)).collect()
}

fn motion_violation(
    motion: &Motion,
    rule: &str,
    message: &str,
    before: u8,
    after: u8,
) -> Violation {
    Violation {
        rule: rule.to_owned(),
        message: message.to_owned(),
        voices: motion.voices.to_vec(),
        notes: motion.notes.to_vec(),
        span: motion.span.clone(),
        metric: MetricEvidence {
            metric: "successive-harmonic-interval-class".to_owned(),
            observed: format!("{before}->{after}"),
            expected: "no similar-motion perfect arrival".to_owned(),
            unit: "interval-class".to_owned(),
            facts: vec![
                format!("first-motion={:?}", motion.first),
                format!("second-motion={:?}", motion.second),
            ],
        },
    }
}

fn violation(
    rule: &str,
    message: &str,
    notes: Vec<NoteEvidence>,
    span: TimeSpan,
    metric: MetricEvidence,
) -> Violation {
    let voices = notes
        .iter()
        .map(|note| note.voice.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Violation {
        rule: rule.to_owned(),
        message: message.to_owned(),
        voices,
        notes,
        span,
        metric,
    }
}

fn event_key(notes: &[NoteEvidence]) -> Vec<ObjectId> {
    notes.iter().map(|note| note.event_id.clone()).collect()
}

fn direction(
    before: sim_lib_music_core::Pitch,
    after: sim_lib_music_core::Pitch,
) -> MotionDirection {
    match after.semitone().cmp(&before.semitone()) {
        std::cmp::Ordering::Less => MotionDirection::Down,
        std::cmp::Ordering::Equal => MotionDirection::Static,
        std::cmp::Ordering::Greater => MotionDirection::Up,
    }
}

fn next_signed(before: sim_lib_music_core::Pitch, after: sim_lib_music_core::Pitch) -> i32 {
    after.semitone() - before.semitone()
}

fn absolute_interval(first: sim_lib_music_core::Pitch, second: sim_lib_music_core::Pitch) -> i32 {
    next_signed(first, second).abs()
}

fn interval_class(first: sim_lib_music_core::Pitch, second: sim_lib_music_core::Pitch) -> u8 {
    folded_class(absolute_interval(first, second))
}

fn folded_class(interval: i32) -> u8 {
    let class = interval.rem_euclid(12) as u8;
    class.min(12 - class)
}

fn rational(value: sim_lib_music_core::Time) -> String {
    format!("{}/{}", value.numer(), value.denom())
}

fn pitch_name(pitch: sim_lib_music_core::Pitch) -> String {
    format!("{}{}", pitch.class.canonical_name(), pitch.octave)
}
