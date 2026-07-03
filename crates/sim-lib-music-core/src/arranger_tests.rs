use num_rational::Ratio;
use sim_kernel::Symbol;

use crate::{
    Arranger, ArrangerPlacement, Articulation, Channel, FilterRef, LaneId, Music, Note, PianoRoll,
    Pitch, PitchClass, PitchRemap, PlacementTransform, PlayContext, PlayEvent, PlayableRef,
    StretchPolicy, Time, TimeRange, TimedNote, TracePolicy,
};

fn q() -> Time {
    Ratio::new(1, 4)
}

fn note(midi: u8, duration: Time) -> Note {
    Note::new(
        duration,
        Pitch::from_midi(midi),
        100,
        Channel::new(0).expect("channel"),
        Articulation::Normal,
    )
    .expect("note")
}

fn placement(id: &str, music: Music, at: Time) -> ArrangerPlacement {
    ArrangerPlacement::new(id, PlayableRef::inline(music), at).expect("placement")
}

fn midis(arranger: &Arranger) -> Vec<u8> {
    arranger
        .rendered_notes()
        .iter()
        .map(|item| item.note.pitch.to_midi().expect("midi pitch"))
        .collect()
}

fn onsets(arranger: &Arranger) -> Vec<Time> {
    arranger
        .rendered_notes()
        .iter()
        .map(|item| item.onset)
        .collect()
}

#[test]
fn arranger_places_notes_piano_rolls_and_nested_arrangers() {
    let roll = PianoRoll::new(vec![TimedNote {
        onset: q(),
        note: note(64, q()),
    }])
    .expect("roll");
    let nested = Arranger::new(
        vec![placement("nested-note", Music::Note(note(67, q())), q())],
        vec![LaneId::new("notes")],
    )
    .expect("nested");
    let arranger = Arranger::new(
        vec![
            placement("note", Music::Note(note(60, q())), Time::from_integer(0)),
            placement("roll", Music::PianoRoll(roll), q()),
            placement("nested", Music::Arranger(nested), Ratio::new(1, 2)),
        ],
        vec![LaneId::new("notes")],
    )
    .expect("arranger");

    assert_eq!(midis(&arranger), vec![60, 64, 67]);
    assert_eq!(
        onsets(&arranger),
        vec![Time::from_integer(0), Ratio::new(1, 2), Ratio::new(3, 4)]
    );
}

#[test]
fn arranger_stretches_transforms_remaps_and_merges_stably() {
    let roll = PianoRoll::new(vec![
        TimedNote {
            onset: Time::from_integer(0),
            note: note(60, q()),
        },
        TimedNote {
            onset: q(),
            note: note(64, q()),
        },
    ])
    .expect("roll");
    let transformed = placement("b", Music::PianoRoll(roll), Time::from_integer(0))
        .with_duration(Time::from_integer(1))
        .expect("duration")
        .with_stretch(StretchPolicy::FitToDuration)
        .with_transform(vec![
            PlacementTransform::TransposeSemitones(2),
            PlacementTransform::InvertAroundPitch(Pitch::from_midi(62)),
            PlacementTransform::Retrograde,
        ])
        .with_pitch_remap(PitchRemap::PitchClass {
            from: PitchClass::AS,
            to: PitchClass::C,
        });
    let low = placement("a", Music::Note(note(55, q())), Ratio::new(1, 4));
    let arranger =
        Arranger::new(vec![transformed, low], vec![LaneId::new("notes")]).expect("arranger");

    let notes = arranger.rendered_notes();
    let pairs = notes
        .iter()
        .map(|item| (item.onset, item.note.pitch.to_midi().expect("midi pitch")))
        .collect::<Vec<_>>();
    assert_eq!(
        pairs,
        vec![
            (Time::from_integer(0), 48),
            (q(), 55),
            (Ratio::new(1, 2), 62)
        ]
    );
}

#[test]
fn arranger_reports_unresolved_refs_and_filter_diagnostics() {
    let unresolved = ArrangerPlacement::new(
        "missing",
        PlayableRef::symbol(Symbol::qualified("music/playable", "missing")),
        q(),
    )
    .expect("unresolved")
    .with_trace(TracePolicy::Full);
    let filtered = placement(
        "filtered",
        Music::Note(note(60, q())),
        Time::from_integer(0),
    )
    .with_lane(LaneId::new("notes"))
    .with_filter(FilterRef::new(
        Symbol::qualified("music/filter", "not-notes"),
        vec![LaneId::new("controls")],
    ));
    let arranger =
        Arranger::new(vec![unresolved, filtered], vec![LaneId::new("notes")]).expect("arranger");
    let cx = PlayContext::new(TimeRange::from_ticks(0, 960, 480).expect("range"));
    let rendered = arranger.render_arrangement(&cx).expect("render");

    assert!(
        rendered
            .events
            .iter()
            .any(|event| matches!(event, PlayEvent::Diagnostic(_)))
    );
    assert!(
        rendered
            .events
            .iter()
            .any(|event| matches!(event, PlayEvent::Trace(_)))
    );
    assert_eq!(arranger.rendered_notes().len(), 0);
}

#[test]
fn arranger_expression_round_trips() {
    let arranger = Arranger::new(
        vec![
            placement("note", Music::Note(note(60, q())), q())
                .with_duration(q())
                .expect("duration")
                .with_stretch(StretchPolicy::TimeRatio(Ratio::new(2, 1)))
                .with_pitch_remap(PitchRemap::Chromatic(1)),
        ],
        vec![LaneId::new("notes")],
    )
    .expect("arranger");

    let decoded = Arranger::from_expr(&arranger.to_expr()).expect("decode");
    assert_eq!(decoded, arranger);
    assert_eq!(midis(&decoded), vec![61]);
}
