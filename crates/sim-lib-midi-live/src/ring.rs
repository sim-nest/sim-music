use std::{collections::VecDeque, convert::Infallible};

use sim_lib_midi_core::{MidiEvent, MidiSink, MidiSource, TrackedMidiEvent, TrackedMidiSource};

use crate::LiveMidiError;

/// A fixed-capacity ring buffer of [`MidiEvent`]s usable as both a
/// [`MidiSource`] and a [`MidiSink`].
///
/// Writing to a full buffer evicts the oldest event and increments the dropped
/// count; reading pops from the front.
#[derive(Clone, Debug)]
pub struct RingMidiBuffer {
    tpq: u32,
    capacity: usize,
    dropped_events: usize,
    events: VecDeque<MidiEvent>,
}

impl RingMidiBuffer {
    /// Creates a buffer at `tpq` resolution holding up to `capacity` events.
    ///
    /// Fails with [`LiveMidiError::ZeroCapacity`] when `capacity` is zero.
    pub fn new(tpq: u32, capacity: usize) -> Result<Self, LiveMidiError> {
        if capacity == 0 {
            return Err(LiveMidiError::ZeroCapacity);
        }
        Ok(Self {
            tpq,
            capacity,
            dropped_events: 0,
            events: VecDeque::with_capacity(capacity),
        })
    }

    /// Returns the buffer's maximum capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the number of buffered events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns the running count of events dropped due to overflow.
    pub fn dropped_events(&self) -> usize {
        self.dropped_events
    }

    /// Removes all buffered events (the dropped count is preserved).
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Returns a copy of the currently buffered events without consuming them.
    pub fn snapshot(&self) -> Vec<MidiEvent> {
        self.events.iter().cloned().collect()
    }

    fn push_back(&mut self, event: MidiEvent) {
        if self.events.len() == self.capacity {
            let _ = self.events.pop_front();
            self.dropped_events += 1;
        }
        self.events.push_back(event);
    }
}

impl MidiSource for RingMidiBuffer {
    type Err = Infallible;

    fn tpq(&self) -> u32 {
        self.tpq
    }

    fn next(&mut self) -> Result<Option<MidiEvent>, Self::Err> {
        Ok(self.events.pop_front())
    }
}

impl MidiSink for RingMidiBuffer {
    type Err = Infallible;

    fn tpq(&self) -> u32 {
        self.tpq
    }

    fn write(&mut self, event: &MidiEvent) -> Result<(), Self::Err> {
        self.push_back(event.clone());
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Err> {
        Ok(())
    }
}

/// A fixed-capacity ring buffer of [`TrackedMidiEvent`]s usable as a
/// [`TrackedMidiSource`], tracking the highest track index seen.
#[derive(Clone, Debug)]
pub struct RingTrackedMidiBuffer {
    tpq: u32,
    capacity: usize,
    dropped_events: usize,
    last_track: usize,
    n_tracks: usize,
    events: VecDeque<TrackedMidiEvent>,
}

impl RingTrackedMidiBuffer {
    /// Creates a tracked buffer at `tpq` resolution holding up to `capacity`
    /// events.
    ///
    /// Fails with [`LiveMidiError::ZeroCapacity`] when `capacity` is zero.
    pub fn new(tpq: u32, capacity: usize) -> Result<Self, LiveMidiError> {
        if capacity == 0 {
            return Err(LiveMidiError::ZeroCapacity);
        }
        Ok(Self {
            tpq,
            capacity,
            dropped_events: 0,
            last_track: 0,
            n_tracks: 0,
            events: VecDeque::with_capacity(capacity),
        })
    }

    /// Returns the buffer's maximum capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the number of buffered events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns the track index of the most recently read event.
    pub fn last_track(&self) -> usize {
        self.last_track
    }

    /// Returns the running count of events dropped due to overflow.
    pub fn dropped_events(&self) -> usize {
        self.dropped_events
    }

    /// Removes all buffered events and resets the track counters.
    pub fn clear(&mut self) {
        self.events.clear();
        self.last_track = 0;
        self.n_tracks = 0;
    }

    /// Returns a copy of the currently buffered tracked events.
    pub fn snapshot(&self) -> Vec<TrackedMidiEvent> {
        self.events.iter().cloned().collect()
    }

    /// Pushes a tracked event, updating the track count and dropping the oldest
    /// event if the buffer is full.
    pub fn push_tracked_event(&mut self, event: TrackedMidiEvent) {
        self.n_tracks = self.n_tracks.max(event.last_track + 1);
        if self.events.len() == self.capacity {
            let _ = self.events.pop_front();
            self.dropped_events += 1;
        }
        self.events.push_back(event);
    }
}

impl MidiSource for RingTrackedMidiBuffer {
    type Err = Infallible;

    fn tpq(&self) -> u32 {
        self.tpq
    }

    fn next(&mut self) -> Result<Option<MidiEvent>, Self::Err> {
        let event = self.events.pop_front();
        if let Some(event) = &event {
            self.last_track = event.last_track;
        }
        Ok(event.map(|item| item.event))
    }
}

impl TrackedMidiSource for RingTrackedMidiBuffer {
    fn last_track(&self) -> usize {
        self.last_track
    }

    fn n_tracks(&self) -> usize {
        self.n_tracks
    }

    fn next_tracked(&mut self) -> Result<Option<TrackedMidiEvent>, Self::Err> {
        let event = self.events.pop_front();
        if let Some(event) = &event {
            self.last_track = event.last_track;
        }
        Ok(event)
    }
}
