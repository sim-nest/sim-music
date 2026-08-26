use std::sync::Arc;

// conformance: MIDI notation workflows realize exact tempo, pedal, overlap, and note-slice policy.

use num_rational::Ratio;
use sim_kernel::{Cx, DefaultFactory, EagerPolicy, ExportKind, Symbol};
use sim_lib_midi_core::{
    CC_ALL_NOTES_OFF, CC_RESET_ALL_CONTROLLERS, CC_SOSTENUTO, CC_SUSTAIN_PEDAL, Channel,
    ChannelMessage, MetaEvent, MidiEvent, MidiPayload, TickTime, U7, U14, meta_view,
    synthetic_origin,
};
use sim_lib_midi_smf::{SmfDivision, SmfFile, SmfFormat, SmfTrack, SmpteRate};
use sim_lib_music_analysis::ChordWindowMode;
use sim_lib_music_core::{Chord, MusicObject, Progression};
use sim_lib_music_lower::{LowerOpts, lower};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_scale::{Key, Mode};

use crate::{
    CounterpointLiftOpts, DanglingNotePolicy, LabelStrategy, MidiLifter, MidiNoteEnd,
    MidiRealizationPolicy, MidiTimelineId, MidiToCounterpoint, OverlapPolicy, PedalPolicy,
    ProgressionLiftOpts, SameTickPolicy, VoiceAssignment, install_music_lift_lib,
    lift_to_counterpoint_report, lift_to_diff_roll_report, lift_to_piano_roll_report,
    lift_to_progression, lift_to_progression_report, realize_midi, realize_midi_pattern,
};

fn event(time: i64, payload: MidiPayload) -> MidiEvent {
    MidiEvent {
        time: TickTime::new(time, 480).expect("tick"),
        origin: synthetic_origin(),
        payload,
    }
}

fn note_on(time: i64, key: u8, channel: u8) -> MidiEvent {
    event(
        time,
        MidiPayload::Channel(ChannelMessage::NoteOn {
            ch: Channel::new(channel).expect("channel"),
            key: U7(key),
            vel: U7(100),
        }),
    )
}

fn note_off(time: i64, key: u8, channel: u8) -> MidiEvent {
    event(
        time,
        MidiPayload::Channel(ChannelMessage::NoteOff {
            ch: Channel::new(channel).expect("channel"),
            key: U7(key),
            vel: U7(0),
        }),
    )
}

fn eot(time: i64) -> MidiEvent {
    event(time, MidiPayload::Meta(MetaEvent::EndOfTrack))
}

#[test]
fn note_on_without_note_off_is_closed_at_eot_with_diagnostic() {
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        division: SmfDivision::metrical(480).unwrap(),
        tracks: vec![SmfTrack {
            events: vec![note_on(0, 60, 0), eot(480)],
        }],
    };
    let report = lift_to_piano_roll_report(&file).expect("lift");
    assert_eq!(report.value.items.len(), 1);
    assert_eq!(report.value.items[0].note.duration, Ratio::new(1, 4));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("closed at end-of-track"))
    );
}

#[test]
fn overlapping_notes_split_under_highest_first() {
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        division: SmfDivision::metrical(480).unwrap(),
        tracks: vec![SmfTrack {
            events: vec![
                note_on(0, 60, 0),
                note_on(120, 67, 0),
                note_off(240, 67, 0),
                note_off(480, 60, 0),
                eot(480),
            ],
        }],
    };
    let report = lift_to_counterpoint_report(
        &file,
        CounterpointLiftOpts {
            min_rest_to_close: Ratio::new(1, 64),
            max_voices_per_track: 4,
            voice_assignment: VoiceAssignment::HighestFirst,
        },
    )
    .expect("counterpoint");
    assert_eq!(report.value.voices.len(), 2);
    let lifted_pitches = report
        .value
        .voices
        .iter()
        .flat_map(|voice| voice.items.iter())
        .filter_map(|item| match item {
            sim_lib_music_core::MelodyItem::Note(note) => Some(note.pitch.to_midi().expect("midi")),
            sim_lib_music_core::MelodyItem::Rest(_) => None,
        })
        .collect::<Vec<_>>();
    assert!(lifted_pitches.contains(&60));
    assert!(lifted_pitches.contains(&67));
}

#[test]
fn channel_only_never_splits_a_track() {
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        division: SmfDivision::metrical(480).unwrap(),
        tracks: vec![SmfTrack {
            events: vec![
                note_on(0, 60, 0),
                note_on(0, 64, 0),
                note_off(240, 60, 0),
                note_off(240, 64, 0),
                eot(240),
            ],
        }],
    };
    let report = lift_to_counterpoint_report(
        &file,
        CounterpointLiftOpts {
            min_rest_to_close: Ratio::new(1, 64),
            max_voices_per_track: 4,
            voice_assignment: VoiceAssignment::ChannelOnly,
        },
    )
    .expect("counterpoint");
    assert!(report.value.voices.len() <= 1);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("rejected"))
    );
}

#[test]
fn progression_lifter_can_use_multiple_label_strategies() {
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        division: SmfDivision::metrical(480).unwrap(),
        tracks: vec![SmfTrack {
            events: vec![
                note_on(0, 60, 0),
                note_on(0, 64, 0),
                note_on(0, 67, 0),
                note_off(480, 60, 0),
                note_off(480, 64, 0),
                note_off(480, 67, 0),
                eot(480),
            ],
        }],
    };
    for strategy in [
        LabelStrategy::JazzChord,
        LabelStrategy::Functional,
        LabelStrategy::SetClass,
    ] {
        let report = lift_to_progression_report(
            &file,
            ProgressionLiftOpts {
                key_hint: Some(Key {
                    tonic: PitchClass::C,
                    mode: Mode::Major,
                }),
                label_strategy: strategy,
                ..ProgressionLiftOpts::default()
            },
        )
        .expect("progression");
        assert_eq!(report.value.chords.len(), 1);
        assert!(!report.value.chords[0].symbol.contains('?'));
    }
}

#[test]
fn lift_lower_lift_is_stable_for_simple_progression() {
    let progression = Progression::new(
        Some("C-major".to_owned()),
        vec![
            Chord::new(
                Ratio::new(1, 4),
                "C",
                vec![
                    sim_lib_music_core::Pitch::from_midi(60),
                    sim_lib_music_core::Pitch::from_midi(64),
                    sim_lib_music_core::Pitch::from_midi(67),
                ],
                100,
                Channel::new(0).expect("channel"),
            )
            .expect("chord"),
            Chord::new(
                Ratio::new(1, 4),
                "G",
                vec![
                    sim_lib_music_core::Pitch::from_midi(67),
                    sim_lib_music_core::Pitch::from_midi(71),
                    sim_lib_music_core::Pitch::from_midi(74),
                ],
                100,
                Channel::new(0).expect("channel"),
            )
            .expect("chord"),
        ],
    )
    .expect("progression");
    let smf = lower(&progression, &LowerOpts::default()).expect("lower");
    let lifted = lift_to_progression(
        &smf,
        ProgressionLiftOpts {
            grid: Ratio::new(1, 4),
            key_hint: Some(Key {
                tonic: PitchClass::C,
                mode: Mode::Major,
            }),
            label_strategy: LabelStrategy::JazzChord,
            window_mode: ChordWindowMode::StartingNotes,
            ..ProgressionLiftOpts::default()
        },
    )
    .expect("lift");
    assert_eq!(lifted.chords.len(), 2);
    assert_eq!(lifted.duration(), progression.duration());
}

#[test]
fn midi_to_diff_roll_emits_expected_masks() {
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        division: SmfDivision::metrical(480).unwrap(),
        tracks: vec![SmfTrack {
            events: vec![
                note_on(0, 60, 0),
                note_on(240, 64, 0),
                note_off(480, 60, 0),
                note_off(720, 64, 0),
                eot(720),
            ],
        }],
    };
    let report = lift_to_diff_roll_report(&file).expect("diff");
    assert_eq!(report.value.frames.len(), 4);
    assert_eq!(report.value.frames[0].started.bits, 1u128 << 60);
    assert_eq!(report.value.frames[1].started.bits, 1u128 << 64);
}

#[test]
fn install_music_lift_lib_registers_runtime_exports() {
    let mut cx = Cx::new(
        Arc::new(EagerPolicy),
        Arc::new(DefaultFactory),
        sim_kernel::HandleSeed::new(0x2646_88f7_1e51_0694),
    );
    install_music_lift_lib(&mut cx).expect("install");
    install_music_lift_lib(&mut cx).expect("install");
    let lib = cx
        .registry()
        .lib(&Symbol::new("music-lift"))
        .expect("music-lift lib");
    assert!(lib.exports.iter().any(|record| {
        record.kind == ExportKind::named("MidiLifter")
            && record.symbol == Symbol::qualified("music", "MidiToProgression")
    }));
}

#[test]
fn midi_to_counterpoint_trait_symbol_is_stable() {
    let lifter = MidiToCounterpoint::default();
    assert_eq!(lifter.symbol(), "music:MidiToCounterpoint");
}

#[test]
fn midi_to_piano_roll_lifts_control_pitch_and_pressure_lanes() {
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        division: SmfDivision::metrical(480).unwrap(),
        tracks: vec![SmfTrack {
            events: vec![
                note_on(0, 60, 0),
                event(
                    120,
                    MidiPayload::Channel(ChannelMessage::ControlChange {
                        ch: Channel::new(0).expect("channel"),
                        cc: U7(74),
                        value: U7(64),
                    }),
                ),
                event(
                    180,
                    MidiPayload::Channel(ChannelMessage::PitchBend {
                        ch: Channel::new(0).expect("channel"),
                        value: U14(8192),
                    }),
                ),
                event(
                    240,
                    MidiPayload::Channel(ChannelMessage::PolyAftertouch {
                        ch: Channel::new(0).expect("channel"),
                        key: U7(60),
                        pressure: U7(70),
                    }),
                ),
                note_off(480, 60, 0),
                eot(480),
            ],
        }],
    };

    let report = lift_to_piano_roll_report(&file).expect("lift");
    let control_lane = report
        .value
        .lanes
        .iter()
        .find(|lane| lane.kind == sim_lib_music_core::LaneKind::Control)
        .expect("control lane");

    assert_eq!(report.value.items.len(), 1);
    assert!(
        control_lane
            .cells
            .iter()
            .any(|cell| matches!(cell, sim_lib_music_core::PianoRollCell::ControlChange(_)))
    );
    assert!(
        control_lane
            .cells
            .iter()
            .any(|cell| matches!(cell, sim_lib_music_core::PianoRollCell::PitchBend(_)))
    );
    assert!(
        control_lane
            .cells
            .iter()
            .any(|cell| matches!(cell, sim_lib_music_core::PianoRollCell::PolyPressure(_)))
    );
}

#[test]
fn track_names_become_voice_names_when_available() {
    let file = SmfFile {
        format: SmfFormat::Simultaneous,
        division: SmfDivision::metrical(480).unwrap(),
        tracks: vec![
            SmfTrack {
                events: vec![
                    event(
                        0,
                        MidiPayload::Meta(MetaEvent::Other(meta_view::make_track_name("Soprano"))),
                    ),
                    note_on(0, 72, 0),
                    note_off(240, 72, 0),
                    eot(240),
                ],
            },
            SmfTrack {
                events: vec![
                    event(
                        0,
                        MidiPayload::Meta(MetaEvent::Other(meta_view::make_track_name("Bass"))),
                    ),
                    note_on(0, 48, 1),
                    note_off(240, 48, 1),
                    eot(240),
                ],
            },
        ],
    };
    let report = lift_to_counterpoint_report(&file, CounterpointLiftOpts::default()).expect("cp");
    assert_eq!(
        report.value.voice_names,
        vec!["Soprano".to_owned(), "Bass".to_owned()]
    );
}

fn control(time: i64, controller: U7, value: u8) -> MidiEvent {
    event(
        time,
        MidiPayload::Channel(ChannelMessage::ControlChange {
            ch: Channel::new(0).expect("channel"),
            cc: controller,
            value: U7(value),
        }),
    )
}

fn single_track(events: Vec<MidiEvent>) -> SmfFile {
    SmfFile {
        format: SmfFormat::SingleTrack,
        division: SmfDivision::metrical(480).expect("division"),
        tracks: vec![SmfTrack { events }],
    }
}

#[test]
fn sustain_and_sostenuto_release_only_the_notes_each_pedal_holds() {
    let file = single_track(vec![
        note_on(0, 60, 0),
        control(100, CC_SUSTAIN_PEDAL, 127),
        note_off(200, 60, 0),
        control(300, CC_SOSTENUTO, 127),
        control(400, CC_SUSTAIN_PEDAL, 0),
        note_on(450, 64, 0),
        note_off(500, 64, 0),
        control(600, CC_SOSTENUTO, 0),
        eot(600),
    ]);
    let realization = realize_midi(&file, MidiRealizationPolicy::default()).expect("realization");
    let notes = &realization.timelines[0].notes;

    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].id.event_index, 0);
    assert_eq!(notes[0].key_release, Some(Ratio::new(5, 48)));
    assert_eq!(notes[0].sounding_until, Ratio::new(5, 16));
    assert_eq!(notes[0].ended_by, MidiNoteEnd::SostenutoRelease);
    assert_eq!(notes[1].note.pitch.to_midi(), Some(64));
    assert_eq!(notes[1].sounding_until, Ratio::new(25, 96));
    assert_eq!(notes[1].ended_by, MidiNoteEnd::NoteOff);
}

#[test]
fn all_notes_off_obeys_pedals_and_reset_releases_deferred_notes() {
    let file = single_track(vec![
        note_on(0, 60, 0),
        control(100, CC_SUSTAIN_PEDAL, 127),
        control(200, CC_ALL_NOTES_OFF, 0),
        control(300, CC_RESET_ALL_CONTROLLERS, 0),
        eot(300),
    ]);
    let realization = realize_midi(&file, MidiRealizationPolicy::default()).expect("realization");
    let note = &realization.timelines[0].notes[0];

    assert_eq!(note.key_release, Some(Ratio::new(5, 48)));
    assert_eq!(note.sounding_until, Ratio::new(5, 32));
    assert_eq!(note.ended_by, MidiNoteEnd::ResetControllers);
}

#[test]
fn same_tick_policy_controls_whether_a_note_off_sees_pedal_down() {
    let file = single_track(vec![
        note_on(0, 60, 0),
        control(480, CC_SUSTAIN_PEDAL, 127),
        note_off(480, 60, 0),
        control(960, CC_SUSTAIN_PEDAL, 0),
        eot(960),
    ]);
    let encoded = realize_midi(
        &file,
        MidiRealizationPolicy {
            same_tick: SameTickPolicy::Encoded,
            ..MidiRealizationPolicy::default()
        },
    )
    .expect("encoded realization");
    let offs_first =
        realize_midi(&file, MidiRealizationPolicy::default()).expect("ordered realization");

    assert_eq!(
        encoded.timelines[0].notes[0].sounding_until,
        Ratio::new(1, 2)
    );
    assert_eq!(
        offs_first.timelines[0].notes[0].sounding_until,
        Ratio::new(1, 4)
    );
}

#[test]
fn overlap_pairing_is_explicit_and_reject_mode_fails_closed() {
    let file = single_track(vec![
        note_on(0, 60, 0),
        note_on(120, 60, 0),
        note_off(240, 60, 0),
        note_off(360, 60, 0),
        eot(360),
    ]);
    let realized = |overlap| {
        realize_midi(
            &file,
            MidiRealizationPolicy {
                overlap,
                pedals: PedalPolicy::Ignore,
                ..MidiRealizationPolicy::default()
            },
        )
    };
    let fifo = realized(OverlapPolicy::Fifo).expect("FIFO");
    let lifo = realized(OverlapPolicy::Lifo).expect("LIFO");

    assert_eq!(fifo.timelines[0].notes[0].id.event_index, 0);
    assert_eq!(fifo.timelines[0].notes[0].sounding_until, Ratio::new(1, 8));
    assert_eq!(lifo.timelines[0].notes[0].id.event_index, 0);
    assert_eq!(lifo.timelines[0].notes[0].sounding_until, Ratio::new(3, 16));
    assert!(matches!(
        realized(OverlapPolicy::Reject),
        Err(crate::LiftError::OverlappingNote {
            tick: 120,
            channel: 0,
            key: 60
        })
    ));
}

#[test]
fn unmatched_notes_are_diagnostic_or_rejected_by_policy() {
    let file = single_track(vec![note_off(0, 62, 0), note_on(120, 60, 0), eot(480)]);
    let closed = realize_midi(&file, MidiRealizationPolicy::default()).expect("diagnostic close");

    assert_eq!(closed.timelines[0].notes.len(), 1);
    assert_eq!(
        closed.timelines[0].notes[0].ended_by,
        MidiNoteEnd::EndOfTimeline
    );
    assert_eq!(closed.timelines[0].diagnostics.len(), 2);
    assert!(
        closed.timelines[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unmatched note-off"))
    );
    assert!(matches!(
        realize_midi(
            &file,
            MidiRealizationPolicy {
                dangling_notes: DanglingNotePolicy::Reject,
                ..MidiRealizationPolicy::default()
            }
        ),
        Err(crate::LiftError::DanglingNotes { count: 1 })
    ));
}

#[test]
fn format_two_realization_returns_independent_identity_bearing_timelines() {
    let file = SmfFile {
        format: SmfFormat::Independent,
        division: SmfDivision::metrical(480).expect("division"),
        tracks: vec![
            SmfTrack {
                events: vec![note_on(0, 60, 0), note_off(480, 60, 0), eot(480)],
            },
            SmfTrack {
                events: vec![note_on(0, 67, 0), note_off(240, 67, 0), eot(240)],
            },
        ],
    };
    let realization = realize_midi(&file, MidiRealizationPolicy::default()).expect("realization");

    assert_eq!(realization.timelines.len(), 2);
    assert_eq!(realization.timelines[0].id, MidiTimelineId::Pattern(0));
    assert_eq!(realization.timelines[1].id, MidiTimelineId::Pattern(1));
    assert_eq!(realization.timelines[0].notes[0].id.track, 0);
    assert_eq!(realization.timelines[1].notes[0].id.track, 1);
    assert_eq!(
        realize_midi_pattern(&file, 1, MidiRealizationPolicy::default())
            .expect("pattern")
            .id,
        MidiTimelineId::Pattern(1)
    );
    assert!(matches!(
        realize_midi_pattern(&file, 2, MidiRealizationPolicy::default()),
        Err(crate::LiftError::PatternOutOfRange {
            pattern: 2,
            patterns: 2
        })
    ));
    assert!(matches!(
        lift_to_piano_roll_report(&file),
        Err(crate::LiftError::IndependentPatternsRequireSelection)
    ));
}

#[test]
fn smpte_realization_requires_an_explicit_metrical_adapter() {
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        division: SmfDivision::smpte(SmpteRate::Fps25, 40).expect("SMPTE division"),
        tracks: vec![SmfTrack { events: Vec::new() }],
    };

    assert!(matches!(
        realize_midi(&file, MidiRealizationPolicy::default()),
        Err(crate::LiftError::MetricalTimingRequired)
    ));
}

#[test]
fn realized_note_slices_preserve_same_pitch_multiplicity_and_source_ids() {
    let file = SmfFile {
        format: SmfFormat::Simultaneous,
        division: SmfDivision::metrical(480).expect("division"),
        tracks: vec![
            SmfTrack {
                events: vec![note_on(0, 60, 0), note_off(480, 60, 0), eot(480)],
            },
            SmfTrack {
                events: vec![note_on(0, 60, 0), note_off(240, 60, 0), eot(240)],
            },
        ],
    };
    let realization = realize_midi(&file, MidiRealizationPolicy::default()).expect("realization");
    let slices = realization.timelines[0].note_slices();

    assert_eq!(slices.len(), 2);
    assert_eq!(slices[0].notes.len(), 2);
    assert_eq!(slices[0].notes[0].note.pitch.to_midi(), Some(60));
    assert_eq!(slices[0].notes[1].note.pitch.to_midi(), Some(60));
    assert_ne!(slices[0].notes[0].id, slices[0].notes[1].id);
    assert_eq!(slices[1].notes.len(), 1);
}
