use num_rational::Ratio;

// conformance: catalog score conversions preserve identities or report every exact loss.

use crate::{
    AmbiguousConversionPolicy, Articulation, AutomationCell, Channel, Chord, ConversionError,
    ConversionLossKind, LaneId, LaneKind, Melody, MelodyItem, MusicObject, Note, ObjectId,
    PianoRoll, PianoRollCell, PianoRollLane, Pitch, Progression, Rest, ScoreForm, ScoreFormKind,
    Staff, StaffNote, StaffVoice, Time, TimeGrid, convert_score,
};

fn quarter() -> Time {
    Ratio::new(1, 4)
}

fn note(midi: u8, duration: Time) -> Note {
    Note::new(
        duration,
        Pitch::from_midi(midi),
        96,
        Channel::new(0).expect("channel"),
        Articulation::Normal,
    )
    .expect("note")
}

fn staff_note(path: &str, voice: &ObjectId, onset: Time, midi: u8) -> StaffNote {
    StaffNote {
        voice_id: voice.clone(),
        note_id: ObjectId::new(format!("note/{path}")).expect("note id"),
        event_id: ObjectId::new(format!("event/{path}")).expect("event id"),
        onset,
        note: note(midi, quarter()),
    }
}

fn polyphonic_staff() -> Staff {
    let high = ObjectId::new("voice/high").expect("voice");
    let low = ObjectId::new("voice/low").expect("voice");
    Staff::new(vec![
        StaffVoice {
            id: high.clone(),
            name: "High".to_owned(),
            duration: Time::from_integer(1),
            notes: vec![
                staff_note("high/0", &high, Time::from_integer(0), 72),
                staff_note("high/1", &high, quarter(), 74),
            ],
        },
        StaffVoice {
            id: low.clone(),
            name: "Low".to_owned(),
            duration: Time::from_integer(1),
            notes: vec![
                staff_note("low/0", &low, Time::from_integer(0), 48),
                staff_note("low/1", &low, quarter(), 50),
            ],
        },
    ])
    .expect("staff")
}

#[test]
fn melody_staff_round_trip_keeps_exact_time_and_audits_rest_boundaries() {
    let melody = Melody::new(vec![
        MelodyItem::Note(note(60, quarter())),
        MelodyItem::Rest(Rest::new(quarter()).expect("rest")),
        MelodyItem::Note(note(64, Ratio::new(1, 2))),
    ])
    .expect("melody");

    let staff = convert_score(
        &ScoreForm::Melody(melody.clone()),
        ScoreFormKind::Staff,
        AmbiguousConversionPolicy::Reject,
    )
    .expect("to staff");
    assert_eq!(staff.value.kind(), ScoreFormKind::Staff);
    assert_eq!(staff.losses.len(), 1);
    assert_eq!(staff.losses[0].kind, ConversionLossKind::ExplicitRest);
    let ScoreForm::Staff(staff_value) = staff.value else {
        panic!("staff");
    };
    assert_eq!(staff_value.duration(), Time::from_integer(1));

    let restored = convert_score(
        &ScoreForm::Staff(staff_value),
        ScoreFormKind::Melody,
        AmbiguousConversionPolicy::Reject,
    )
    .expect("to melody");
    assert_eq!(restored.value, ScoreForm::Melody(melody));
}

#[test]
fn snapshots_and_changes_round_trip_staff_identities() {
    let staff = polyphonic_staff();
    for kind in [ScoreFormKind::Snapshot, ScoreFormKind::ChangeStream] {
        let encoded = convert_score(
            &ScoreForm::Staff(staff.clone()),
            kind,
            AmbiguousConversionPolicy::Reject,
        )
        .expect("encode event form");
        assert!(encoded.is_lossless());
        assert_eq!(encoded.preserved, staff.object_ids());

        let decoded = convert_score(
            &encoded.value,
            ScoreFormKind::Staff,
            AmbiguousConversionPolicy::Reject,
        )
        .expect("decode event form");
        assert_eq!(decoded.value, ScoreForm::Staff(staff.clone()));
        assert_eq!(decoded.preserved, staff.object_ids());
    }
}

#[test]
fn snapshot_decoder_reports_incomplete_activity_sets() {
    let mut staff = polyphonic_staff();
    staff.voices[0].notes[0].note.duration = Ratio::new(1, 2);
    let omitted = staff.voices[0].notes[0].event_id.clone();
    let encoded = convert_score(
        &ScoreForm::Staff(staff),
        ScoreFormKind::Snapshot,
        AmbiguousConversionPolicy::Reject,
    )
    .expect("encode snapshots");
    let ScoreForm::Snapshot(mut snapshots) = encoded.value else {
        panic!("snapshot");
    };
    snapshots
        .snapshots
        .iter_mut()
        .find(|snapshot| snapshot.at == quarter())
        .expect("quarter-note boundary")
        .sounding
        .retain(|note| note.event_id != omitted);

    let decoded = convert_score(
        &ScoreForm::Snapshot(snapshots),
        ScoreFormKind::Staff,
        AmbiguousConversionPolicy::Reject,
    )
    .expect("decode incomplete snapshots");
    assert!(
        decoded
            .losses
            .iter()
            .any(|loss| loss.kind == ConversionLossKind::InconsistentChange)
    );
}

#[test]
fn ambiguous_monophonic_conversion_requires_a_policy() {
    let staff = polyphonic_staff();
    let rejected = convert_score(
        &ScoreForm::Staff(staff.clone()),
        ScoreFormKind::Melody,
        AmbiguousConversionPolicy::Reject,
    );
    assert!(matches!(
        rejected,
        Err(ConversionError::Ambiguous {
            to: ScoreFormKind::Melody,
            ..
        })
    ));

    let selected = convert_score(
        &ScoreForm::Staff(staff),
        ScoreFormKind::Melody,
        AmbiguousConversionPolicy::KeepHighest,
    )
    .expect("select high line");
    let ScoreForm::Melody(melody) = selected.value else {
        panic!("melody");
    };
    let midis = melody
        .items
        .iter()
        .filter_map(|item| match item {
            MelodyItem::Note(note) => note.pitch.to_midi(),
            MelodyItem::Rest(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(midis, vec![72, 74]);
    assert!(
        selected
            .losses
            .iter()
            .any(|loss| loss.kind == ConversionLossKind::DiscardedVoice)
    );
}

#[test]
fn every_catalog_target_converts_or_reports_its_loss() {
    let staff = polyphonic_staff();
    for target in [
        ScoreFormKind::Melody,
        ScoreFormKind::Chord,
        ScoreFormKind::Staff,
        ScoreFormKind::Counterpoint,
        ScoreFormKind::PianoRoll,
        ScoreFormKind::Snapshot,
        ScoreFormKind::ChangeStream,
        ScoreFormKind::Progression,
    ] {
        let report = convert_score(
            &ScoreForm::Staff(staff.clone()),
            target,
            AmbiguousConversionPolicy::KeepFirst,
        )
        .unwrap_or_else(|error| panic!("{target:?}: {error}"));
        assert_eq!(report.value.kind(), target);
        if !report.is_lossless() {
            assert!(report.losses.iter().all(|loss| !loss.detail.is_empty()));
        }
    }
}

#[test]
fn full_catalog_matrix_round_trips_or_carries_explicit_losses() {
    let staff = polyphonic_staff();
    let melody = Melody::new(vec![
        MelodyItem::Note(note(60, quarter())),
        MelodyItem::Rest(Rest::new(quarter()).expect("rest")),
        MelodyItem::Note(note(62, quarter())),
    ])
    .expect("melody");
    let chord = Chord::new(
        quarter(),
        "Dm",
        vec![Pitch::from_midi(62), Pitch::from_midi(65)],
        90,
        Channel::new(0).expect("channel"),
    )
    .expect("chord");
    let progression =
        Progression::new(Some("D minor".to_owned()), vec![chord.clone()]).expect("progression");
    let counterpoint = convert_score(
        &ScoreForm::Staff(staff.clone()),
        ScoreFormKind::Counterpoint,
        AmbiguousConversionPolicy::KeepFirst,
    )
    .expect("counterpoint")
    .value;
    let piano_roll = convert_score(
        &ScoreForm::Staff(staff.clone()),
        ScoreFormKind::PianoRoll,
        AmbiguousConversionPolicy::KeepFirst,
    )
    .expect("piano roll")
    .value;
    let snapshot = convert_score(
        &ScoreForm::Staff(staff.clone()),
        ScoreFormKind::Snapshot,
        AmbiguousConversionPolicy::KeepFirst,
    )
    .expect("snapshot")
    .value;
    let changes = convert_score(
        &ScoreForm::Staff(staff.clone()),
        ScoreFormKind::ChangeStream,
        AmbiguousConversionPolicy::KeepFirst,
    )
    .expect("changes")
    .value;
    let sources = vec![
        ScoreForm::Melody(melody),
        ScoreForm::Chord(chord),
        ScoreForm::Staff(staff),
        counterpoint,
        piano_roll,
        snapshot,
        changes,
        ScoreForm::Progression(progression),
    ];
    let targets = [
        ScoreFormKind::Melody,
        ScoreFormKind::Chord,
        ScoreFormKind::Staff,
        ScoreFormKind::Counterpoint,
        ScoreFormKind::PianoRoll,
        ScoreFormKind::Snapshot,
        ScoreFormKind::ChangeStream,
        ScoreFormKind::Progression,
    ];

    for source in sources {
        for target in targets {
            let converted = convert_score(&source, target, AmbiguousConversionPolicy::KeepFirst)
                .unwrap_or_else(|error| panic!("{:?} -> {target:?}: {error}", source.kind()));
            let restored = convert_score(
                &converted.value,
                source.kind(),
                AmbiguousConversionPolicy::KeepFirst,
            )
            .unwrap_or_else(|error| panic!("{target:?} -> {:?}: {error}", source.kind()));
            if restored.value != source {
                assert!(
                    !converted.losses.is_empty() || !restored.losses.is_empty(),
                    "{:?} -> {target:?} changed without a loss report",
                    source.kind()
                );
            }
        }
    }
}

#[test]
fn harmonic_and_piano_roll_metadata_losses_are_explicit() {
    let chord = Chord::new(
        quarter(),
        "C",
        vec![Pitch::from_midi(60), Pitch::from_midi(64)],
        90,
        Channel::new(0).expect("channel"),
    )
    .expect("chord");
    let progression =
        Progression::new(Some("C major".to_owned()), vec![chord]).expect("progression");
    let progression_report = convert_score(
        &ScoreForm::Progression(progression),
        ScoreFormKind::Staff,
        AmbiguousConversionPolicy::Reject,
    )
    .expect("progression to staff");
    assert!(
        progression_report
            .losses
            .iter()
            .any(|loss| loss.kind == ConversionLossKind::KeyAnnotation)
    );
    assert!(
        progression_report
            .losses
            .iter()
            .any(|loss| loss.kind == ConversionLossKind::HarmonicLabel)
    );

    let automation = PianoRollCell::Automation(AutomationCell {
        time: Time::from_integer(0),
        target: sim_kernel::Symbol::qualified("music/parameter", "expression"),
        value: 72,
    });
    let roll = PianoRoll::from_lanes_with_time(
        vec![
            PianoRollLane::new(
                LaneId::new("automation"),
                LaneKind::Automation,
                vec![automation],
            )
            .expect("lane"),
        ],
        TimeGrid::new(960, Ratio::new(1, 32)).expect("grid"),
    )
    .expect("roll");
    let roll_report = convert_score(
        &ScoreForm::PianoRoll(roll),
        ScoreFormKind::Staff,
        AmbiguousConversionPolicy::Reject,
    )
    .expect("roll to staff");
    assert!(
        roll_report
            .losses
            .iter()
            .any(|loss| loss.kind == ConversionLossKind::PianoRollGrid)
    );
    assert!(
        roll_report
            .losses
            .iter()
            .any(|loss| loss.kind == ConversionLossKind::NonNoteCell)
    );
}

#[test]
fn staff_duration_is_exact_parallel_maximum() {
    let staff = polyphonic_staff();
    assert_eq!(staff.duration(), Time::from_integer(1));
    assert_eq!(MusicObject::duration(&staff), Time::from_integer(1));
}
