//! Typed views over raw [`MetaBucket`] payloads.
//!
//! These helpers interpret the common Standard MIDI File meta types (text,
//! track name, marker, SMPTE offset) and build the matching buckets, without
//! the core model needing a variant per meta type.

use crate::{MetaBucket, SmpteOffset};

/// Returns the bucket's UTF-8 text if it is a text meta event (type `0x01`).
pub fn as_text(bucket: &MetaBucket) -> Option<&str> {
    (bucket.type_byte == 0x01)
        .then(|| std::str::from_utf8(&bucket.data).ok())
        .flatten()
}

/// Returns the bucket's UTF-8 text if it is a track-name meta event (type
/// `0x03`).
pub fn as_track_name(bucket: &MetaBucket) -> Option<&str> {
    (bucket.type_byte == 0x03)
        .then(|| std::str::from_utf8(&bucket.data).ok())
        .flatten()
}

/// Returns the bucket's UTF-8 text if it is a marker meta event (type `0x06`).
pub fn as_marker(bucket: &MetaBucket) -> Option<&str> {
    (bucket.type_byte == 0x06)
        .then(|| std::str::from_utf8(&bucket.data).ok())
        .flatten()
}

/// Returns the [`SmpteOffset`] if the bucket is a 5-byte SMPTE-offset meta
/// event (type `0x54`).
pub fn as_smpte_offset(bucket: &MetaBucket) -> Option<SmpteOffset> {
    (bucket.type_byte == 0x54 && bucket.data.len() == 5).then(|| SmpteOffset {
        hours: bucket.data[0],
        minutes: bucket.data[1],
        seconds: bucket.data[2],
        frames: bucket.data[3],
        subframes: bucket.data[4],
    })
}

/// Builds a text meta bucket (type `0x01`) from `value`.
pub fn make_text(value: &str) -> MetaBucket {
    MetaBucket {
        type_byte: 0x01,
        data: value.as_bytes().to_vec(),
    }
}

/// Builds a track-name meta bucket (type `0x03`) from `value`.
pub fn make_track_name(value: &str) -> MetaBucket {
    MetaBucket {
        type_byte: 0x03,
        data: value.as_bytes().to_vec(),
    }
}

/// Builds a marker meta bucket (type `0x06`) from `value`.
pub fn make_marker(value: &str) -> MetaBucket {
    MetaBucket {
        type_byte: 0x06,
        data: value.as_bytes().to_vec(),
    }
}

/// Builds a SMPTE-offset meta bucket (type `0x54`) from `offset`.
pub fn make_smpte_offset(offset: SmpteOffset) -> MetaBucket {
    MetaBucket {
        type_byte: 0x54,
        data: vec![
            offset.hours,
            offset.minutes,
            offset.seconds,
            offset.frames,
            offset.subframes,
        ],
    }
}
