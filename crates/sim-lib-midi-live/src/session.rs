use std::convert::Infallible;

use sim_lib_midi_core::{MidiEvent, MidiSink, MidiSource};

use crate::{LiveMidiError, RingMidiBuffer};

const DEFAULT_TPQ: u32 = 480;
const DEFAULT_CAPACITY: usize = 1024;

/// Direction exposed by a live MIDI session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveMidiDirection {
    /// Session produces MIDI events.
    Source,
    /// Session consumes MIDI events.
    Sink,
    /// Session both produces and consumes MIDI events.
    Duplex,
}

impl LiveMidiDirection {
    fn sink_enabled(self) -> bool {
        matches!(self, Self::Sink | Self::Duplex)
    }
}

/// Public handle for an active live MIDI stream.
///
/// The handle owns the bounded live ring used by host callbacks. Callback-side
/// entry points enqueue into the ring only; evaluation code consumes the ring
/// through the [`MidiSource`] and [`MidiSink`] accessors.
#[derive(Debug)]
pub struct LiveMidiSession {
    ring: RingMidiBuffer,
    direction: LiveMidiDirection,
}

impl LiveMidiSession {
    /// Creates a modeled session with the default MIDI tick resolution and ring
    /// capacity.
    pub fn modeled(direction: LiveMidiDirection) -> Result<Self, LiveMidiError> {
        Self::with_ring(DEFAULT_TPQ, DEFAULT_CAPACITY, direction)
    }

    /// Creates a session backed by a bounded ring buffer.
    pub fn with_ring(
        tpq: u32,
        capacity: usize,
        direction: LiveMidiDirection,
    ) -> Result<Self, LiveMidiError> {
        Ok(Self {
            ring: RingMidiBuffer::new(tpq, capacity)?,
            direction,
        })
    }

    /// Returns the stream direction exposed by this session.
    pub fn direction(&self) -> LiveMidiDirection {
        self.direction
    }

    /// Returns the live source side.
    pub fn source_mut(&mut self) -> &mut dyn MidiSource<Err = Infallible> {
        &mut self.ring
    }

    /// Returns the live sink side when the session supports output.
    pub fn sink_mut(&mut self) -> Option<&mut dyn MidiSink<Err = Infallible>> {
        if self.direction.sink_enabled() {
            Some(&mut self.ring)
        } else {
            None
        }
    }

    /// Enqueues one event from a host callback into the bounded live ring.
    pub fn enqueue_from_callback(&mut self, event: &MidiEvent) -> Result<(), Infallible> {
        self.ring.write(event)
    }

    /// Closes the session.
    pub fn close(self) -> Result<(), LiveMidiError> {
        Ok(())
    }
}
