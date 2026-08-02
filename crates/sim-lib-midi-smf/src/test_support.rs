#![forbid(unsafe_code)]

use sim_lib_midi_core::{MetaEvent, MidiEvent, MidiPayload, TickTime, synthetic_origin};

use crate::{SmfDivision, SmfFormat, SmfTrack};

pub(crate) fn minimal_track(time_base: u32) -> SmfTrack {
    SmfTrack {
        events: vec![MidiEvent {
            time: TickTime::new(0, time_base).unwrap(),
            origin: synthetic_origin(),
            payload: MidiPayload::Meta(MetaEvent::EndOfTrack),
        }],
    }
}

pub(crate) fn metrical_division() -> SmfDivision {
    SmfDivision::metrical(480).unwrap()
}

pub(crate) fn canonical_format_zero_fixture(running_status: bool) -> Vec<u8> {
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

pub(crate) fn format_one_merge_fixture() -> Vec<u8> {
    let track0 = vec![
        0x00, 0xff, 0x51, 0x03, 0x07, 0xa1, 0x20, 0x78, 0x90, 60, 100, 0x78, 0x80, 60, 0, 0x00,
        0xff, 0x2f, 0x00,
    ];
    let track1 = vec![
        0x3c, 0x91, 67, 110, 0x78, 0x81, 67, 0, 0x00, 0xff, 0x2f, 0x00,
    ];
    wrap_track(SmfFormat::Simultaneous, 480, vec![track0, track1])
}

pub(crate) fn wrap_track(format: SmfFormat, tpq: u16, tracks: Vec<Vec<u8>>) -> Vec<u8> {
    wrap_track_raw(format, tpq, tracks)
}

pub(crate) fn wrap_track_raw(format: SmfFormat, division: u16, tracks: Vec<Vec<u8>>) -> Vec<u8> {
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
    bytes.extend_from_slice(&division.to_be_bytes());
    for track in tracks {
        bytes.extend_from_slice(b"MTrk");
        bytes.extend_from_slice(&(track.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&track);
    }
    bytes
}
