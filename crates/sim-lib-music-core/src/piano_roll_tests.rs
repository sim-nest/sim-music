use num_rational::Ratio;
use sim_kernel::{Expr, Symbol};
use sim_lib_midi_core::U7;

use crate::{
    Articulation, AutomationCell, Channel, ControlChangeCell, DrumCell, LaneId, LaneKind, Music,
    Note, ObjectCell, PerformanceEvent, PerformanceIntent, PerformanceTake, PianoRoll,
    PianoRollCell, PianoRollLane, ScaleDegreeCell, Tick, TimedNote,
};

fn note(midi: u8) -> Note {
    Note::new(
        Ratio::new(1, 4),
        crate::Pitch::from_midi(midi),
        100,
        Channel::new(0).expect("channel"),
        Articulation::Normal,
    )
    .expect("note")
}

fn tick(ticks: i64) -> Tick {
    Tick::new(ticks, 480).expect("tick")
}

#[test]
fn piano_roll_lanes_keep_supported_cell_families() {
    let channel = Channel::new(0).expect("channel");
    let roll = PianoRoll::from_lanes(vec![
        PianoRollLane::new(
            LaneId::new("notes"),
            LaneKind::Note,
            vec![PianoRollCell::Note(TimedNote {
                onset: Ratio::new(1, 4),
                note: note(64),
            })],
        )
        .expect("note lane"),
        PianoRollLane::new(
            LaneId::new("drums"),
            LaneKind::Drum,
            vec![PianoRollCell::Drum(DrumCell {
                onset: Ratio::new(0, 1),
                duration: Ratio::new(1, 8),
                key: U7(36),
                velocity: U7(100),
                channel,
            })],
        )
        .expect("drum lane"),
        PianoRollLane::new(
            LaneId::new("scale"),
            LaneKind::ScaleDegree,
            vec![PianoRollCell::ScaleDegree(ScaleDegreeCell {
                onset: Ratio::new(1, 2),
                duration: Ratio::new(1, 8),
                degree: 5,
                octave: 0,
                velocity: U7(96),
                channel,
            })],
        )
        .expect("scale lane"),
        PianoRollLane::new(
            LaneId::new("objects"),
            LaneKind::Object,
            vec![PianoRollCell::Object(ObjectCell {
                onset: Ratio::new(3, 4),
                duration: Ratio::new(1, 4),
                object: Symbol::qualified("music/playable", "phrase"),
            })],
        )
        .expect("object lane"),
        PianoRollLane::new(
            LaneId::new("automation"),
            LaneKind::Automation,
            vec![PianoRollCell::Automation(AutomationCell {
                time: Ratio::new(1, 8),
                target: Symbol::qualified("music/param", "cutoff"),
                value: 42,
            })],
        )
        .expect("automation lane"),
        PianoRollLane::new(
            LaneId::new("control"),
            LaneKind::Control,
            vec![PianoRollCell::ControlChange(ControlChangeCell {
                time: Ratio::new(1, 8),
                channel,
                controller: U7(74),
                value: U7(64),
            })],
        )
        .expect("control lane"),
    ])
    .expect("roll");

    assert_eq!(roll.lanes.len(), 6);
    assert_eq!(roll.items.len(), 2);
    assert_eq!(roll.items[0].note.pitch.to_midi(), Some(36));
    assert_eq!(roll.items[1].note.pitch.to_midi(), Some(64));
    assert!(matches!(roll.to_expr(), Expr::Map(entries) if entries.len() == 4));
}

#[test]
fn lane_kind_mismatch_is_rejected() {
    let err = PianoRollLane::new(
        LaneId::new("wrong"),
        LaneKind::Control,
        vec![PianoRollCell::Note(TimedNote {
            onset: Ratio::new(0, 1),
            note: note(60),
        })],
    )
    .expect_err("mismatch");

    assert!(matches!(
        err,
        crate::MusicError::PianoRollLaneCellMismatch { .. }
    ));
}

#[test]
fn captured_performance_take_imports_as_editable_cells() {
    let channel = Channel::new(0).expect("channel");
    let take = PerformanceTake::new(
        Symbol::qualified("music/performance-source", "keyboard"),
        Symbol::qualified("music/performance-take", "take-1"),
        vec![
            PerformanceEvent {
                lane_id: LaneId::new("perf"),
                source_id: Symbol::qualified("music/performance-source", "keyboard"),
                input_time: tick(0),
                time: tick(0),
                intent: PerformanceIntent::NoteOn {
                    pitch: crate::Pitch::from_midi(60),
                    velocity: 100,
                    channel,
                },
            },
            PerformanceEvent {
                lane_id: LaneId::new("perf"),
                source_id: Symbol::qualified("music/performance-source", "keyboard"),
                input_time: tick(120),
                time: tick(120),
                intent: PerformanceIntent::NoteOff {
                    pitch: crate::Pitch::from_midi(60),
                    velocity: 64,
                    channel,
                },
            },
        ],
    )
    .expect("take");

    let roll = PianoRoll::from_performance_take(&take).expect("roll");
    assert_eq!(roll.items.len(), 1);
    assert_eq!(roll.lanes[0].id.as_ref(), "performance-notes");
    assert_eq!(roll.items[0].note.duration, Ratio::new(1, 16));

    let Music::PianoRoll(clip) = take.as_clip().expect("clip") else {
        panic!("expected piano-roll clip");
    };
    assert_eq!(clip, roll);
}
