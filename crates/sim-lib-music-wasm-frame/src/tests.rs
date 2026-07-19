use super::*;

use sim_lib_midi_core::{U7, synthetic_origin};
use sim_lib_midi_smf::{SmfFile, SmfFormat, SmfTrack, write_smf};
use sim_lib_music_core::{
    Articulation, Channel, ChannelMessage, MidiEvent, MidiPayload, Music, Note, Score, TickTime,
    Time, parse_pitch,
};

#[test]
fn lower_music_file_to_frames_round_trips() {
    let score = Score::new(
        120,
        (4, 4),
        Some("C".to_owned()),
        Music::Note(
            Note::new(
                Time::new(1, 4),
                parse_pitch("C4").unwrap(),
                100,
                Channel(0),
                Articulation::Normal,
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let music = sim_lib_music_shapes::encode_music_file(&score).unwrap();
    let frames = lower_music_file_to_frames(&music).unwrap();
    let lifted = lift_frames_to_music_file(&frames).unwrap();
    assert!(lifted.starts_with("#(music/Score v1 "));
    assert!(lifted.contains("#(Score "));
}

#[test]
fn analyze_smf_bytes_reports_all_views() {
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        tpq: 480,
        tracks: vec![SmfTrack {
            events: vec![
                MidiEvent {
                    time: TickTime::new(0, 480).unwrap(),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Channel(ChannelMessage::NoteOn {
                        ch: Channel(0),
                        key: U7(60),
                        vel: U7(100),
                    }),
                },
                MidiEvent {
                    time: TickTime::new(480, 480).unwrap(),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Channel(ChannelMessage::NoteOff {
                        ch: Channel(0),
                        key: U7(60),
                        vel: U7(0),
                    }),
                },
            ],
        }],
    };
    let report = analyze_smf_bytes(&write_smf(&file).unwrap()).unwrap();
    assert!(report.progression.starts_with("#(Progression"));
    assert!(report.music_file.starts_with("#(music/Score v1 "));
    assert!(report.music_file.contains("#(Score "));
}

#[test]
fn music_wasm_entry_points_are_stable() {
    let entries = music_wasm_engine_entry_points();

    assert_eq!(entries.lower_frames, "music-wasm-lower-frames");
    assert_eq!(entries.lift_frames, "music-wasm-lift-frames");
    assert_eq!(entries.analyze, "music-wasm-analyze");
}
