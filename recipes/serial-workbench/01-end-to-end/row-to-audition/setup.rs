use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use sim_lib_discrete_search::{NeverInterrupt, SearchControl};
use sim_lib_music_core::{
    Articulation, Channel, MusicObject, Note, ObjectId, Pitch, PitchClass, Score, StaffNote, Time,
};
use sim_lib_music_lower::LowerOpts;
use sim_lib_music_notation::export_lilypond;
use sim_lib_music_serial::{
    BuiltInPracticeRule, CompletionCandidate, CompletionRequest, DeclaredWaivers, EventPlacement,
    NoteAddition, OrdinalRef, PracticeId, PracticeRuleId, RowInstanceId,
    SerialCompletionAllowances, SerialCompletionRequest, SerialOrigin, SerialPlan, SerialPractice,
    SerialRenderOptions, SerialRole, StrictEventSpec, StrictRealizationContext, StructuralLicense,
    StructuralReadingId, complete_serial, default_realizer_registry, lower_serial_score,
    realize_strict, render_serial_audition_score, write_serial_smf,
};
use sim_lib_music_transform::{remove_additive_staff_patch, render_serial_notation_score};
use sim_lib_pitch_scale::{PlayerScale, Scale};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

#[derive(Debug, Deserialize)]
struct FixtureManifest {
    fixture: Vec<FixtureRow>,
}

#[derive(Debug, Deserialize)]
struct FixtureRow {
    id: String,
    source: String,
    data_status: String,
    proves: String,
    contains_copyrighted_score: bool,
}

#[derive(Debug)]
pub struct WorkbenchOutcome {
    pub strict_note_count: usize,
    pub modal_note_count: usize,
    pub midi_bytes: usize,
    pub lilypond_chars: usize,
    pub audition_events: usize,
}

fn fixture_manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/workbench-fixtures.toml")
}

pub fn validate_fixture_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(fixture_manifest_path())?;
    let manifest: FixtureManifest = toml::from_str(&source)?;
    if manifest.fixture.is_empty() {
        return Err("fixture manifest must contain at least one fixture".into());
    }
    for fixture in &manifest.fixture {
        if fixture.id.trim().is_empty()
            || fixture.source.trim().is_empty()
            || fixture.data_status.trim().is_empty()
            || fixture.proves.trim().is_empty()
        {
            return Err(
                format!("fixture {} is missing required coverage fields", fixture.id).into(),
            );
        }
        if fixture.contains_copyrighted_score {
            return Err(format!(
                "fixture {} violates the no-copyrighted-score rule",
                fixture.id
            )
            .into());
        }
    }
    Ok(())
}

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

fn build_row_plan() -> Result<SerialPlan, Box<dyn std::error::Error>> {
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
    let row_id = RowInstanceId::new("row/workbench/op25-p0")?;
    let license = StructuralLicense::new(
        StructuralReadingId::new("reading/serial-workbench")?,
        "serial workbench structural reading",
    )?;
    let event =
        |id: &str,
         ordinal: usize,
         voice: &str|
         -> Result<sim_lib_music_serial::PlannedSerialEvent, Box<dyn std::error::Error>> {
            Ok(sim_lib_music_serial::PlannedSerialEvent {
                id: sim_lib_music_serial::SerialEventId::new(id)?,
                ordinals: vec![OrdinalRef::new(row_id.clone(), ordinal)],
                role: SerialRole::Structural,
                origin: SerialOrigin::Structural {
                    rationale: "serial workbench statement".to_owned(),
                },
                voice: ObjectId::new(voice)?,
                placement: EventPlacement::independent(),
                parents: Vec::new(),
                licenses: vec![license.clone()],
            })
        };
    Ok(SerialPlan::try_new(
        [(row_id.clone(), row)].into_iter().collect(),
        [
            event("event/a", 0, "voice/high")?,
            event("event/b", 1, "voice/high")?,
            event("event/c", 2, "voice/low")?,
            event("event/d", 3, "voice/high")?,
            event("event/e", 4, "voice/low")?,
            event("event/f", 5, "voice/high")?,
            event("event/g", 6, "voice/low")?,
            event("event/h", 7, "voice/high")?,
            event("event/i", 8, "voice/low")?,
            event("event/j", 9, "voice/high")?,
            event("event/k", 10, "voice/low")?,
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
    )?)
}

fn strict_specs() -> Result<
    BTreeMap<sim_lib_music_serial::SerialEventId, StrictEventSpec>,
    Box<dyn std::error::Error>,
> {
    let channel = Channel::new(0)?;
    [
        (
            "event/a",
            StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Accent),
        ),
        (
            "event/b",
            StrictEventSpec::notes(4, quarter(), 92, channel, Articulation::Tenuto),
        ),
        (
            "event/c",
            StrictEventSpec::notes(3, quarter(), 88, channel, Articulation::Marcato),
        ),
        (
            "event/d",
            StrictEventSpec::notes(4, quarter(), 84, channel, Articulation::Tenuto),
        ),
        (
            "event/e",
            StrictEventSpec::notes(3, quarter(), 84, channel, Articulation::Tenuto),
        ),
        (
            "event/f",
            StrictEventSpec::notes(5, quarter(), 90, channel, Articulation::Normal),
        ),
        (
            "event/g",
            StrictEventSpec::notes(3, quarter(), 90, channel, Articulation::Normal),
        ),
        (
            "event/h",
            StrictEventSpec::notes(4, quarter(), 78, channel, Articulation::Staccato),
        ),
        (
            "event/i",
            StrictEventSpec::notes(3, quarter(), 78, channel, Articulation::Staccato),
        ),
        (
            "event/j",
            StrictEventSpec::notes(4, quarter(), 88, channel, Articulation::Accent),
        ),
        (
            "event/k",
            StrictEventSpec::notes(3, quarter(), 88, channel, Articulation::Accent),
        ),
        (
            "event/l",
            StrictEventSpec::notes(4, quarter(), 96, channel, Articulation::Marcato),
        ),
    ]
    .into_iter()
    .map(|(id, spec)| Ok((sim_lib_music_serial::SerialEventId::new(id)?, spec)))
    .collect()
}

pub fn row_to_audition() -> Result<WorkbenchOutcome, Box<dyn std::error::Error>> {
    validate_fixture_manifest()?;

    let plan = build_row_plan()?;
    let strict = realize_strict(&plan, &StrictRealizationContext::new(strict_specs()?))?;
    let mut modal_context = StrictRealizationContext::new(strict_specs()?);
    modal_context.modal_scale = Some(PlayerScale::from_scale(Scale::dorian(PitchClass::C)));
    let registry = default_realizer_registry();
    let modal = registry.realize_named("realizer/modal-degree-cycle", &plan, &modal_context)?;

    assert!(strict.ledger().is_preserved("serial/chromatic-aggregate"));
    assert!(modal.ledger().is_relaxed("serial/chromatic-aggregate"));

    let modal_pitch = modal.spine_report().expect("modal spine report").entries[0]
        .landed_pitch
        .to_midi()
        .expect("modal midi pitch");
    let practice = SerialPractice::new(
        PracticeId::new("practice/serial-workbench")?,
        vec![
            Arc::new(BuiltInPracticeRule::aggregate(PracticeRuleId::new(
                "rule/aggregate",
            )?)),
            Arc::new(BuiltInPracticeRule::order(PracticeRuleId::new(
                "rule/order",
            )?)),
            Arc::new(BuiltInPracticeRule::repeats(PracticeRuleId::new(
                "rule/repeats",
            )?)),
        ],
    );
    let completion = complete_serial(
        &modal,
        &practice,
        &DeclaredWaivers::default(),
        &SerialCompletionRequest {
            completion: CompletionRequest {
                candidates: vec![CompletionCandidate::Note(NoteAddition {
                    note: note(
                        "voice/high",
                        "added-modal",
                        modal_pitch,
                        Time::from_integer(0),
                        quarter(),
                    ),
                })],
                min_candidates: 1,
                max_candidates: Some(1),
                pitch_ranges: Vec::new(),
            },
            allowances: SerialCompletionAllowances {
                current_partition: false,
                stated_pitch_classes: false,
                aggregate_remainder: false,
                modal_projection: true,
                ..SerialCompletionAllowances::default()
            },
        },
        SearchControl::default(),
        &NeverInterrupt,
    )?;
    assert_eq!(completion.structural_plan, plan);
    assert_eq!(completion.structural_before, completion.structural_after);
    let restored = remove_additive_staff_patch(
        &completion.generic.after,
        &sim_lib_music_transform::AdditiveStaffPatch {
            voices: completion.generic.patch.voices.clone(),
            notes: completion.generic.patch.notes.clone(),
        },
    )?;
    assert_eq!(restored.value, completion.generic.before);

    let render_options = SerialRenderOptions::default();
    let midi = write_serial_smf(&modal, &render_options, &LowerOpts::default())?;
    let notation_score = render_serial_notation_score(&modal, &render_options)?;
    let total_duration = notation_score.body.duration();
    let export_score = Score::new(
        notation_score.tempo_bpm,
        (*total_duration.numer() as u8, *total_duration.denom() as u8),
        notation_score.key.clone(),
        notation_score.body.clone(),
    )?;
    let lilypond = export_lilypond(&export_score)?;
    let _audition = render_serial_audition_score(&modal, &render_options)?;
    let lowered_audition = lower_serial_score(&modal, &render_options, &LowerOpts::default())?;

    assert_eq!(&midi[..4], b"MThd");
    assert!(lilypond.contains("\\score"));
    assert!(!lowered_audition.tracks.is_empty());

    Ok(WorkbenchOutcome {
        strict_note_count: strict.notes().len(),
        modal_note_count: modal.notes().len(),
        midi_bytes: midi.len(),
        lilypond_chars: lilypond.len(),
        audition_events: lowered_audition
            .tracks
            .iter()
            .map(|track| track.events.len())
            .sum(),
    })
}
