use std::collections::BTreeMap;
use std::sync::Arc;

use sim_lib_discrete_search::{NeverInterrupt, SearchControl};
use sim_lib_music_core::{Articulation, Channel, Note, ObjectId, Pitch, PitchClass, StaffNote, Time};
use sim_lib_music_serial::{
    BuiltInPracticeRule, CompletionCandidate, CompletionRequest, DeclaredWaivers, EventPlacement,
    NoteAddition, OrdinalRef, PracticeId, PracticeRuleId, RowInstanceId,
    SerialCompletionAllowances, SerialCompletionRequest, SerialOrigin, SerialPlan,
    SerialPractice, SerialRole, StrictEventSpec, StrictRealizationContext, StructuralLicense,
    StructuralReadingId, complete_serial, realize_strict,
};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

fn quarter() -> Time {
    Time::new(1, 4)
}

fn note(voice_id: &str, event: &str, pitch: u8, onset: Time, duration: Time) -> StaffNote {
    StaffNote {
        voice_id: ObjectId::new(voice_id).expect("voice id"),
        note_id: ObjectId::new(format!("note/{event}")).expect("note id"),
        event_id: ObjectId::new(format!("event/{event}")).expect("event id"),
        onset,
        note: Note::new(
            duration,
            Pitch::from_midi(pitch),
            96,
            Channel::new(0).expect("channel"),
            Articulation::Normal,
        )
        .expect("note"),
    }
}

pub fn serial_consonance_completion() -> Result<(), Box<dyn std::error::Error>> {
    let row = ToneRow::try_from_classes([
        PitchClass::E,
        PitchClass::F,
        PitchClass::G,
        PitchClass::CS,
        PitchClass::FS,
        PitchClass::DS,
        PitchClass::GS,
        PitchClass::D,
        PitchClass::B,
        PitchClass::C,
        PitchClass::A,
        PitchClass::AS,
    ])?
    .apply(RowOperation::new(RowFamily::P, 0));
    let row_id = RowInstanceId::new("row/op25/p0")?;
    let license = StructuralLicense::new(
        StructuralReadingId::new("reading/serial-completion-recipe")?,
        "serial completion recipe reading",
    )?;
    let event = |id: &str, ordinal: usize, voice: &str| -> Result<sim_lib_music_serial::PlannedSerialEvent, Box<dyn std::error::Error>> {
        Ok(sim_lib_music_serial::PlannedSerialEvent {
            id: sim_lib_music_serial::SerialEventId::new(id)?,
            ordinals: vec![OrdinalRef::new(row_id.clone(), ordinal)],
            role: SerialRole::Structural,
            origin: SerialOrigin::Structural {
                rationale: "recipe structural statement".to_owned(),
            },
            voice: ObjectId::new(voice)?,
            placement: EventPlacement::independent(),
            parents: Vec::new(),
            licenses: vec![license.clone()],
        })
    };
    let plan = SerialPlan::try_new(
        [(row_id.clone(), row)].into_iter().collect(),
        [
            event("event/a", 0, "voice/high")?,
            event("event/b", 1, "voice/high")?,
            event("event/c", 2, "voice/high")?,
            event("event/d", 3, "voice/high")?,
            event("event/e", 4, "voice/high")?,
            event("event/f", 5, "voice/high")?,
            event("event/g", 6, "voice/high")?,
            event("event/h", 7, "voice/high")?,
            event("event/i", 8, "voice/high")?,
            event("event/j", 9, "voice/high")?,
            event("event/k", 10, "voice/high")?,
            event("event/l", 11, "voice/high")?,
        ]
        .into_iter()
        .map(|event| (event.id.clone(), event))
        .collect(),
        [
            ("event/a", "event/b"),
            ("event/b", "event/c"),
            ("event/c", "event/d"),
            ("event/d", "event/e"),
            ("event/e", "event/f"),
            ("event/f", "event/g"),
            ("event/g", "event/h"),
            ("event/h", "event/i"),
            ("event/i", "event/j"),
            ("event/j", "event/k"),
            ("event/k", "event/l"),
        ]
        .into_iter()
        .map(|(before, after)| {
            Ok((
                sim_lib_music_serial::SerialEventId::new(before)?,
                sim_lib_music_serial::SerialEventId::new(after)?,
            ))
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?,
    )?;
    let channel = Channel::new(0)?;
    let specs = [
        "event/a", "event/b", "event/c", "event/d", "event/e", "event/f", "event/g", "event/h",
        "event/i", "event/j", "event/k", "event/l",
    ]
    .into_iter()
    .map(|id| {
        Ok((
            sim_lib_music_serial::SerialEventId::new(id)?,
            StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Normal),
        ))
    })
    .collect::<Result<BTreeMap<_, _>, Box<dyn std::error::Error>>>()?;
    let realization = realize_strict(&plan, &StrictRealizationContext::new(specs))?;
    let practice = SerialPractice::new(
        PracticeId::new("practice/serial-completion-recipe")?,
        vec![
            Arc::new(BuiltInPracticeRule::aggregate(PracticeRuleId::new("rule/aggregate")?)),
            Arc::new(BuiltInPracticeRule::order(PracticeRuleId::new("rule/order")?)),
            Arc::new(BuiltInPracticeRule::repeats(PracticeRuleId::new("rule/repeats")?)),
        ],
    );
    let result = complete_serial(
        &realization,
        &practice,
        &DeclaredWaivers::default(),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![CompletionCandidate::Note(NoteAddition {
                    note: note("voice/high", "added-e", 64, Time::from_integer(0), quarter()),
                })],
                min_candidates: 1,
                max_candidates: Some(1),
                pitch_ranges: Vec::new(),
            },
            allowances: SerialCompletionAllowances::default(),
        },
        SearchControl::default(),
        &NeverInterrupt,
    )?;

    assert_eq!(result.structural_plan, plan);
    assert_eq!(result.structural_before, result.structural_after);
    assert!(
        result
            .sounding_after
            .entries()
            .iter()
            .any(|entry| entry.rule_id.as_str() == "rule/repeats")
    );
    Ok(())
}
