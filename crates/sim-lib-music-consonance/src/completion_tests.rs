use std::collections::BTreeSet;

// conformance: additive completion is bounded, reversible, and identity preserving.

use num_rational::Ratio;
use sim_lib_discrete_search::{NeverInterrupt, SearchControl, SearchStatus};
use sim_lib_music_core::{
    Articulation, Channel, Note, ObjectId, Pitch, Staff, StaffNote, StaffVoice, Time,
};

use crate::{
    Addition, AdditionKind, ChordAddition, CompletionConstraints, CompletionRequest,
    ConsonancePatch, ConsonancePolicy, DoublingAddition, MetricBounds, MetricFamily,
    MetricThreshold, NoteAddition, OrnamentAddition, PedalAddition, PitchRangeConstraint,
    PreservationConstraints, StyleConstraints, TimeSpan, VoiceAddition, apply_patch,
    complete_staff, remove_patch,
};

fn time(numerator: i64, denominator: i64) -> Time {
    Ratio::new(numerator, denominator)
}

fn id(value: &str) -> ObjectId {
    ObjectId::new(value).expect("identity")
}

fn note(voice: &str, name: &str, pitch: u8, onset: Time, duration: Time) -> StaffNote {
    StaffNote {
        voice_id: id(voice),
        note_id: id(&format!("note/{name}")),
        event_id: id(&format!("event/{name}")),
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

fn source_staff(notes: Vec<StaffNote>) -> Staff {
    Staff::new(vec![StaffVoice {
        id: id("voice/main"),
        name: "Main".to_owned(),
        duration: time(1, 1),
        notes,
    }])
    .expect("staff")
}

#[test]
fn every_typed_addition_applies_without_rewriting_and_inverts_exactly() {
    let source = source_staff(vec![
        note("voice/main", "source-c", 60, time(0, 1), time(1, 1)),
        note("voice/main", "source-g", 67, time(1, 2), time(1, 2)),
    ]);
    let original = source.clone();
    let additions = vec![
        Addition::Note(NoteAddition {
            note: note("voice/main", "single", 64, time(0, 1), time(1, 4)),
        }),
        Addition::Ornament(OrnamentAddition {
            anchor_event_id: id("event/source-c"),
            notes: vec![
                note("voice/main", "ornament-a", 62, time(1, 4), time(1, 8)),
                note("voice/main", "ornament-b", 59, time(3, 8), time(1, 8)),
            ],
        }),
        Addition::Chord(ChordAddition {
            label: Some("C major".to_owned()),
            notes: vec![
                note("voice/main", "chord-e", 64, time(1, 2), time(1, 4)),
                note("voice/main", "chord-g", 67, time(1, 2), time(1, 4)),
            ],
        }),
        Addition::Pedal(PedalAddition {
            label: Some("tonic".to_owned()),
            note: note("voice/main", "pedal", 48, time(0, 1), time(1, 1)),
        }),
        Addition::Doubling(DoublingAddition {
            source_event_id: id("event/source-c"),
            note: note("voice/main", "double-c", 72, time(0, 1), time(1, 1)),
        }),
        Addition::Voice(VoiceAddition {
            voice: StaffVoice {
                id: id("voice/counterline"),
                name: "Counterline".to_owned(),
                duration: time(1, 1),
                notes: vec![note(
                    "voice/counterline",
                    "voice-a",
                    55,
                    time(0, 1),
                    time(1, 1),
                )],
            },
        }),
    ];
    let patch = ConsonancePatch::new(&source, additions.clone()).expect("typed patch");
    let completed = apply_patch(&source, &patch).expect("apply");
    let restored = remove_patch(&completed, &patch).expect("remove");

    assert_eq!(source, original, "construction and apply borrow immutably");
    assert_eq!(restored, source);
    assert_eq!(restored.object_ids(), source.object_ids());
    assert_eq!(
        additions
            .iter()
            .map(Addition::kind)
            .collect::<BTreeSet<_>>(),
        [
            AdditionKind::Note,
            AdditionKind::Ornament,
            AdditionKind::Chord,
            AdditionKind::Pedal,
            AdditionKind::Doubling,
            AdditionKind::Voice,
        ]
        .into_iter()
        .collect()
    );

    let mut altered = completed;
    let added = altered
        .voices
        .iter_mut()
        .find(|voice| voice.id == id("voice/main"))
        .and_then(|voice| {
            voice
                .notes
                .iter_mut()
                .find(|note| note.event_id == id("event/single"))
        })
        .expect("added note");
    added.note.velocity = 1;
    assert!(remove_patch(&altered, &patch).is_err());
}

#[test]
fn bounded_completion_meets_thresholds_and_labels_partial_search_honestly() {
    let source = source_staff(vec![
        note("voice/main", "c", 60, time(0, 1), time(1, 1)),
        note("voice/main", "fs", 66, time(0, 1), time(1, 1)),
    ]);
    let target = TimeSpan::new(time(0, 1), time(1, 2)).expect("target");
    let protected = TimeSpan::new(time(1, 2), time(1, 1)).expect("protected");
    let request = CompletionRequest {
        candidates: vec![
            Addition::Note(NoteAddition {
                note: note("voice/main", "candidate-e", 64, time(0, 1), time(1, 2)),
            }),
            Addition::Note(NoteAddition {
                note: note("voice/main", "candidate-g", 67, time(0, 1), time(1, 2)),
            }),
        ],
        constraints: CompletionConstraints {
            thresholds: vec![MetricThreshold {
                family: MetricFamily::Pitch,
                model: "tritone-density".to_owned(),
                span: Some(target.clone()),
                bounds: MetricBounds {
                    max_normalized_density: Some(0.5),
                    ..MetricBounds::default()
                },
            }],
            preservation: PreservationConstraints {
                required_ids: source.object_ids(),
                protected_spans: vec![protected],
            },
            ranges: vec![PitchRangeConstraint {
                voice_id: None,
                lowest: Pitch::from_midi(60),
                highest: Pitch::from_midi(72),
            }],
            style: StyleConstraints {
                allowed_kinds: [AdditionKind::Note].into_iter().collect(),
                min_additions: 1,
                max_additions: Some(1),
                max_added_notes: Some(1),
                max_new_voices: Some(0),
                max_simultaneous_added_notes: Some(1),
                allowed_articulations: vec![Articulation::Normal],
            },
        },
    };
    assert!(
        request
            .constraints
            .accepts_partial(&source, &request.candidates[..1])
            .expect("candidate constraints")
    );

    let result = complete_staff(
        &source,
        &ConsonancePolicy::default(),
        &request,
        SearchControl::default().with_max_results(1),
        &NeverInterrupt,
    )
    .expect("bounded completion");

    assert_eq!(result.patch.additions.len(), 1);
    assert_eq!(result.changed_windows, vec![target]);
    assert_eq!(result.search.status, SearchStatus::Partial);
    assert_eq!(
        result.search.reason.as_deref(),
        Some("result bound reached")
    );
    assert_eq!(result.search.result_count, 1);
    assert_eq!(result.provenance.preserved_ids, source.object_ids());
    assert_eq!(
        remove_patch(
            &apply_patch(&source, &result.patch).expect("apply result"),
            &result.patch
        )
        .expect("remove result"),
        source
    );

    let before_density = result.before.windows[0]
        .pitch
        .iter()
        .find(|metric| metric.model == "tritone-density")
        .expect("before tritone metric")
        .normalized_density;
    let after_density = result.after.windows[0]
        .pitch
        .iter()
        .find(|metric| metric.model == "tritone-density")
        .expect("after tritone metric")
        .normalized_density;
    assert_eq!(before_density, 1.0);
    assert!(after_density <= 0.5);
}
