//! Reversible materialization for generated counterpoint assignments.

use sim_lib_music_consonance::{
    Addition, ConsonancePatch, VoiceAddition, apply_patch, remove_patch,
};
use sim_lib_music_core::{
    AmbiguousConversionPolicy, Articulation, Channel, Counterpoint, Melody, MelodyItem, Note,
    ObjectId, Pitch, ScoreForm, ScoreFormKind, Staff, StaffNote, StaffVoice, Time, convert_score,
};

use crate::generator::CounterpointAssignment;
use crate::{
    CounterpointCsp, CounterpointGenerationPolicy, CounterpointGenerationResult, DiversityPolicy,
    GenerationError, RuleSet, analyze_counterpoint,
};

pub(crate) fn retain_diverse(
    assignments: Vec<CounterpointAssignment>,
    policy: &DiversityPolicy,
) -> (Vec<CounterpointAssignment>, usize) {
    let mut retained: Vec<CounterpointAssignment> = Vec::new();
    let mut rejected = 0;
    for assignment in assignments {
        let diverse = retained.iter().all(|prior| {
            assignment
                .pitches
                .iter()
                .zip(&prior.pitches)
                .filter(|(left, right)| left != right)
                .count()
                >= policy.minimum_pitch_changes
        });
        if diverse {
            retained.push(assignment);
        } else {
            rejected += 1;
        }
    }
    (retained, rejected)
}

pub(crate) fn materialize_result(
    cantus: &Melody,
    source: &Staff,
    rules: &RuleSet,
    policy: &CounterpointGenerationPolicy,
    csp: &CounterpointCsp,
    seed: u64,
    assignment: CounterpointAssignment,
) -> Result<CounterpointGenerationResult, GenerationError> {
    let counterpoint = counterpoint_from_assignment(cantus, policy, csp, &assignment.pitches)?;
    let report = analyze_counterpoint(&counterpoint, rules);
    if !report.is_legal() {
        return Err(GenerationError::Invariant(
            "search emitted a counterpoint rejected by its source rules".to_owned(),
        ));
    }
    if counterpoint.voices.first() != Some(cantus) {
        return Err(GenerationError::Invariant(
            "materialization changed the fixed cantus".to_owned(),
        ));
    }
    let fingerprint = assignment_fingerprint(&assignment.pitches);
    let generated_voices = staff_voices(
        policy,
        csp,
        &assignment.pitches,
        source.duration(),
        seed,
        &fingerprint,
    )?;
    let patch = ConsonancePatch::new(
        source,
        generated_voices
            .into_iter()
            .map(|voice| Addition::Voice(VoiceAddition { voice }))
            .collect(),
    )?;
    let completed = apply_patch(source, &patch)?;
    let restored = remove_patch(&completed, &patch)?;
    if restored != *source {
        return Err(GenerationError::Invariant(
            "remove(apply(source, patch), patch) did not restore the source".to_owned(),
        ));
    }
    Ok(CounterpointGenerationResult {
        counterpoint,
        completed,
        patch,
        analysis: report,
        score: assignment.score,
        fingerprint,
    })
}

pub(crate) fn counterpoint_from_assignment(
    cantus: &Melody,
    policy: &CounterpointGenerationPolicy,
    csp: &CounterpointCsp,
    pitches: &[u8],
) -> Result<Counterpoint, GenerationError> {
    if pitches.len() != csp.variables.len() {
        return Err(GenerationError::Invariant(
            "assignment length differs from compiled variables".to_owned(),
        ));
    }
    let mut voices = vec![cantus.clone()];
    let mut names = vec!["Cantus".to_owned()];
    for voice in 0..policy.voices {
        let channel = Channel::new(u8::try_from((voice + 1) % 16).map_err(|_| {
            GenerationError::Invariant("generated channel does not fit u8".to_owned())
        })?)
        .map_err(|error| GenerationError::InvalidPolicy(error.to_string()))?;
        let items = (0..csp.slots())
            .map(|slot| {
                let pitch = pitches[slot * policy.voices + voice];
                Note::new(
                    csp.rhythm,
                    Pitch::from_midi(pitch),
                    policy.velocity,
                    channel,
                    Articulation::Normal,
                )
                .map(MelodyItem::Note)
                .map_err(GenerationError::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        voices.push(Melody::new(items)?);
        names.push(format!("Generated {}", voice + 1));
    }
    Ok(Counterpoint::new(voices, names)?)
}

fn staff_voices(
    policy: &CounterpointGenerationPolicy,
    csp: &CounterpointCsp,
    pitches: &[u8],
    duration: Time,
    seed: u64,
    fingerprint: &str,
) -> Result<Vec<StaffVoice>, GenerationError> {
    let mut voices = Vec::with_capacity(policy.voices);
    for voice in 0..policy.voices {
        let voice_id = object_id(format!(
            "counterpoint-generation/{seed}/{fingerprint}/voice/{voice}"
        ))?;
        let channel = Channel::new(u8::try_from((voice + 1) % 16).map_err(|_| {
            GenerationError::Invariant("generated channel does not fit u8".to_owned())
        })?)
        .map_err(|error| GenerationError::InvalidPolicy(error.to_string()))?;
        let mut notes = Vec::with_capacity(csp.slots());
        for slot in 0..csp.slots() {
            let path =
                format!("counterpoint-generation/{seed}/{fingerprint}/voice/{voice}/slot/{slot}");
            notes.push(StaffNote {
                voice_id: voice_id.clone(),
                note_id: object_id(format!("{path}/note"))?,
                event_id: object_id(format!("{path}/event"))?,
                onset: csp.variables[slot * policy.voices + voice].onset,
                note: Note::new(
                    csp.rhythm,
                    Pitch::from_midi(pitches[slot * policy.voices + voice]),
                    policy.velocity,
                    channel,
                    Articulation::Normal,
                )?,
            });
        }
        voices.push(StaffVoice {
            id: voice_id,
            name: format!("Generated {}", voice + 1),
            duration,
            notes,
        });
    }
    Ok(voices)
}

pub(crate) fn cantus_staff(cantus: &Melody) -> Result<Staff, GenerationError> {
    let counterpoint = Counterpoint::new(vec![cantus.clone()], vec!["Cantus".to_owned()])?;
    let converted = convert_score(
        &ScoreForm::Counterpoint(counterpoint),
        ScoreFormKind::Staff,
        AmbiguousConversionPolicy::Reject,
    )?;
    match converted.value {
        ScoreForm::Staff(staff) => Ok(staff),
        _ => Err(GenerationError::Invariant(
            "staff conversion returned a different score form".to_owned(),
        )),
    }
}

fn object_id(value: String) -> Result<ObjectId, GenerationError> {
    ObjectId::new(value).map_err(GenerationError::from)
}

fn assignment_fingerprint(pitches: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for pitch in pitches {
        hash ^= u64::from(*pitch);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}
