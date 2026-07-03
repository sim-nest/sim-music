use sim_kernel::{Error, Result, Symbol};
use sim_lib_midi_core::TickTime;
use sim_lib_stream_host::{HostDirection, HostReconnectPolicy};

/// Timestamp conversion policy for RtMidi callback timestamps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RtmidiTiming {
    tpq: u32,
    us_per_quarter: u32,
}

/// One timestamped raw MIDI callback payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RtmidiEvent {
    timestamp_micros: u64,
    bytes: Vec<u8>,
}

/// SIM-visible RtMidi port metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RtmidiPort {
    id: Symbol,
    name: String,
    direction: HostDirection,
    index: usize,
    reconnect: HostReconnectPolicy,
}

impl Default for RtmidiTiming {
    fn default() -> Self {
        Self {
            tpq: 960,
            us_per_quarter: 500_000,
        }
    }
}

impl RtmidiTiming {
    /// Creates a timing conversion using MIDI ticks per quarter and tempo.
    pub fn new(tpq: u32, us_per_quarter: u32) -> Result<Self> {
        if tpq == 0 {
            return Err(Error::Eval(
                "RtMidi TPQ must be greater than zero".to_owned(),
            ));
        }
        if us_per_quarter == 0 {
            return Err(Error::Eval(
                "RtMidi tempo must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            tpq,
            us_per_quarter,
        })
    }

    /// Returns the resolution in ticks per quarter note.
    pub fn tpq(self) -> u32 {
        self.tpq
    }

    /// Returns the tempo in microseconds per quarter note.
    pub fn us_per_quarter(self) -> u32 {
        self.us_per_quarter
    }

    /// Converts backend microsecond timestamps into SIM MIDI tick time.
    pub fn timestamp_to_ticks(self, timestamp_micros: u64) -> TickTime {
        let scaled = u128::from(timestamp_micros) * u128::from(self.tpq);
        let ticks = scaled / u128::from(self.us_per_quarter);
        TickTime {
            ticks: ticks.min(i64::MAX as u128) as i64,
            tpq: self.tpq,
        }
    }
}

impl RtmidiEvent {
    /// Creates one raw MIDI event with a backend timestamp in microseconds.
    pub fn new(timestamp_micros: u64, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            timestamp_micros,
            bytes: bytes.into(),
        }
    }

    /// Returns the backend timestamp in microseconds.
    pub fn timestamp_micros(&self) -> u64 {
        self.timestamp_micros
    }

    /// Returns the raw MIDI message bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl RtmidiPort {
    /// Builds an input port at the given provider `index`.
    pub fn input(id: impl Into<String>, name: impl Into<String>, index: usize) -> Self {
        Self::new(id, name, HostDirection::Input, index)
    }

    /// Builds an output port at the given provider `index`.
    pub fn output(id: impl Into<String>, name: impl Into<String>, index: usize) -> Self {
        Self::new(id, name, HostDirection::Output, index)
    }

    /// Builds a port with an explicit direction and reconnection disabled.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        direction: HostDirection,
        index: usize,
    ) -> Self {
        Self {
            id: Symbol::new(id.into()),
            name: name.into(),
            direction,
            index,
            reconnect: HostReconnectPolicy::disabled(),
        }
    }

    /// Returns this port with the given reconnection policy applied.
    pub fn with_reconnect(mut self, reconnect: HostReconnectPolicy) -> Self {
        self.reconnect = reconnect;
        self
    }

    /// Returns the port's stable id symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the human-readable port name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the port direction (input or output).
    pub fn direction(&self) -> HostDirection {
        self.direction
    }

    /// Returns the provider-reported port index.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Returns the port's reconnection policy.
    pub fn reconnect(&self) -> &HostReconnectPolicy {
        &self.reconnect
    }
}
