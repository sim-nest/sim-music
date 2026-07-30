//! Additional SMF conformance: configured resource ceilings and complete
//! channel-message coverage.

use sim_lib_midi_core::{
    Channel, ChannelMessage, MidiEvent, MidiPayload, TickTime, U7, U14, synthetic_origin,
};

use crate::{
    SmfError, SmfFile, SmfFormat, SmfLimitKind, SmfReadLimits, SmfTrack, SmfWriteOptions, read_smf,
    read_smf_with_limits,
    test_support::{metrical_division, wrap_track},
    write_smf_with_options,
};

#[test]
fn reader_checks_counts_lengths_and_payload_budgets_before_copying() {
    let two_tracks = wrap_track(
        SmfFormat::Simultaneous,
        480,
        vec![vec![0x00, 0xff, 0x2f, 0x00], vec![0x00, 0xff, 0x2f, 0x00]],
    );
    assert_eq!(
        read_smf_with_limits(
            &two_tracks,
            SmfReadLimits {
                max_tracks: 1,
                ..SmfReadLimits::default()
            }
        )
        .unwrap_err(),
        SmfError::LimitExceeded {
            offset: 10,
            kind: SmfLimitKind::TrackCount,
            actual: 2,
            maximum: 1,
        }
    );

    let one_track = wrap_track(
        SmfFormat::SingleTrack,
        480,
        vec![vec![0x00, 0xff, 0x2f, 0x00]],
    );
    assert_eq!(
        read_smf_with_limits(
            &one_track,
            SmfReadLimits {
                max_track_bytes: 3,
                ..SmfReadLimits::default()
            }
        )
        .unwrap_err(),
        SmfError::LimitExceeded {
            offset: 18,
            kind: SmfLimitKind::TrackBytes,
            actual: 4,
            maximum: 3,
        }
    );
    assert_eq!(
        read_smf_with_limits(
            &one_track,
            SmfReadLimits {
                max_file_bytes: one_track.len() - 1,
                ..SmfReadLimits::default()
            }
        )
        .unwrap_err(),
        SmfError::LimitExceeded {
            offset: 0,
            kind: SmfLimitKind::FileBytes,
            actual: one_track.len(),
            maximum: one_track.len() - 1,
        }
    );

    let payload = wrap_track(
        SmfFormat::SingleTrack,
        480,
        vec![vec![0x00, 0xff, 0x7f, 0x03, 1, 2, 3]],
    );
    assert_eq!(
        read_smf_with_limits(
            &payload,
            SmfReadLimits {
                max_event_payload_bytes: 2,
                ..SmfReadLimits::default()
            }
        )
        .unwrap_err(),
        SmfError::LimitExceeded {
            offset: 25,
            kind: SmfLimitKind::EventPayloadBytes,
            actual: 3,
            maximum: 2,
        }
    );

    let events = wrap_track(
        SmfFormat::SingleTrack,
        480,
        vec![vec![0x00, 0xf8, 0x00, 0xf8, 0x00, 0xff, 0x2f, 0x00]],
    );
    assert_eq!(
        read_smf_with_limits(
            &events,
            SmfReadLimits {
                max_events: 2,
                ..SmfReadLimits::default()
            }
        )
        .unwrap_err(),
        SmfError::LimitExceeded {
            offset: 26,
            kind: SmfLimitKind::EventCount,
            actual: 3,
            maximum: 2,
        }
    );
}

#[test]
fn channel_control_pitch_and_pressure_messages_round_trip() {
    let channel = Channel::new(0).unwrap();
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        division: metrical_division(),
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
