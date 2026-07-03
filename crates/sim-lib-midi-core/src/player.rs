use std::convert::TryFrom;

use crate::{Channel, ChannelMessage, MidiEvent, MidiPayload, TickTime, U7, synthetic_origin};

/// Controls whether echoed notes are snapped to a scale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoteEchoScaleSnap {
    /// No snapping; the drifted key is used as-is.
    Off,
    /// Snap to the nearest key whose pitch class is in this set (each value is
    /// reduced modulo 12).
    PitchClasses(Vec<u8>),
}

impl NoteEchoScaleSnap {
    /// Builds a major-scale snap for the given `tonic` pitch class.
    pub fn major(tonic: u8) -> Self {
        Self::PitchClasses(
            [0, 2, 4, 5, 7, 9, 11]
                .into_iter()
                .map(|class| (class + tonic) % 12)
                .collect(),
        )
    }

    fn snap(&self, key: i16) -> Option<u8> {
        if !(0..=127).contains(&key) {
            return None;
        }
        match self {
            Self::Off => Some(key as u8),
            Self::PitchClasses(classes) => nearest_key_in_classes(key, classes),
        }
    }
}

/// Controls which channel each echoed note is emitted on.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NoteEchoChannelPolicy {
    /// Keep the source note's channel.
    Preserve,
    /// Force every echo onto a fixed channel.
    Fixed(Channel),
    /// Shift the channel by `offset` per repeat, wrapping within `0..=15`.
    Offset(i8),
}

impl NoteEchoChannelPolicy {
    fn apply(self, channel: Channel, repeat: u8) -> Channel {
        match self {
            Self::Preserve => channel,
            Self::Fixed(channel) => channel,
            Self::Offset(offset) => Channel(
                (i16::from(channel.0) + i16::from(offset) * i16::from(repeat)).rem_euclid(16) as u8,
            ),
        }
    }
}

/// Configuration for the [`NoteEchoPlayer`] echo/delay transform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteEchoConfig {
    /// Number of echo repeats to emit per source note.
    pub repeats: u8,
    /// Cap on the number of repeats actually rendered (`0` means no cap).
    pub feedback_count: u8,
    /// Time delay added per repeat.
    pub time_offset: TickTime,
    /// Note length used when no matching note-off is found in the input.
    pub fallback_gate: TickTime,
    /// Velocity reduction applied per repeat.
    pub velocity_decay: u8,
    /// Semitone pitch shift applied per repeat before snapping.
    pub pitch_offset: i8,
    /// Scale-snapping policy for shifted pitches.
    pub scale_snap: NoteEchoScaleSnap,
    /// Channel-assignment policy for echoes.
    pub channel_policy: NoteEchoChannelPolicy,
    /// Whether the original input events are included in the render.
    pub include_source: bool,
}

impl NoteEchoConfig {
    /// Creates a default single-repeat config delayed by `time_offset`.
    pub fn new(time_offset: TickTime) -> Self {
        Self {
            repeats: 1,
            feedback_count: 1,
            time_offset,
            fallback_gate: time_offset,
            velocity_decay: 0,
            pitch_offset: 0,
            scale_snap: NoteEchoScaleSnap::Off,
            channel_policy: NoteEchoChannelPolicy::Preserve,
            include_source: true,
        }
    }

    /// Returns the effective number of repeats after applying the
    /// [`feedback_count`](Self::feedback_count) cap.
    pub fn repeat_count(&self) -> u8 {
        if self.feedback_count == 0 {
            self.repeats
        } else {
            self.repeats.min(self.feedback_count)
        }
    }
}

/// A record of a single emitted echo, linking it back to its source note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteEchoTrace {
    /// Index of the source note in the input slice.
    pub source_index: usize,
    /// Which repeat (1-based) produced this echo.
    pub repeat: u8,
    /// Time of the echoed note-on.
    pub time: TickTime,
    /// Echoed note number.
    pub key: U7,
    /// Echoed velocity.
    pub velocity: U7,
    /// Echoed channel.
    pub channel: Channel,
}

/// The result of a [`NoteEchoPlayer`] render: emitted events plus per-echo
/// traces.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NoteEchoRender {
    /// Rendered events in stable time order.
    pub events: Vec<MidiEvent>,
    /// One trace per emitted echo.
    pub traces: Vec<NoteEchoTrace>,
}

/// Renders rhythmic note echoes from an input event stream per a
/// [`NoteEchoConfig`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteEchoPlayer {
    /// The echo configuration driving this player.
    pub config: NoteEchoConfig,
}

impl NoteEchoPlayer {
    /// Creates a player from `config`.
    pub fn new(config: NoteEchoConfig) -> Self {
        Self { config }
    }

    /// Renders echoes for `input`, returning the emitted events and traces.
    pub fn render(&self, input: &[MidiEvent]) -> NoteEchoRender {
        let mut render = NoteEchoRender::default();
        if self.config.include_source {
            render.events.extend(input.iter().cloned());
        }

        for (source_index, event) in input.iter().enumerate() {
            let Some((channel, key, velocity)) = note_on(event) else {
                continue;
            };
            let duration =
                note_duration(input, source_index, channel, key, self.config.fallback_gate);
            for repeat in 1..=self.config.repeat_count() {
                let Some((echo_key, echo_velocity, echo_channel)) =
                    self.echo_values(key, velocity, channel, repeat)
                else {
                    continue;
                };
                let time = event.time + self.config.time_offset.mul_int(i64::from(repeat));
                let off_time = time + duration;
                render
                    .events
                    .push(note_on_event(time, echo_channel, echo_key, echo_velocity));
                render
                    .events
                    .push(note_off_event(off_time, echo_channel, echo_key));
                render.traces.push(NoteEchoTrace {
                    source_index,
                    repeat,
                    time,
                    key: echo_key,
                    velocity: echo_velocity,
                    channel: echo_channel,
                });
            }
        }
        stable_midi_event_order(&mut render.events);
        render.traces.sort_by(|left, right| {
            left.time
                .cmp(&right.time)
                .then_with(|| left.source_index.cmp(&right.source_index))
                .then_with(|| left.repeat.cmp(&right.repeat))
        });
        render
    }

    /// Renders `input` and returns the result; an alias for
    /// [`render`](Self::render) reading as a commit-to-output step.
    pub fn freeze(&self, input: &[MidiEvent]) -> NoteEchoRender {
        self.render(input)
    }

    fn echo_values(
        &self,
        key: U7,
        velocity: U7,
        channel: Channel,
        repeat: u8,
    ) -> Option<(U7, U7, Channel)> {
        let drifted_key =
            i16::from(key.0) + i16::from(self.config.pitch_offset) * i16::from(repeat);
        let snapped_key = self.config.scale_snap.snap(drifted_key)?;
        let decayed = velocity
            .0
            .saturating_sub(self.config.velocity_decay.saturating_mul(repeat))
            .max(1);
        Some((
            U7::try_from(u16::from(snapped_key)).ok()?,
            U7(decayed),
            self.config.channel_policy.apply(channel, repeat),
        ))
    }
}

fn note_on(event: &MidiEvent) -> Option<(Channel, U7, U7)> {
    match event.payload {
        MidiPayload::Channel(ChannelMessage::NoteOn { ch, key, vel }) if vel.0 > 0 => {
            Some((ch, key, vel))
        }
        _ => None,
    }
}

fn is_note_off(event: &MidiEvent, channel: Channel, key: U7) -> bool {
    matches!(
        event.payload,
        MidiPayload::Channel(ChannelMessage::NoteOff { ch, key: off_key, .. })
            if ch == channel && off_key == key
    ) || matches!(
        event.payload,
        MidiPayload::Channel(ChannelMessage::NoteOn { ch, key: off_key, vel })
            if ch == channel && off_key == key && vel.0 == 0
    )
}

fn note_duration(
    input: &[MidiEvent],
    source_index: usize,
    channel: Channel,
    key: U7,
    fallback: TickTime,
) -> TickTime {
    let start = input[source_index].time;
    input
        .iter()
        .skip(source_index + 1)
        .find(|event| event.time > start && is_note_off(event, channel, key))
        .map(|event| event.time - start)
        .unwrap_or(fallback)
}

fn nearest_key_in_classes(key: i16, classes: &[u8]) -> Option<u8> {
    if classes.is_empty() {
        return Some(key as u8);
    }
    (-12..=12)
        .filter_map(|delta| {
            let candidate = key + delta;
            if !(0..=127).contains(&candidate) {
                return None;
            }
            let class = candidate.rem_euclid(12) as u8;
            classes
                .iter()
                .any(|allowed| allowed % 12 == class)
                .then_some((delta.abs(), candidate as u8))
        })
        .min_by_key(|(distance, candidate)| (*distance, *candidate))
        .map(|(_, candidate)| candidate)
}

fn note_on_event(time: TickTime, ch: Channel, key: U7, vel: U7) -> MidiEvent {
    MidiEvent {
        time,
        origin: synthetic_origin(),
        payload: MidiPayload::Channel(ChannelMessage::NoteOn { ch, key, vel }),
    }
}

fn note_off_event(time: TickTime, ch: Channel, key: U7) -> MidiEvent {
    MidiEvent {
        time,
        origin: synthetic_origin(),
        payload: MidiPayload::Channel(ChannelMessage::NoteOff {
            ch,
            key,
            vel: U7(0),
        }),
    }
}

/// Sorts events into a stable, deterministic order by time and then by a
/// payload tiebreak (note-offs before note-ons at the same tick).
pub fn stable_midi_event_order(events: &mut [MidiEvent]) {
    events.sort_by(|left, right| {
        left.time
            .cmp(&right.time)
            .then_with(|| event_sort_key(left).cmp(&event_sort_key(right)))
    });
}

fn event_sort_key(event: &MidiEvent) -> (u8, u8, u8, u8) {
    match event.payload {
        MidiPayload::Channel(ChannelMessage::NoteOff { ch, key, vel }) => (0, ch.0, key.0, vel.0),
        MidiPayload::Channel(ChannelMessage::NoteOn { ch, key, vel }) => (1, ch.0, key.0, vel.0),
        MidiPayload::Channel(_) => (2, 0, 0, 0),
        MidiPayload::Meta(_) => (3, 0, 0, 0),
        MidiPayload::SysEx(_) => (4, 0, 0, 0),
        MidiPayload::Raw(_) => (5, 0, 0, 0),
    }
}
