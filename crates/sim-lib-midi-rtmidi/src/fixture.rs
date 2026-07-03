use sim_kernel::{Error, Result, Symbol};
use sim_lib_midi_live::RingMidiBuffer;
use sim_lib_stream_host::HostDirection;

use crate::native::{RtmidiInputDriver, RtmidiOutputDriver, RtmidiProvider};
use crate::{RtmidiBackend, RtmidiPort, RtmidiTiming};

/// Deterministic RtMidi provider for catalog and driver tests.
#[derive(Clone, Debug)]
pub struct FixtureRtmidiProvider {
    ports: Vec<RtmidiPort>,
    timing: RtmidiTiming,
}

impl FixtureRtmidiProvider {
    /// Creates a fixture provider from static port metadata.
    pub fn new(ports: Vec<RtmidiPort>) -> Self {
        Self {
            ports,
            timing: RtmidiTiming::default(),
        }
    }

    /// Returns this provider with the given timestamp policy.
    pub fn with_timing(mut self, timing: RtmidiTiming) -> Self {
        self.timing = timing;
        self
    }

    /// Enumerates the fixture as a host backend.
    pub fn enumerate_backend(&self) -> Result<RtmidiBackend> {
        Ok(RtmidiBackend::new(self.list_ports()?).with_timing(self.timing))
    }

    fn require_port(&self, port: &Symbol, direction: HostDirection) -> Result<&RtmidiPort> {
        let Some(candidate) = self.ports.iter().find(|candidate| candidate.id() == port) else {
            return Err(Error::Eval(format!(
                "RtMidi fixture port {port} was not found"
            )));
        };
        if candidate.direction() != direction {
            return Err(Error::TypeMismatch {
                expected: "RtMidi fixture port with requested direction",
                found: "RtMidi fixture port with another direction",
            });
        }
        Ok(candidate)
    }
}

impl RtmidiProvider for FixtureRtmidiProvider {
    fn list_ports(&self) -> Result<Vec<RtmidiPort>> {
        Ok(self.ports.clone())
    }

    fn open_input(&self, port: &Symbol, queue: RingMidiBuffer) -> Result<RtmidiInputDriver> {
        self.require_port(port, HostDirection::Input)?;
        RtmidiInputDriver::fixture(port.clone(), queue)
    }

    fn open_output(&self, port: &Symbol) -> Result<RtmidiOutputDriver> {
        self.require_port(port, HostDirection::Output)?;
        RtmidiOutputDriver::fixture(port.clone())
    }

    fn enumerate_backend(&self) -> Result<RtmidiBackend> {
        FixtureRtmidiProvider::enumerate_backend(self)
    }
}
