use std::io::Cursor;

use sim_lib_midi_core::{
    Channel, ChannelMessage, MetaBucket, MetaEvent, MidiEvent, MidiPayload, SysExEvent, TickTime,
    U7, U14, synthetic_origin,
};

use crate::writer::{MAX_SMF_VLQ, checked_chunk_len, checked_payload_len};
use crate::{
    SmfError, SmfFile, SmfFormat, SmfTrack, SmfWriteOptions, decode_vlq, encode_vlq, read_smf,
    write_smf_with_options,
};

#[test]
fn vlq_known_vectors_decode_and_encode_exactly() {
    assert_eq!(encode_vlq(0), vec![0x00]);
    assert_eq!(encode_vlq(0x7f), vec![0x7f]);
    assert_eq!(encode_vlq(0x80), vec![0x81, 0x00]);
    assert_eq!(encode_vlq(0x3fff), vec![0xff, 0x7f]);
    assert_eq!(
        decode_vlq(&mut Cursor::new(vec![0x81, 0x00])).unwrap(),
        0x80
    );
    assert_eq!(
        decode_vlq(&mut Cursor::new(vec![0xff, 0x7f])).unwrap(),
        0x3fff
    );
}

#[test]
fn malformed_running_status_returns_structured_error() {
    let bytes = [
        b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 1, 0xE0, b'M', b'T', b'r', b'k', 0, 0, 0,
        4, 0x00, 0x3c, 0x40, 0xff, 0x2f, 0x00,
    ];
    let error = read_smf(&bytes).unwrap_err();
    assert_eq!(error, SmfError::MalformedRunningStatus { offset: 23 });
}

#[test]
fn smf_headers_round_trip_for_formats_0_1_and_2() {
    for (format, tracks) in [
        (SmfFormat::SingleTrack, vec![minimal_track(480)]),
        (
            SmfFormat::Simultaneous,
            vec![minimal_track(480), minimal_track(480)],
        ),
        (
            SmfFormat::Independent,
            vec![minimal_track(480), minimal_track(480)],
        ),
    ] {
        let file = SmfFile {
            format,
            tpq: 480,
            tracks,
        };
        let bytes = write_smf_with_options(&file, SmfWriteOptions::default()).unwrap();
        let decoded = read_smf(&bytes).unwrap();
        assert_eq!(decoded.format, format);
        assert_eq!(decoded.tpq, 480);
        assert_eq!(decoded.tracks.len(), file.tracks.len());
    }
}

#[test]
fn canonical_fixtures_round_trip_byte_identically_without_running_status() {
    let fixture = canonical_format_zero_fixture(false);
    let decoded = read_smf(&fixture).unwrap();
    let encoded = write_smf_with_options(&decoded, SmfWriteOptions::default()).unwrap();
    assert_eq!(encoded, fixture);
}

#[test]
fn canonical_fixtures_round_trip_byte_identically_with_running_status() {
    let fixture = canonical_format_zero_fixture(true);
    let decoded = read_smf(&fixture).unwrap();
    let encoded = write_smf_with_options(
        &decoded,
        SmfWriteOptions {
            running_status: true,
        },
    )
    .unwrap();
    assert_eq!(encoded, fixture);
}

#[test]
fn running_status_files_read_correctly() {
    let fixture = canonical_format_zero_fixture(true);
    let decoded = read_smf(&fixture).unwrap();
    let events = &decoded.tracks[0].events;
    assert!(matches!(
        events[1].payload,
        MidiPayload::Channel(ChannelMessage::NoteOn { .. })
    ));
    assert!(matches!(
        events[2].payload,
        MidiPayload::Channel(ChannelMessage::NoteOn { .. })
    ));
}

#[test]
fn multi_track_reader_emits_time_sorted_events_and_preserves_last_track() {
    let file = read_smf(&format_one_merge_fixture()).unwrap();
    let merged = file.merged_events();
    let pairs = merged
        .iter()
        .map(|tracked| {
            (
                tracked.last_track,
                tracked.event.time.ticks,
                tracked.event.payload.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(pairs[0].0, 0);
    assert_eq!(pairs[1].0, 1);
    assert_eq!(pairs[2].0, 0);
    assert_eq!(pairs[3].0, 1);
    assert!(pairs.windows(2).all(|window| window[0].1 <= window[1].1));
}

#[test]
fn merge_cursor_skips_exhausted_earlier_tracks() {
    let file = SmfFile {
        format: SmfFormat::Simultaneous,
        tpq: 480,
        tracks: vec![
            SmfTrack {
                events: vec![MidiEvent {
                    time: TickTime::new(0, 480).unwrap(),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Meta(MetaEvent::Tempo {
                        us_per_quarter: 500_000,
                    }),
                }],
            },
            SmfTrack {
                events: vec![
                    MidiEvent {
                        time: TickTime::new(120, 480).unwrap(),
                        origin: synthetic_origin(),
                        payload: MidiPayload::Channel(ChannelMessage::NoteOn {
                            ch: Channel::new(1).unwrap(),
                            key: U7(64),
                            vel: U7(90),
                        }),
                    },
                    MidiEvent {
                        time: TickTime::new(240, 480).unwrap(),
                        origin: synthetic_origin(),
                        payload: MidiPayload::Channel(ChannelMessage::NoteOff {
                            ch: Channel::new(1).unwrap(),
                            key: U7(64),
                            vel: U7(0),
                        }),
                    },
                ],
            },
        ],
    };

    let merged = file.merged_events();
    let pairs = merged
        .iter()
        .map(|tracked| (tracked.last_track, tracked.event.time.ticks))
        .collect::<Vec<_>>();

    assert_eq!(pairs, vec![(0, 0), (1, 120), (1, 240)]);
}

#[test]
fn writer_rejects_tpq_with_smpte_high_bit() {
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        tpq: 0x8000,
        tracks: vec![minimal_track(480)],
    };

    let error = write_smf_with_options(&file, SmfWriteOptions::default()).unwrap_err();

    assert_eq!(error, SmfError::TpqOutOfRange(0x8000));
}

#[test]
fn writer_rejects_too_many_tracks() {
    let tracks = (0..=u16::MAX)
        .map(|_| minimal_track(480))
        .collect::<Vec<_>>();
    let file = SmfFile {
        format: SmfFormat::Simultaneous,
        tpq: 480,
        tracks,
    };

    let error = write_smf_with_options(&file, SmfWriteOptions::default()).unwrap_err();

    assert_eq!(error, SmfError::TrackCountOutOfRange(65_536));
}

#[test]
fn writer_rejects_delta_above_four_byte_vlq_limit() {
    let delta = i64::from(MAX_SMF_VLQ) + 1;
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        tpq: 480,
        tracks: vec![SmfTrack {
            events: vec![MidiEvent {
                time: TickTime::new(delta, 480).unwrap(),
                origin: synthetic_origin(),
                payload: MidiPayload::Meta(MetaEvent::EndOfTrack),
            }],
        }],
    };

    let error = write_smf_with_options(&file, SmfWriteOptions::default()).unwrap_err();

    assert_eq!(error, SmfError::DeltaOutOfRange(delta));
}

#[test]
fn writer_length_guards_reject_unrepresentable_lengths() {
    if let Some(chunk_len) = usize::try_from(u32::MAX).unwrap().checked_add(1) {
        assert_eq!(
            checked_chunk_len(chunk_len),
            Err(SmfError::ChunkTooLarge(chunk_len))
        );
    }

    let payload_len = usize::try_from(MAX_SMF_VLQ).unwrap() + 1;
    assert_eq!(
        checked_payload_len(payload_len),
        Err(SmfError::PayloadTooLarge(payload_len))
    );
}

#[test]
fn unknown_meta_and_sysex_round_trip() {
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        tpq: 480,
        tracks: vec![SmfTrack {
            events: vec![
                MidiEvent {
                    time: TickTime::new(0, 480).unwrap(),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Meta(MetaEvent::Other(MetaBucket {
                        type_byte: 0x7f,
                        data: vec![1, 2, 3],
                    })),
                },
                MidiEvent {
                    time: TickTime::new(120, 480).unwrap(),
                    origin: synthetic_origin(),
                    payload: MidiPayload::SysEx(SysExEvent::F0 {
                        data: vec![0x7d, 0x10, 0x11],
                    }),
                },
            ],
        }],
    };
    let bytes = write_smf_with_options(&file, SmfWriteOptions::default()).unwrap();
    let decoded = read_smf(&bytes).unwrap();
    assert_eq!(
        decoded.tracks[0].events[0].payload,
        file.tracks[0].events[0].payload
    );
    assert_eq!(
        decoded.tracks[0].events[1].payload,
        file.tracks[0].events[1].payload
    );
}

#[test]
fn channel_control_pitch_and_pressure_messages_round_trip() {
    let channel = Channel::new(0).unwrap();
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        tpq: 480,
        tracks: vec![SmfTrack {
            events: vec![
                MidiEvent {
                    time: TickTime::new(0, 480).unwrap(),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Channel(ChannelMessage::ControlChange {
                        ch: channel,
                        cc: U7(74),
                        value: U7(64),
                    }),
                },
                MidiEvent {
                    time: TickTime::new(120, 480).unwrap(),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Channel(ChannelMessage::PitchBend {
                        ch: channel,
                        value: U14(8192),
                    }),
                },
                MidiEvent {
                    time: TickTime::new(240, 480).unwrap(),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Channel(ChannelMessage::PolyAftertouch {
                        ch: channel,
                        key: U7(60),
                        pressure: U7(70),
                    }),
                },
            ],
        }],
    };

    let bytes = write_smf_with_options(&file, SmfWriteOptions::default()).unwrap();
    let decoded = read_smf(&bytes).unwrap();
    assert!(decoded.tracks[0].events.iter().any(|event| matches!(
        event.payload,
        MidiPayload::Channel(ChannelMessage::ControlChange {
            cc: U7(74),
            value: U7(64),
            ..
        })
    )));
    assert!(decoded.tracks[0].events.iter().any(|event| matches!(
        event.payload,
        MidiPayload::Channel(ChannelMessage::PitchBend {
            value: U14(8192),
            ..
        })
    )));
    assert!(decoded.tracks[0].events.iter().any(|event| matches!(
        event.payload,
        MidiPayload::Channel(ChannelMessage::PolyAftertouch {
            pressure: U7(70),
            ..
        })
    )));
}

fn minimal_track(tpq: u32) -> SmfTrack {
    SmfTrack {
        events: vec![MidiEvent {
            time: TickTime::new(0, tpq).unwrap(),
            origin: synthetic_origin(),
            payload: MidiPayload::Meta(MetaEvent::EndOfTrack),
        }],
    }
}

fn canonical_format_zero_fixture(running_status: bool) -> Vec<u8> {
    let track = if running_status {
        vec![
            0x00, 0xff, 0x51, 0x03, 0x07, 0xa1, 0x20, 0x00, 0x90, 60, 100, 0x78, 62, 96, 0x78,
            0x80, 60, 0, 0x00, 62, 0, 0x00, 0xff, 0x2f, 0x00,
        ]
    } else {
        vec![
            0x00, 0xff, 0x51, 0x03, 0x07, 0xa1, 0x20, 0x00, 0x90, 60, 100, 0x78, 0x90, 62, 96,
            0x78, 0x80, 60, 0, 0x00, 0x80, 62, 0, 0x00, 0xff, 0x2f, 0x00,
        ]
    };
    wrap_track(SmfFormat::SingleTrack, 480, vec![track])
}

fn format_one_merge_fixture() -> Vec<u8> {
    let track0 = vec![
        0x00, 0xff, 0x51, 0x03, 0x07, 0xa1, 0x20, 0x78, 0x90, 60, 100, 0x78, 0x80, 60, 0, 0x00,
        0xff, 0x2f, 0x00,
    ];
    let track1 = vec![
        0x3c, 0x91, 67, 110, 0x78, 0x81, 67, 0, 0x00, 0xff, 0x2f, 0x00,
    ];
    wrap_track(SmfFormat::Simultaneous, 480, vec![track0, track1])
}

fn wrap_track(format: SmfFormat, tpq: u16, tracks: Vec<Vec<u8>>) -> Vec<u8> {
    let format_u16 = match format {
        SmfFormat::SingleTrack => 0u16,
        SmfFormat::Simultaneous => 1u16,
        SmfFormat::Independent => 2u16,
    };
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"MThd");
    bytes.extend_from_slice(&6u32.to_be_bytes());
    bytes.extend_from_slice(&format_u16.to_be_bytes());
    bytes.extend_from_slice(&(tracks.len() as u16).to_be_bytes());
    bytes.extend_from_slice(&tpq.to_be_bytes());
    for track in tracks {
        bytes.extend_from_slice(b"MTrk");
        bytes.extend_from_slice(&(track.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&track);
    }
    bytes
}
