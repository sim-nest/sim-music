use std::collections::BTreeMap;

use sim_kernel::{Error, Result, Symbol};

use crate::{
    Channel, LaneId, Music, PerformanceEvent, PerformanceInput, PerformanceIntent, PerformanceTake,
    Pitch, Tick,
};

/// Binds a named performance input to a lane and channel.
///
/// Connects an input device identifier to the [`LaneId`] its events land on and the
/// [`Channel`] they default to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerformanceInputBinding {
    /// Symbol identifying the bound input.
    pub input_id: Symbol,
    /// Lane the input's events are routed to.
    pub lane_id: LaneId,
    /// Channel assigned to the input.
    pub channel: Channel,
}

impl PerformanceInputBinding {
    /// Creates a binding from an input id, lane, and channel.
    pub fn new(input_id: Symbol, lane_id: LaneId, channel: Channel) -> Self {
        Self {
            input_id,
            lane_id,
            channel,
        }
    }
}

/// A set of allowed pitch classes that incoming notes are snapped to.
///
/// Holds the permitted pitch classes (0..12); [`apply`](Self::apply) maps any pitch
/// to the nearest allowed class.
///
/// # Examples
///
/// ```
/// use sim_lib_music_core::{Pitch, ScaleLock};
///
/// let lock = ScaleLock::major();
/// // C-sharp (class 1) is not in C major and snaps down to C.
/// assert_eq!(lock.apply(Pitch::from_semitone(1)).semitone(), 0);
/// // A pitch already in the scale is left unchanged.
/// assert_eq!(lock.apply(Pitch::from_semitone(2)).semitone(), 2);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScaleLock {
    /// Sorted, deduplicated allowed pitch classes (each in 0..12).
    pub allowed_classes: Vec<u8>,
}

impl ScaleLock {
    /// Builds a scale lock from a list of pitch classes.
    ///
    /// The list is sorted and deduplicated; returns an error if it is empty or any
    /// class is outside 0..12.
    pub fn new(mut allowed_classes: Vec<u8>) -> Result<Self> {
        if allowed_classes.is_empty() || allowed_classes.iter().any(|class| *class >= 12) {
            return Err(Error::Eval(
                "scale lock pitch classes must be in 0..12".to_owned(),
            ));
        }
        allowed_classes.sort_unstable();
        allowed_classes.dedup();
        Ok(Self { allowed_classes })
    }

    /// Returns a scale lock for the diatonic major scale.
    pub fn major() -> Self {
        Self::new(vec![0, 2, 4, 5, 7, 9, 11]).expect("major scale lock is valid")
    }

    /// Snaps `pitch` to the nearest allowed pitch class.
    ///
    /// Pitches already in the lock pass through unchanged; otherwise the smallest
    /// semitone shift to an allowed class is applied, preferring downward ties.
    pub fn apply(&self, pitch: Pitch) -> Pitch {
        let semitone = pitch.semitone();
        let class = semitone.rem_euclid(12) as u8;
        if self.allowed_classes.binary_search(&class).is_ok() {
            return pitch;
        }
        let delta = (-6..=6)
            .filter(|delta| {
                let candidate = (class as i32 + delta).rem_euclid(12) as u8;
                self.allowed_classes.binary_search(&candidate).is_ok()
            })
            .min_by_key(|delta| (delta.abs(), (*delta > 0) as u8))
            .expect("scale lock has at least one class");
        Pitch::from_semitone(semitone + delta)
    }
}

/// Identity key for a held note, combining channel and pitch.
///
/// Used to track note-on/note-off pairing in source and clip state.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PerformanceNoteKey {
    /// Raw channel number.
    pub channel: u8,
    /// Pitch in semitones.
    pub semitone: i32,
}

impl PerformanceNoteKey {
    /// Builds a key from a channel and pitch.
    pub fn new(channel: Channel, pitch: Pitch) -> Self {
        Self {
            channel: channel.0,
            semitone: pitch.semitone(),
        }
    }
}

/// A note currently held down at a source.
///
/// Records the active note's pitch, velocity, channel, start tick, and whether it
/// was released while the sustain pedal was down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldPerformanceNote {
    /// Pitch of the held note.
    pub pitch: Pitch,
    /// Attack velocity.
    pub velocity: u8,
    /// Channel the note plays on.
    pub channel: Channel,
    /// Tick at which the note started.
    pub started_at: Tick,
    /// Whether note-off arrived while sustain was held.
    pub released_while_sustained: bool,
}

/// Mutable performance state tracked by a source.
///
/// Holds the currently sounding notes plus the transforms applied to incoming
/// gestures: sustain pedal, octave shift, transpose, and optional scale lock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerformanceSourceState {
    /// Notes currently held, keyed by channel and pitch.
    pub held_notes: BTreeMap<PerformanceNoteKey, HeldPerformanceNote>,
    /// Whether the sustain pedal is down.
    pub sustain_pedal: bool,
    /// Octave shift applied to incoming pitches.
    pub octave_shift: i8,
    /// Semitone transpose applied to incoming pitches.
    pub transpose: i8,
    /// Optional scale lock snapping incoming pitches.
    pub scale_lock: Option<ScaleLock>,
    /// Default channel for the source.
    pub channel: Channel,
}

impl PerformanceSourceState {
    /// Creates empty state defaulting to `channel`.
    pub fn new(channel: Channel) -> Self {
        Self {
            held_notes: BTreeMap::new(),
            sustain_pedal: false,
            octave_shift: 0,
            transpose: 0,
            scale_lock: None,
            channel,
        }
    }

    /// Returns the number of notes currently held.
    pub fn held_note_count(&self) -> usize {
        self.held_notes.len()
    }

    fn transform_pitch(&self, pitch: Pitch) -> Pitch {
        let transposed =
            pitch.transpose(i32::from(self.transpose) + i32::from(self.octave_shift) * 12);
        self.scale_lock
            .as_ref()
            .map(|lock| lock.apply(transposed))
            .unwrap_or(transposed)
    }

    fn observe_event(&mut self, event: &PerformanceEvent) {
        match &event.intent {
            PerformanceIntent::NoteOn {
                pitch,
                velocity,
                channel,
            } => {
                self.held_notes.insert(
                    PerformanceNoteKey::new(*channel, *pitch),
                    HeldPerformanceNote {
                        pitch: *pitch,
                        velocity: *velocity,
                        channel: *channel,
                        started_at: event.time,
                        released_while_sustained: false,
                    },
                );
            }
            PerformanceIntent::NoteOff { pitch, channel, .. } => {
                let key = PerformanceNoteKey::new(*channel, *pitch);
                if self.sustain_pedal {
                    if let Some(note) = self.held_notes.get_mut(&key) {
                        note.released_while_sustained = true;
                    }
                } else {
                    self.held_notes.remove(&key);
                }
            }
            PerformanceIntent::Sustain { down, .. } => {
                self.sustain_pedal = *down;
                if !down {
                    self.held_notes
                        .retain(|_, note| !note.released_while_sustained);
                }
            }
            PerformanceIntent::Panic => {
                self.held_notes.clear();
                self.sustain_pedal = false;
            }
            PerformanceIntent::Aftertouch { .. }
            | PerformanceIntent::PitchBend { .. }
            | PerformanceIntent::Parameter { .. } => {}
        }
    }
}

/// A live performance source that turns inputs into events and captures takes.
///
/// Implementors bind an input, poll queued [`PerformanceInput`]s into transformed
/// [`PerformanceEvent`](crate::PerformanceEvent)s, emit panics, record capture takes,
/// and render a captured [`PerformanceTake`] into [`Music`](crate::Music).
pub trait PerformanceSource {
    /// Binds an input to this source, setting its lane and channel.
    fn bind_input(&mut self, binding: PerformanceInputBinding) -> Result<()>;
    /// Transforms queued inputs into events, updating held-note state.
    fn poll_events(&mut self, inputs: Vec<PerformanceInput>) -> Result<Vec<PerformanceEvent>>;
    /// Emits note-offs for all held notes followed by a panic event.
    fn panic(&mut self, input_time: Tick) -> Result<Vec<PerformanceEvent>>;
    /// Begins capturing emitted events under `take_id`.
    fn capture_start(&mut self, take_id: Symbol) -> Result<()>;
    /// Ends capture and returns the recorded [`PerformanceTake`].
    fn capture_stop(&mut self) -> Result<PerformanceTake>;
    /// Renders a captured take into a [`Music`](crate::Music) clip.
    fn as_clip(&self, take: &PerformanceTake) -> Result<Music>;
}

/// An in-memory [`PerformanceSource`] holding state and an optional capture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryPerformanceSource {
    source_id: Symbol,
    binding: Option<PerformanceInputBinding>,
    state: PerformanceSourceState,
    capture: Option<PerformanceCapture>,
}

impl MemoryPerformanceSource {
    /// Creates an unbound source with the given id and default channel.
    pub fn new(source_id: Symbol, channel: Channel) -> Self {
        Self {
            source_id,
            binding: None,
            state: PerformanceSourceState::new(channel),
            capture: None,
        }
    }

    /// Returns the symbol identifying this source.
    pub fn source_id(&self) -> &Symbol {
        &self.source_id
    }

    /// Returns a shared reference to the source's state.
    pub fn state(&self) -> &PerformanceSourceState {
        &self.state
    }

    /// Returns a mutable reference to the source's state.
    pub fn state_mut(&mut self) -> &mut PerformanceSourceState {
        &mut self.state
    }

    /// Sets the octave shift applied to incoming pitches.
    pub fn set_octave_shift(&mut self, octave_shift: i8) {
        self.state.octave_shift = octave_shift;
    }

    /// Sets the semitone transpose applied to incoming pitches.
    pub fn set_transpose(&mut self, transpose: i8) {
        self.state.transpose = transpose;
    }

    /// Sets or clears the scale lock applied to incoming pitches.
    pub fn set_scale_lock(&mut self, scale_lock: Option<ScaleLock>) {
        self.state.scale_lock = scale_lock;
    }

    fn binding(&self) -> Result<&PerformanceInputBinding> {
        self.binding
            .as_ref()
            .ok_or_else(|| Error::Eval("performance source input is not bound".to_owned()))
    }

    fn push_capture(&mut self, events: &[PerformanceEvent]) {
        if let Some(capture) = &mut self.capture {
            capture.events.extend_from_slice(events);
        }
    }
}

impl PerformanceSource for MemoryPerformanceSource {
    fn bind_input(&mut self, binding: PerformanceInputBinding) -> Result<()> {
        self.state.channel = binding.channel;
        self.binding = Some(binding);
        Ok(())
    }

    fn poll_events(&mut self, inputs: Vec<PerformanceInput>) -> Result<Vec<PerformanceEvent>> {
        let binding = self.binding()?.clone();
        let mut events = Vec::new();
        for input in inputs {
            let event = PerformanceEvent {
                lane_id: binding.lane_id.clone(),
                source_id: self.source_id.clone(),
                input_time: input.input_time,
                time: input.input_time,
                intent: transform_intent(input.intent, &self.state),
            };
            self.state.observe_event(&event);
            events.push(event);
        }
        self.push_capture(&events);
        Ok(events)
    }

    fn panic(&mut self, input_time: Tick) -> Result<Vec<PerformanceEvent>> {
        let binding = self.binding()?.clone();
        let mut events = self
            .state
            .held_notes
            .values()
            .map(|note| PerformanceEvent {
                lane_id: binding.lane_id.clone(),
                source_id: self.source_id.clone(),
                input_time,
                time: input_time,
                intent: PerformanceIntent::NoteOff {
                    pitch: note.pitch,
                    velocity: 0,
                    channel: note.channel,
                },
            })
            .collect::<Vec<_>>();
        events.push(PerformanceEvent {
            lane_id: binding.lane_id,
            source_id: self.source_id.clone(),
            input_time,
            time: input_time,
            intent: PerformanceIntent::Panic,
        });
        for event in &events {
            self.state.observe_event(event);
        }
        self.push_capture(&events);
        Ok(events)
    }

    fn capture_start(&mut self, take_id: Symbol) -> Result<()> {
        if self.capture.is_some() {
            return Err(Error::Eval(
                "performance capture is already active".to_owned(),
            ));
        }
        self.capture = Some(PerformanceCapture {
            take_id,
            events: Vec::new(),
        });
        Ok(())
    }

    fn capture_stop(&mut self) -> Result<PerformanceTake> {
        let capture = self
            .capture
            .take()
            .ok_or_else(|| Error::Eval("performance capture is not active".to_owned()))?;
        PerformanceTake::new(self.source_id.clone(), capture.take_id, capture.events)
    }

    fn as_clip(&self, take: &PerformanceTake) -> Result<Music> {
        take.as_clip()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PerformanceCapture {
    take_id: Symbol,
    events: Vec<PerformanceEvent>,
}

fn transform_intent(
    intent: PerformanceIntent,
    state: &PerformanceSourceState,
) -> PerformanceIntent {
    match intent {
        PerformanceIntent::NoteOn {
            pitch,
            velocity,
            channel,
        } => PerformanceIntent::NoteOn {
            pitch: state.transform_pitch(pitch),
            velocity,
            channel,
        },
        PerformanceIntent::NoteOff {
            pitch,
            velocity,
            channel,
        } => PerformanceIntent::NoteOff {
            pitch: state.transform_pitch(pitch),
            velocity,
            channel,
        },
        PerformanceIntent::Aftertouch {
            pitch,
            pressure,
            channel,
        } => PerformanceIntent::Aftertouch {
            pitch: state.transform_pitch(pitch),
            pressure,
            channel,
        },
        other => other,
    }
}
