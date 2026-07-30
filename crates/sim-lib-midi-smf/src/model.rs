#![forbid(unsafe_code)]

use std::{
    cmp::Ordering,
    num::{NonZeroU8, NonZeroU16},
};

use sim_lib_midi_core::{
    ChannelMessage, MetaEvent, MidiEvent, MidiPayload, TickTime, TrackedMidiEvent, synthetic_origin,
};

use crate::SmfError;

/// The SMF header format field: how the file's tracks relate.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SmfFormat {
    /// Format 0: a single multi-channel track.
    SingleTrack,
    /// Format 1: several tracks played simultaneously.
    Simultaneous,
    /// Format 2: several independent single-track patterns.
    Independent,
}

impl SmfFormat {
    /// Returns the relationship between track-local times in this format.
    pub const fn time_semantics(self) -> SmfTimeSemantics {
        match self {
            Self::SingleTrack | Self::Simultaneous => SmfTimeSemantics::SharedTimeline,
            Self::Independent => SmfTimeSemantics::IndependentPatterns,
        }
    }
}

/// How track-local event times relate in an SMF format.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SmfTimeSemantics {
    /// All tracks share one time origin and may be merged chronologically.
    SharedTimeline,
    /// Every track is a separate pattern whose time starts at that track's
    /// origin; tracks must not be merged as though they played together.
    IndependentPatterns,
}

/// A valid SMPTE frame rate encoded by the signed high byte of an SMF
/// division.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SmpteRate {
    /// 24 frames per second (`-24`).
    Fps24,
    /// 25 frames per second (`-25`).
    Fps25,
    /// 29.97 drop-frame timecode (`-29` in the SMF header).
    Fps29Drop,
    /// 30 frames per second (`-30`).
    Fps30,
}

impl SmpteRate {
    pub(crate) const fn from_header_byte(value: u8) -> Option<Self> {
        match value as i8 {
            -24 => Some(Self::Fps24),
            -25 => Some(Self::Fps25),
            -29 => Some(Self::Fps29Drop),
            -30 => Some(Self::Fps30),
            _ => None,
        }
    }

    pub(crate) const fn header_byte(self) -> u8 {
        match self {
            Self::Fps24 => (-24_i8) as u8,
            Self::Fps25 => (-25_i8) as u8,
            Self::Fps29Drop => (-29_i8) as u8,
            Self::Fps30 => (-30_i8) as u8,
        }
    }

    /// Returns the exact frame-rate ratio as frames per second.
    pub const fn frames_per_second_ratio(self) -> (u32, u32) {
        match self {
            Self::Fps24 => (24, 1),
            Self::Fps25 => (25, 1),
            Self::Fps29Drop => (30_000, 1_001),
            Self::Fps30 => (30, 1),
        }
    }

    const fn nominal_frames_per_second(self) -> u32 {
        match self {
            Self::Fps24 => 24,
            Self::Fps25 => 25,
            Self::Fps29Drop | Self::Fps30 => 30,
        }
    }
}

/// The lossless time-division field from an SMF header.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SmfDivision {
    /// Metrical timing in ticks per quarter note.
    Metrical {
        /// Non-zero ticks per quarter note.
        ticks_per_quarter: NonZeroU16,
    },
    /// Timecode timing in subdivisions of an SMPTE frame.
    Smpte {
        /// One of the four frame rates permitted by SMF.
        frames_per_second: SmpteRate,
        /// Non-zero subdivisions per frame.
        ticks_per_frame: NonZeroU8,
    },
}

impl SmfDivision {
    /// Constructs a metrical division, returning `None` for zero or a value
    /// with the SMPTE high bit set.
    pub const fn metrical(ticks_per_quarter: u16) -> Option<Self> {
        if ticks_per_quarter >= 0x8000 {
            return None;
        }
        match NonZeroU16::new(ticks_per_quarter) {
            Some(ticks_per_quarter) => Some(Self::Metrical { ticks_per_quarter }),
            None => None,
        }
    }

    /// Constructs an SMPTE division, returning `None` for zero ticks per frame.
    pub const fn smpte(frames_per_second: SmpteRate, ticks_per_frame: u8) -> Option<Self> {
        match NonZeroU8::new(ticks_per_frame) {
            Some(ticks_per_frame) => Some(Self::Smpte {
                frames_per_second,
                ticks_per_frame,
            }),
            None => None,
        }
    }

    /// Returns metrical ticks per quarter note, or `None` for SMPTE timing.
    pub const fn ticks_per_quarter(self) -> Option<NonZeroU16> {
        match self {
            Self::Metrical { ticks_per_quarter } => Some(ticks_per_quarter),
            Self::Smpte { .. } => None,
        }
    }

    /// Returns the exact number of SMF ticks per second as a ratio, or `None`
    /// for metrical timing whose tempo is carried by events.
    pub const fn ticks_per_second_ratio(self) -> Option<(u32, u32)> {
        match self {
            Self::Metrical { .. } => None,
            Self::Smpte {
                frames_per_second,
                ticks_per_frame,
            } => {
                let (numerator, denominator) = frames_per_second.frames_per_second_ratio();
                Some((numerator * ticks_per_frame.get() as u32, denominator))
            }
        }
    }

    /// Returns the resolution used in each event's [`TickTime`].
    ///
    /// For metrical files this is ticks per quarter note. For SMPTE files it
    /// is nominal ticks per second (30 fps for the `-29` drop-frame code);
    /// [`ticks_per_second_ratio`](Self::ticks_per_second_ratio) retains the
    /// exact `30_000 / 1_001` rate for duration calculations.
    pub const fn event_time_base(self) -> u32 {
        match self {
            Self::Metrical { ticks_per_quarter } => ticks_per_quarter.get() as u32,
            Self::Smpte {
                frames_per_second,
                ticks_per_frame,
            } => frames_per_second.nominal_frames_per_second() * ticks_per_frame.get() as u32,
        }
    }

    pub(crate) const fn header_word(self) -> u16 {
        match self {
            Self::Metrical { ticks_per_quarter } => ticks_per_quarter.get(),
            Self::Smpte {
                frames_per_second,
                ticks_per_frame,
            } => u16::from_be_bytes([frames_per_second.header_byte(), ticks_per_frame.get()]),
        }
    }
}

/// One track: an ordered list of timestamped events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmfTrack {
    /// Events in this track, in absolute time order after canonicalisation.
    pub events: Vec<MidiEvent>,
}

/// A parsed Standard MIDI File: its format, time division, and tracks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmfFile {
    /// Header format field.
    pub format: SmfFormat,
    /// Lossless metrical or SMPTE time division.
    pub division: SmfDivision,
    /// Tracks in file order.
    pub tracks: Vec<SmfTrack>,
}

/// Options controlling SMF serialisation.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct SmfWriteOptions {
    /// When set, omit a redundant status byte for consecutive channel messages
    /// (MIDI running status).
    pub running_status: bool,
}

/// An iterator that merges all tracks of an [`SmfFile`] into a single
/// time-ordered, track-tagged stream.
///
/// Created by [`SmfFile::merge_cursor`]; yields
/// [`TrackedMidiEvent`](sim_lib_midi_core::TrackedMidiEvent)s.
pub struct SmfMergeCursor<'a> {
    file: &'a SmfFile,
    next_index: Vec<usize>,
}

impl SmfFile {
    /// Returns this file's metrical ticks per quarter note, or `None` when its
    /// division is SMPTE.
    pub const fn ticks_per_quarter(&self) -> Option<u32> {
        match self.division.ticks_per_quarter() {
            Some(value) => Some(value.get() as u32),
            None => None,
        }
    }

    /// Sorts every track into canonical order and ensures each ends with an
    /// end-of-track meta event.
    pub fn canonicalize(&mut self) {
        for track in &mut self.tracks {
            canonicalize_track(track, self.division.event_time_base());
        }
    }

    /// Returns a cursor that merges tracks sharing one timeline.
    ///
    /// Format-2 tracks are independent patterns, so merging them would invent
    /// a relationship between their local time origins and is rejected.
    pub fn merge_cursor(&self) -> Result<SmfMergeCursor<'_>, SmfError> {
        if self.format.time_semantics() == SmfTimeSemantics::IndependentPatterns {
            return Err(SmfError::IndependentPatternsCannotMerge);
        }
        Ok(SmfMergeCursor {
            file: self,
            next_index: vec![0; self.tracks.len()],
        })
    }

    /// Collects every track's events into a single time-ordered,
    /// track-tagged vector.
    ///
    /// Format-2 files return [`SmfError::IndependentPatternsCannotMerge`];
    /// callers must choose and process a track as an independent pattern.
    pub fn merged_events(&self) -> Result<Vec<TrackedMidiEvent>, SmfError> {
        let mut merged = Vec::new();
        for event in self.merge_cursor()? {
            merged.push(event);
        }
        Ok(merged)
    }
}

impl<'a> Iterator for SmfMergeCursor<'a> {
    type Item = TrackedMidiEvent;

    fn next(&mut self) -> Option<Self::Item> {
        let mut best: Option<(usize, &MidiEvent)> = None;
        for (track_idx, track) in self.file.tracks.iter().enumerate() {
            let Some(event) = track.events.get(self.next_index[track_idx]) else {
                continue;
            };
            match best {
                None => best = Some((track_idx, event)),
                Some((best_track, best_event)) => {
                    if compare_event_order(event, track_idx, best_event, best_track)
                        == Ordering::Less
                    {
                        best = Some((track_idx, event));
                    }
                }
            }
        }
        let (track_idx, event) = best?;
        self.next_index[track_idx] += 1;
        Some(TrackedMidiEvent {
            last_track: track_idx,
            event: event.clone(),
        })
    }
}

pub(crate) fn canonicalize_track(track: &mut SmfTrack, tpq: u32) {
    track.events.sort_by(compare_events_same_track);
    if !track
        .events
        .iter()
        .any(|event| matches!(event.payload, MidiPayload::Meta(MetaEvent::EndOfTrack)))
    {
        let last_ticks = track
            .events
            .last()
            .map(|event| event.time.ticks)
            .unwrap_or(0);
        track.events.push(MidiEvent {
            time: TickTime::new(last_ticks, tpq).unwrap_or(TickTime::ZERO),
            origin: synthetic_origin(),
            payload: MidiPayload::Meta(MetaEvent::EndOfTrack),
        });
    }
    track.events.sort_by(compare_events_same_track);
}

pub(crate) fn compare_event_order(
    left: &MidiEvent,
    left_track: usize,
    right: &MidiEvent,
    right_track: usize,
) -> Ordering {
    compare_time(left.time, right.time)
        .then_with(|| event_priority(left).cmp(&event_priority(right)))
        .then_with(|| left_track.cmp(&right_track))
}

fn compare_events_same_track(left: &MidiEvent, right: &MidiEvent) -> Ordering {
    compare_time(left.time, right.time)
        .then_with(|| event_priority(left).cmp(&event_priority(right)))
}

fn compare_time(left: TickTime, right: TickTime) -> Ordering {
    let left_scaled = i128::from(left.ticks) * i128::from(right.tpq);
    let right_scaled = i128::from(right.ticks) * i128::from(left.tpq);
    left_scaled.cmp(&right_scaled)
}

fn event_priority(event: &MidiEvent) -> u8 {
    match event.payload {
        MidiPayload::Meta(MetaEvent::EndOfTrack) => 4,
        MidiPayload::Meta(_) => 0,
        MidiPayload::Channel(ChannelMessage::NoteOff { .. }) => 1,
        MidiPayload::Channel(ChannelMessage::NoteOn { .. }) => 2,
        _ => 3,
    }
}
