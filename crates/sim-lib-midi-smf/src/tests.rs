//! Defensive SMF conformance: lossless divisions and extension events plus
//! bounded, fail-closed parsing of adversarial input.

use std::io::Cursor;

use sim_lib_midi_core::{
    Channel, ChannelMessage, MetaBucket, MetaEvent, MidiEvent, MidiPayload, RawBytes, SysExEvent,
    TickTime, U7, synthetic_origin,
};

use crate::test_support::{
    canonical_format_zero_fixture, format_one_merge_fixture, metrical_division, minimal_track,
    wrap_track, wrap_track_raw,
};
use crate::writer::{MAX_SMF_VLQ, checked_chunk_len, checked_payload_len};
use crate::{
    SmfDivision, SmfError, SmfFile, SmfFormat, SmfTimeSemantics, SmfTrack, SmfWriteOptions,
    SmpteRate, decode_vlq, encode_vlq, read_smf, write_smf_with_options,
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
            division: metrical_division(),
            tracks,
        };
        let bytes = write_smf_with_options(&file, SmfWriteOptions::default()).unwrap();
        let decoded = read_smf(&bytes).unwrap();
        assert_eq!(decoded.format, format);
        assert_eq!(decoded.division, metrical_division());
        assert_eq!(decoded.tracks.len(), file.tracks.len());
        assert_eq!(
            decoded.format.time_semantics(),
            if format == SmfFormat::Independent {
                SmfTimeSemantics::IndependentPatterns
            } else {
                SmfTimeSemantics::SharedTimeline
            }
        );
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
    let merged = file.merged_events().unwrap();
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
        division: metrical_division(),
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

    let merged = file.merged_events().unwrap();
    let pairs = merged
        .iter()
        .map(|tracked| (tracked.last_track, tracked.event.time.ticks))
        .collect::<Vec<_>>();

    assert_eq!(pairs, vec![(0, 0), (1, 120), (1, 240)]);
}

#[test]
fn format_two_patterns_cannot_be_merged_onto_a_shared_timeline() {
    let file = SmfFile {
        format: SmfFormat::Independent,
        division: metrical_division(),
        tracks: vec![minimal_track(480), minimal_track(480)],
    };

    assert_eq!(
        file.merged_events().unwrap_err(),
        SmfError::IndependentPatternsCannotMerge
    );
}

#[test]
fn every_valid_smpte_division_round_trips_byte_identically() {
    for (rate, rate_byte) in [
        (SmpteRate::Fps24, 0xe8),
        (SmpteRate::Fps25, 0xe7),
        (SmpteRate::Fps29Drop, 0xe3),
        (SmpteRate::Fps30, 0xe2),
    ] {
        let raw_division = u16::from_be_bytes([rate_byte, 80]);
        let fixture = wrap_track_raw(
            SmfFormat::SingleTrack,
            raw_division,
            vec![vec![0x00, 0xff, 0x2f, 0x00]],
        );
        let decoded = read_smf(&fixture).unwrap();
        assert_eq!(decoded.division, SmfDivision::smpte(rate, 80).unwrap());
        assert_eq!(
            write_smf_with_options(&decoded, SmfWriteOptions::default()).unwrap(),
            fixture
        );
    }

    assert_eq!(
        SmfDivision::smpte(SmpteRate::Fps29Drop, 80)
            .unwrap()
            .ticks_per_second_ratio(),
        Some((2_400_000, 1_001))
    );
}

#[test]
fn invalid_smpte_rate_and_zero_subframes_are_rejected_at_the_division_offset() {
    for raw in [0xe600, 0xe800] {
        let fixture = wrap_track_raw(
            SmfFormat::SingleTrack,
            raw,
            vec![vec![0x00, 0xff, 0x2f, 0x00]],
        );
        assert_eq!(
            read_smf(&fixture).unwrap_err(),
            SmfError::InvalidDivision { offset: 12, raw }
        );
    }
}

#[test]
fn writer_rejects_too_many_tracks() {
    let tracks = (0..=u16::MAX)
        .map(|_| minimal_track(480))
        .collect::<Vec<_>>();
    let file = SmfFile {
        format: SmfFormat::Simultaneous,
        division: metrical_division(),
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
        division: metrical_division(),
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
        division: metrical_division(),
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
fn unknown_meta_and_valid_system_events_round_trip() {
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        division: metrical_division(),
        tracks: vec![SmfTrack {
            events: vec![
                MidiEvent {
                    time: TickTime::new(0, 480).unwrap(),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Meta(MetaEvent::Other(MetaBucket {
                        type_byte: 0x6f,
                        data: vec![0x80, 0xff, 0x01],
                    })),
                },
                MidiEvent {
                    time: TickTime::new(1, 480).unwrap(),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Raw(RawBytes {
                        status: 0xf2,
                        data: vec![0x01, 0x7f],
                    }),
                },
                MidiEvent {
                    time: TickTime::new(2, 480).unwrap(),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Raw(RawBytes {
                        status: 0xf8,
                        data: Vec::new(),
                    }),
                },
            ],
        }],
    };

    let encoded = write_smf_with_options(&file, SmfWriteOptions::default()).unwrap();
    let decoded = read_smf(&encoded).unwrap();
    assert_eq!(decoded.tracks[0].events[..3], file.tracks[0].events[..]);
    assert_eq!(
        write_smf_with_options(&decoded, SmfWriteOptions::default()).unwrap(),
        encoded
    );
}

#[test]
fn realtime_system_events_do_not_cancel_running_status() {
    let fixture = wrap_track(
        SmfFormat::SingleTrack,
        480,
        vec![vec![
            0x00, 0x90, 0x3c, 0x40, 0x01, 0xf8, 0x01, 0x3d, 0x41, 0x00, 0xff, 0x2f, 0x00,
        ]],
    );
    let decoded = read_smf(&fixture).unwrap();
    assert_eq!(
        write_smf_with_options(
            &decoded,
            SmfWriteOptions {
                running_status: true,
            },
        )
        .unwrap(),
        fixture
    );
}

#[test]
fn writer_rejects_illegal_raw_system_status_lengths_and_data_bytes() {
    for raw in [
        RawBytes {
            status: 0xf4,
            data: Vec::new(),
        },
        RawBytes {
            status: 0xf2,
            data: vec![0x01],
        },
        RawBytes {
            status: 0xf1,
            data: vec![0x80],
        },
    ] {
        let status = raw.status;
        let file = SmfFile {
            format: SmfFormat::SingleTrack,
            division: metrical_division(),
            tracks: vec![SmfTrack {
                events: vec![MidiEvent {
                    time: TickTime::new(0, 480).unwrap(),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Raw(raw),
                }],
            }],
        };
        assert!(matches!(
            write_smf_with_options(&file, SmfWriteOptions::default()),
            Err(SmfError::InvalidSystemEvent {
                status: actual,
                ..
            }) if actual == status
        ));
    }
}

#[test]
fn malformed_lengths_vlqs_running_status_and_data_bytes_fail_closed() {
    let cases = [
        (
            vec![0x00, 0xff, 0x51, 0x02, 0x07, 0xa1],
            SmfError::InvalidMetaLength {
                offset: 24,
                type_byte: 0x51,
                expected: 3,
                actual: 2,
            },
        ),
        (
            vec![0x81, 0x80, 0x80, 0x80, 0x00],
            SmfError::InvalidVlq { offset: 22 },
        ),
        (
            vec![0x00, 0x90, 0x3c, 0x80],
            SmfError::InvalidChannelData { offset: 25 },
        ),
        (
            vec![
                0x00, 0x90, 0x3c, 0x40, 0x00, 0xff, 0x01, 0x00, 0x00, 0x3d, 0x40,
            ],
            SmfError::MalformedRunningStatus { offset: 31 },
        ),
        (
            vec![0x00, 0x90, 0x3c, 0x40],
            SmfError::MissingEndOfTrack { offset: 26 },
        ),
    ];

    for (track, expected) in cases {
        let bytes = wrap_track(SmfFormat::SingleTrack, 480, vec![track]);
        assert_eq!(read_smf(&bytes).unwrap_err(), expected);
    }
}
