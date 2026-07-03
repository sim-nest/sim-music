use sim_lib_midi_core::{
    Channel, ChannelMessage, MetaEvent, MidiEvent, MidiPayload, TickTime, U7, synthetic_origin,
};

use crate::{
    SmfError, SmfFile, SmfFormat, SmfTrack, SmfWriteOptions, read_smf, write_smf_with_options,
};

fn event(ticks: i64, payload: MidiPayload) -> MidiEvent {
    MidiEvent {
        time: TickTime::new(ticks, 480).expect("tick"),
        origin: synthetic_origin(),
        payload,
    }
}

fn note_on(ticks: i64, key: u8, vel: u8) -> MidiEvent {
    event(
        ticks,
        MidiPayload::Channel(ChannelMessage::NoteOn {
            ch: Channel::new(0).expect("channel"),
            key: U7(key),
            vel: U7(vel),
        }),
    )
}

fn note_off(ticks: i64, key: u8) -> MidiEvent {
    event(
        ticks,
        MidiPayload::Channel(ChannelMessage::NoteOff {
            ch: Channel::new(0).expect("channel"),
            key: U7(key),
            vel: U7(0),
        }),
    )
}

#[test]
fn note_echo_frozen_midi_recipe_has_byte_stable_export() {
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        tpq: 480,
        tracks: vec![SmfTrack {
            events: vec![
                event(
                    0,
                    MidiPayload::Meta(MetaEvent::Tempo {
                        us_per_quarter: 500_000,
                    }),
                ),
                note_on(0, 60, 96),
                note_on(120, 64, 88),
                note_on(240, 67, 80),
                note_off(480, 60),
                note_off(480, 64),
                note_off(480, 67),
            ],
        }],
    };

    let bytes = write_smf_with_options(&file, SmfWriteOptions::default()).expect("smf bytes");

    assert_eq!(
        bytes,
        vec![
            0x4d, 0x54, 0x68, 0x64, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x01, 0x01, 0xe0,
            0x4d, 0x54, 0x72, 0x6b, 0x00, 0x00, 0x00, 0x24, 0x00, 0xff, 0x51, 0x03, 0x07, 0xa1,
            0x20, 0x00, 0x90, 0x3c, 0x60, 0x78, 0x90, 0x40, 0x58, 0x78, 0x90, 0x43, 0x50, 0x81,
            0x70, 0x80, 0x3c, 0x00, 0x00, 0x80, 0x40, 0x00, 0x00, 0x80, 0x43, 0x00, 0x00, 0xff,
            0x2f, 0x00,
        ]
    );
    assert_eq!(
        read_smf(&bytes).expect("round trip").tracks[0].events.len(),
        8
    );
}

#[test]
fn unsupported_export_recipe_rejects_mismatched_format_zero_tracks() {
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        tpq: 480,
        tracks: vec![
            SmfTrack { events: Vec::new() },
            SmfTrack { events: Vec::new() },
        ],
    };

    assert_eq!(
        write_smf_with_options(&file, SmfWriteOptions::default()).expect_err("unsupported export"),
        SmfError::FormatTrackMismatch
    );
}

#[test]
fn smf_recipe_sources_are_registered_for_generated_docs() {
    for source in [
        include_str!("../recipes/02-export-fixtures/note-echo-frozen-midi/recipe.toml"),
        include_str!("../recipes/02-export-fixtures/unsupported-export-failure/recipe.toml"),
    ] {
        assert!(source.contains("smf") || source.contains("failure"));
        assert!(source.contains("codec = \"lisp\""));
    }
}
