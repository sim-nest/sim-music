use std::convert::Infallible;

use crate::{MidiEvent, PumpError, TickTime};

/// A [`MidiEvent`] tagged with the track it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackedMidiEvent {
    /// Index of the track that produced the event.
    pub last_track: usize,
    /// The event itself.
    pub event: MidiEvent,
}

/// A pull-based stream of [`MidiEvent`]s at a fixed resolution.
pub trait MidiSource {
    /// Error type returned by [`next`](Self::next).
    type Err;

    /// Returns the source resolution in ticks per quarter note.
    fn tpq(&self) -> u32;

    /// Returns the next event, or `None` once the stream is exhausted.
    fn next(&mut self) -> Result<Option<MidiEvent>, Self::Err>;
}

/// A [`MidiSource`] that also reports per-track membership.
pub trait TrackedMidiSource: MidiSource {
    /// Returns the track index of the most recently yielded event.
    fn last_track(&self) -> usize;

    /// Returns the total number of tracks in the stream.
    fn n_tracks(&self) -> usize;

    /// Returns the next event together with its track tag.
    fn next_tracked(&mut self) -> Result<Option<TrackedMidiEvent>, Self::Err>;
}

/// A push-based sink that accepts [`MidiEvent`]s at a fixed resolution.
pub trait MidiSink {
    /// Error type returned by [`write`](Self::write) and [`flush`](Self::flush).
    type Err;

    /// Returns the sink resolution in ticks per quarter note.
    fn tpq(&self) -> u32;

    /// Writes one event to the sink.
    fn write(&mut self, event: &MidiEvent) -> Result<(), Self::Err>;

    /// Flushes any buffered state to the underlying destination.
    fn flush(&mut self) -> Result<(), Self::Err>;
}

/// An in-memory [`MidiSource`] backed by a time-sorted event vector.
#[derive(Clone, Debug, Default)]
pub struct MemoryMidiSource {
    tpq: u32,
    events: Vec<MidiEvent>,
    cursor: usize,
}

impl MemoryMidiSource {
    /// Creates a source at `tpq` resolution, sorting `events` by tick.
    pub fn new(tpq: u32, mut events: Vec<MidiEvent>) -> Self {
        events.sort_by_key(|event| event.time.ticks);
        Self {
            tpq,
            events,
            cursor: 0,
        }
    }
}

/// An in-memory [`TrackedMidiSource`] backed by a time-sorted, track-tagged
/// event vector.
#[derive(Clone, Debug, Default)]
pub struct MemoryTrackedMidiSource {
    tpq: u32,
    events: Vec<TrackedMidiEvent>,
    cursor: usize,
    last_track: usize,
    n_tracks: usize,
}

impl MemoryTrackedMidiSource {
    /// Creates a tracked source at `tpq` resolution, sorting `events` by tick
    /// and deriving the track count from the highest track index.
    pub fn new(tpq: u32, mut events: Vec<TrackedMidiEvent>) -> Self {
        events.sort_by_key(|item| item.event.time.ticks);
        let n_tracks = events
            .iter()
            .map(|item| item.last_track + 1)
            .max()
            .unwrap_or(0);
        Self {
            tpq,
            events,
            cursor: 0,
            last_track: 0,
            n_tracks,
        }
    }
}

impl MidiSource for MemoryMidiSource {
    type Err = Infallible;

    fn tpq(&self) -> u32 {
        self.tpq
    }

    fn next(&mut self) -> Result<Option<MidiEvent>, Self::Err> {
        let event = self.events.get(self.cursor).cloned();
        if event.is_some() {
            self.cursor += 1;
        }
        Ok(event)
    }
}

impl MidiSource for MemoryTrackedMidiSource {
    type Err = Infallible;

    fn tpq(&self) -> u32 {
        self.tpq
    }

    fn next(&mut self) -> Result<Option<MidiEvent>, Self::Err> {
        let event = self.events.get(self.cursor).cloned();
        if let Some(event) = &event {
            self.last_track = event.last_track;
            self.cursor += 1;
        }
        Ok(event.map(|item| item.event))
    }
}

impl TrackedMidiSource for MemoryTrackedMidiSource {
    fn last_track(&self) -> usize {
        self.last_track
    }

    fn n_tracks(&self) -> usize {
        self.n_tracks
    }

    fn next_tracked(&mut self) -> Result<Option<TrackedMidiEvent>, Self::Err> {
        let event = self.events.get(self.cursor).cloned();
        if let Some(event) = &event {
            self.last_track = event.last_track;
            self.cursor += 1;
        }
        Ok(event)
    }
}

/// An in-memory [`MidiSink`] that collects written events in tick order.
#[derive(Clone, Debug, Default)]
pub struct MemoryMidiSink {
    tpq: u32,
    events: Vec<MidiEvent>,
}

impl MemoryMidiSink {
    /// Creates an empty sink at `tpq` resolution.
    pub fn new(tpq: u32) -> Self {
        Self {
            tpq,
            events: Vec::new(),
        }
    }

    /// Returns the events collected so far, sorted by tick.
    pub fn events(&self) -> &[MidiEvent] {
        &self.events
    }
}

impl MidiSink for MemoryMidiSink {
    type Err = Infallible;

    fn tpq(&self) -> u32 {
        self.tpq
    }

    fn write(&mut self, event: &MidiEvent) -> Result<(), Self::Err> {
        self.events.push(event.clone());
        self.events.sort_by_key(|item| item.time.ticks);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Err> {
        Ok(())
    }
}

/// Drains every event from `source` into `sink`, re-timing events to the sink's
/// resolution, and returns the number of events transferred.
///
/// Mismatched resolutions are reconciled by [`TickTime::quantize`]; the sink is
/// flushed once the source is exhausted. Errors are wrapped in [`PumpError`] to
/// record which side failed.
pub fn pump<S, T>(source: &mut S, sink: &mut T) -> Result<usize, PumpError<S::Err, T::Err>>
where
    S: MidiSource,
    T: MidiSink,
{
    let mut count = 0usize;
    let sink_tpq = sink.tpq();
    let source_tpq = source.tpq();
    while let Some(mut event) = source.next().map_err(PumpError::Source)? {
        if source_tpq != sink_tpq {
            event.time = event.time.quantize(sink_tpq);
        } else if event.time.tpq != sink_tpq {
            event.time = TickTime {
                ticks: event.time.ticks,
                tpq: sink_tpq,
            };
        }
        sink.write(&event).map_err(PumpError::Sink)?;
        count += 1;
    }
    sink.flush().map_err(PumpError::Sink)?;
    Ok(count)
}
