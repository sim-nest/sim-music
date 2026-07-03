// Tests tweak one option off a default baseline; Default-then-assign keeps the
// varied field obvious per case.
#![allow(clippy::field_reassign_with_default)]

use num_rational::Ratio;

use super::*;
use sim_lib_midi_core::{
    Channel, ChannelMessage, MetaEvent, MidiPayload, U7, U14, bpm_to_us_per_quarter,
};
use sim_lib_midi_smf::SmfFile;
use sim_lib_music_core::{
    Articulation, Chord, ControlChangeCell, Counterpoint, LaneId, LaneKind, Melody, MelodyItem,
    Music, Note, PianoRoll, PianoRollCell, PianoRollLane, PitchBendCell, PolyPressureCell, Rest,
    ScaleDegreeCell, Score, Seq, Time, TimedNote,
};
use sim_lib_pitch_core::Pitch;

fn quarter() -> Time {
    Ratio::new(1, 4)
}

fn note(midi: u8) -> Note {
    Note {
        duration: quarter(),
        pitch: Pitch::from_midi(midi),
        velocity: 100,
        channel: Channel(0),
        articulation: Articulation::Normal,
    }
}

fn channel_events(file: &SmfFile) -> Vec<sim_lib_midi_core::MidiEvent> {
    file.tracks
        .iter()
        .flat_map(|track| track.events.iter())
        .filter(|event| matches!(event.payload, MidiPayload::Channel(_)))
        .cloned()
        .collect()
}

fn track_name(file: &SmfFile, index: usize) -> Option<Vec<u8>> {
    file.tracks[index]
        .events
        .iter()
        .find_map(|event| match &event.payload {
            MidiPayload::Meta(MetaEvent::Other(bucket)) if bucket.type_byte == 0x03 => {
                Some(bucket.data.clone())
            }
            _ => None,
        })
}

fn channel_events_in_track(file: &SmfFile, index: usize) -> Vec<sim_lib_midi_core::MidiEvent> {
    file.tracks[index]
        .events
        .iter()
        .filter(|event| matches!(event.payload, MidiPayload::Channel(_)))
        .cloned()
        .collect()
}

#[test]
fn note_lowers_to_on_and_off() {
    let file = lower(&note(60), &LowerOpts::default()).unwrap();
    let events = channel_events(&file);
    assert_eq!(events.len(), 2);
    assert!(matches!(
        events[0].payload,
        MidiPayload::Channel(ChannelMessage::NoteOn { .. })
    ));
    assert_eq!(events[0].time.ticks, 0);
    assert!(matches!(
        events[1].payload,
        MidiPayload::Channel(ChannelMessage::NoteOff { .. })
    ));
    assert_eq!(events[1].time.ticks, 480);
}

#[test]
fn rest_lowers_to_no_channel_events_and_advances_time() {
    let rest = Rest::new(quarter()).unwrap();
    let seq = Seq {
        children: vec![Box::new(rest), Box::new(note(60))],
    };
    let file = lower(&seq, &LowerOpts::default()).unwrap();
    let events = channel_events(&file);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].time.ticks, 480);
    assert_eq!(events[1].time.ticks, 960);
}

#[test]
fn chord_lowers_in_parallel() {
    let chord = Chord {
        duration: quarter(),
        symbol: "C".to_owned(),
        pitches: vec![Pitch::from_midi(60), Pitch::from_midi(64)],
        velocity: 100,
        channel: Channel(0),
    };
    let file = lower(&Music::Chord(chord), &LowerOpts::default()).unwrap();
    let events = channel_events(&file);
    assert_eq!(events[0].time.ticks, events[1].time.ticks);
    assert_eq!(events[2].time.ticks, events[3].time.ticks);
}

#[test]
fn note_off_sorts_before_note_on_at_equal_time() {
    let seq = Seq {
        children: vec![Box::new(note(60)), Box::new(note(62))],
    };
    let file = lower(&seq, &LowerOpts::default()).unwrap();
    let boundary = channel_events(&file)
        .into_iter()
        .filter(|event| event.time.ticks == 480)
        .collect::<Vec<_>>();
    assert!(matches!(
        boundary[0].payload,
        MidiPayload::Channel(ChannelMessage::NoteOff { .. })
    ));
    assert!(matches!(
        boundary[1].payload,
        MidiPayload::Channel(ChannelMessage::NoteOn { .. })
    ));
}

#[test]
fn score_tempo_time_signature_and_key_meta_are_at_zero() {
    let score = Score::new(
        90,
        (3, 4),
        Some("F".to_owned()),
        Music::Rest(Rest::new(quarter()).unwrap()),
    )
    .unwrap();
    let file = lower_score(&score, &LowerOpts::default()).unwrap();
    assert!(file.tracks[0].events.iter().any(|event| matches!(
        event.payload,
        MidiPayload::Meta(MetaEvent::Tempo {
            us_per_quarter,
        }) if event.time.ticks == 0 && us_per_quarter == bpm_to_us_per_quarter(90.0)
    )));
    assert!(file.tracks[0].events.iter().any(|event| matches!(
        event.payload,
        MidiPayload::Meta(MetaEvent::TimeSig {
            num: 3,
            den_pow2: 2,
            ..
        }) if event.time.ticks == 0
    )));
    assert!(file.tracks[0].events.iter().any(|event| matches!(
        event.payload,
        MidiPayload::Meta(MetaEvent::KeySig {
            sharps_flats: -1,
            minor: false,
        }) if event.time.ticks == 0
    )));
}

#[test]
fn common_rational_duration_is_exact_at_480_tpq() {
    let mut note = note(60);
    note.duration = Ratio::new(1, 8);
    let file = lower(&note, &LowerOpts::default()).unwrap();
    let events = channel_events(&file);
    assert_eq!(events[1].time.ticks, 240);
}

#[test]
fn inexact_tick_conversion_is_diagnostic() {
    let mut note = note(60);
    note.duration = Ratio::new(1, 7);
    assert_eq!(
        lower(&note, &LowerOpts::default()).unwrap_err(),
        LowerError::InexactTime
    );
}

#[test]
fn invalid_pitch_velocity_channel_and_tpq_are_diagnostics() {
    let mut bad_pitch = note(60);
    bad_pitch.pitch = Pitch::from_semitone(-128);
    assert_eq!(
        lower(&bad_pitch, &LowerOpts::default()).unwrap_err(),
        LowerError::PitchOutOfRange
    );

    let mut bad_velocity = note(60);
    bad_velocity.velocity = 0;
    assert_eq!(
        lower(&bad_velocity, &LowerOpts::default()).unwrap_err(),
        LowerError::VelocityOutOfRange { velocity: 0 }
    );

    let mut bad_channel = note(60);
    bad_channel.channel = Channel(16);
    assert_eq!(
        lower(&bad_channel, &LowerOpts::default()).unwrap_err(),
        LowerError::ChannelOutOfRange { channel: 16 }
    );

    let mut bad_opts = LowerOpts::default();
    bad_opts.tpq = 0;
    assert_eq!(
        lower(&note(60), &bad_opts).unwrap_err(),
        LowerError::ZeroTpq
    );
}

#[test]
fn write_smf_emits_standard_midi_file_bytes() {
    let score = Score::new(120, (4, 4), None, Music::Note(note(60))).unwrap();
    let bytes = write_smf(&score, &LowerOpts::default()).unwrap();
    assert_eq!(&bytes[..4], b"MThd");
}

#[test]
fn by_channel_split_creates_named_parallel_tracks() {
    let mut upper = note(72);
    upper.channel = Channel(0);
    let mut lower_note = note(48);
    lower_note.channel = Channel(1);
    let obj = Music::Par(sim_lib_music_core::Par {
        children: vec![Box::new(upper), Box::new(lower_note)],
    });
    let opts = LowerOpts {
        track_split: TrackSplit::ByChannel,
        ..LowerOpts::default()
    };
    let file = lower(&obj, &opts).unwrap();
    assert_eq!(file.format, sim_lib_midi_smf::SmfFormat::Simultaneous);
    assert_eq!(file.tracks.len(), 3);
    assert_eq!(
        track_name(&file, 0).as_deref(),
        Some(b"Conductor".as_slice())
    );
    assert_eq!(
        track_name(&file, 1).as_deref(),
        Some(b"Channel 1".as_slice())
    );
    assert_eq!(
        track_name(&file, 2).as_deref(),
        Some(b"Channel 2".as_slice())
    );
    assert_eq!(channel_events_in_track(&file, 1).len(), 2);
    assert_eq!(channel_events_in_track(&file, 2).len(), 2);
}

#[test]
fn counterpoint_split_uses_voice_names() {
    let soprano = Melody::new(vec![MelodyItem::Note(note(72))]).unwrap();
    let bass = Melody::new(vec![MelodyItem::Note(note(48))]).unwrap();
    let counterpoint = Counterpoint::new(
        vec![soprano, bass],
        vec!["Soprano".to_owned(), "Bass".to_owned()],
    )
    .unwrap();
    let score = Score::new(
        120,
        (4, 4),
        Some("C".to_owned()),
        Music::Counterpoint(counterpoint),
    )
    .unwrap();
    let opts = LowerOpts {
        track_split: TrackSplit::CounterpointVoices,
        ..LowerOpts::default()
    };
    let file = lower_score(&score, &opts).unwrap();
    assert_eq!(file.tracks.len(), 3);
    assert_eq!(
        track_name(&file, 0).as_deref(),
        Some(b"Conductor".as_slice())
    );
    assert_eq!(track_name(&file, 1).as_deref(), Some(b"Soprano".as_slice()));
    assert_eq!(track_name(&file, 2).as_deref(), Some(b"Bass".as_slice()));
}

#[test]
fn equivalent_under_lowering_uses_canonical_smf() {
    let direct = Music::Note(note(60));
    let seq = Music::Seq(Seq {
        children: vec![
            Box::new(Rest::new(Time::from_integer(0)).unwrap()),
            Box::new(note(60)),
        ],
    });
    assert!(equivalent_under_lowering(&direct, &seq, &LowerOpts::default()).unwrap());
}

#[test]
fn piano_roll_lowers_note_control_pitch_and_pressure_cells() {
    let channel = Channel::new(0).unwrap();
    let roll = PianoRoll::from_lanes(vec![
        PianoRollLane::new(
            LaneId::new("notes"),
            LaneKind::Note,
            vec![PianoRollCell::Note(TimedNote {
                onset: Time::from_integer(0),
                note: note(60),
            })],
        )
        .unwrap(),
        PianoRollLane::new(
            LaneId::new("controls"),
            LaneKind::Control,
            vec![
                PianoRollCell::ControlChange(ControlChangeCell {
                    time: Ratio::new(1, 16),
                    channel,
                    controller: U7(74),
                    value: U7(64),
                }),
                PianoRollCell::PitchBend(PitchBendCell {
                    time: Ratio::new(1, 8),
                    channel,
                    value: U14(8192),
                }),
                PianoRollCell::PolyPressure(PolyPressureCell {
                    time: Ratio::new(3, 16),
                    channel,
                    key: U7(60),
                    pressure: U7(70),
                }),
            ],
        )
        .unwrap(),
    ])
    .unwrap();

    let file = lower(&Music::PianoRoll(roll), &LowerOpts::default()).unwrap();
    let events = channel_events(&file);

    assert!(events.iter().any(|event| matches!(
        event.payload,
        MidiPayload::Channel(ChannelMessage::NoteOn { .. })
    )));
    assert!(events.iter().any(|event| matches!(
        event.payload,
        MidiPayload::Channel(ChannelMessage::ControlChange {
            cc: U7(74),
            value: U7(64),
            ..
        })
    )));
    assert!(events.iter().any(|event| matches!(
        event.payload,
        MidiPayload::Channel(ChannelMessage::PitchBend {
            value: U14(8192),
            ..
        })
    )));
    assert!(events.iter().any(|event| matches!(
        event.payload,
        MidiPayload::Channel(ChannelMessage::PolyAftertouch {
            pressure: U7(70),
            ..
        })
    )));
}

#[test]
fn piano_roll_smf_export_fails_closed_for_scale_degree_cells() {
    let roll = PianoRoll::from_lanes(vec![
        PianoRollLane::new(
            LaneId::new("scale"),
            LaneKind::ScaleDegree,
            vec![PianoRollCell::ScaleDegree(ScaleDegreeCell {
                onset: Time::from_integer(0),
                duration: Ratio::new(1, 4),
                degree: 5,
                octave: 0,
                velocity: U7(100),
                channel: Channel::new(0).unwrap(),
            })],
        )
        .unwrap(),
    ])
    .unwrap();

    assert_eq!(
        lower(&Music::PianoRoll(roll), &LowerOpts::default()).unwrap_err(),
        LowerError::UnsupportedPianoRollCell {
            lane: "scale".to_owned(),
            cell_kind: "scale-degree".to_owned(),
        }
    );
}
