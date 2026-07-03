use std::collections::BTreeMap;

use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_core::{BufferPolicy, StreamMedia};
use sim_lib_stream_host::{
    HostBackend, HostBackendCapability, HostBackendInfo, HostClockInfo, HostDeviceInventory,
    HostDeviceSpec, HostDirection, HostLatencyInfo, HostOpenStream, HostPortSpec, HostStreamConfig,
    HostStreamConfigRequest,
};

use crate::{RtmidiEvent, RtmidiMidiSink, RtmidiMidiSource, RtmidiPort, RtmidiTiming};

#[cfg(feature = "rtmidi-hardware")]
use crate::{NativeRtmidiProvider, RtmidiHardwareConfig};

/// Returns the host-backend symbol `stream/host:rtmidi`.
pub fn rtmidi_backend_symbol() -> Symbol {
    Symbol::qualified("stream/host", "rtmidi")
}

/// Returns the transport symbol `stream/transport:rtmidi`.
pub fn rtmidi_transport_symbol() -> Symbol {
    Symbol::qualified("stream/transport", "rtmidi")
}

/// Host backend adapter with deterministic provider data.
#[derive(Clone, Debug)]
pub struct RtmidiBackend {
    info: HostBackendInfo,
    ports: Vec<RtmidiPort>,
    input_events: BTreeMap<Symbol, Vec<RtmidiEvent>>,
    timing: RtmidiTiming,
}

impl Default for RtmidiBackend {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl RtmidiBackend {
    /// Creates an RtMidi adapter from provider-reported ports.
    pub fn new(ports: Vec<RtmidiPort>) -> Self {
        Self {
            info: HostBackendInfo::new(
                rtmidi_backend_symbol(),
                rtmidi_transport_symbol(),
                StreamMedia::Midi,
                true,
            )
            .with_capabilities(vec![
                HostBackendCapability::MidiInput,
                HostBackendCapability::MidiOutput,
                HostBackendCapability::Reconnect,
            ]),
            ports,
            input_events: BTreeMap::new(),
            timing: RtmidiTiming::default(),
        }
    }

    /// Creates deterministic fake RtMidi ports for tests and examples.
    pub fn fake() -> Self {
        let mut backend = Self::new(vec![
            RtmidiPort::input("rtmidi/fake-in", "Fake RtMidi Input", 0),
            RtmidiPort::output("rtmidi/fake-out", "Fake RtMidi Output", 0),
        ]);
        backend.info = HostBackendInfo::new(
            rtmidi_backend_symbol(),
            rtmidi_transport_symbol(),
            StreamMedia::Midi,
            false,
        )
        .with_capabilities(vec![
            HostBackendCapability::MidiInput,
            HostBackendCapability::MidiOutput,
            HostBackendCapability::Reconnect,
            HostBackendCapability::Offline,
            HostBackendCapability::Fake,
        ]);
        backend
    }

    /// Enumerates Linux ALSA-sequencer ports through the native provider.
    #[cfg(all(feature = "rtmidi-hardware", target_os = "linux"))]
    pub fn hardware_alsa(config: RtmidiHardwareConfig) -> Result<Self> {
        NativeRtmidiProvider::alsa_seq(config).enumerate_backend()
    }

    /// Enumerates macOS CoreMIDI ports through the native provider.
    #[cfg(all(feature = "rtmidi-hardware", target_os = "macos"))]
    pub fn hardware_coremidi(config: RtmidiHardwareConfig) -> Result<Self> {
        NativeRtmidiProvider::coremidi(config).enumerate_backend()
    }

    /// Enumerates Windows multimedia MIDI ports through the native provider.
    #[cfg(all(feature = "rtmidi-hardware", target_os = "windows"))]
    pub fn hardware_winmm(config: RtmidiHardwareConfig) -> Result<Self> {
        NativeRtmidiProvider::winmm(config).enumerate_backend()
    }

    /// Returns this backend with the given timestamp-conversion timing applied.
    pub fn with_timing(mut self, timing: RtmidiTiming) -> Self {
        self.timing = timing;
        self
    }

    /// Returns this backend with queued input events for an input `port`.
    ///
    /// Fails if `port` is not a known input port.
    pub fn with_input_events(mut self, port: &Symbol, events: Vec<RtmidiEvent>) -> Result<Self> {
        self.require_port(port, HostDirection::Input)?;
        self.input_events.insert(port.clone(), events);
        Ok(self)
    }

    /// Returns the provider-reported ports.
    pub fn list_ports(&self) -> &[RtmidiPort] {
        &self.ports
    }

    /// Opens a MIDI source over an input `port`, draining any queued events.
    pub fn open_midi_source(&self, port: &Symbol) -> Result<RtmidiMidiSource> {
        self.require_port(port, HostDirection::Input)?;
        RtmidiMidiSource::from_events(
            self.timing,
            self.input_events.get(port).cloned().unwrap_or_default(),
        )
    }

    /// Opens a MIDI sink over an output `port`.
    pub fn open_midi_sink(&self, port: &Symbol) -> Result<RtmidiMidiSink> {
        self.require_port(port, HostDirection::Output)?;
        RtmidiMidiSink::new(self.timing.tpq())
    }

    /// Returns the timestamp-conversion timing for opened streams.
    pub fn timing(&self) -> RtmidiTiming {
        self.timing
    }

    fn require_port(&self, port: &Symbol, direction: HostDirection) -> Result<&RtmidiPort> {
        let Some(port) = self.ports.iter().find(|candidate| candidate.id() == port) else {
            return Err(Error::Eval(format!("RtMidi port {port} was not found")));
        };
        if port.direction() != direction {
            return Err(Error::TypeMismatch {
                expected: "RtMidi port with requested direction",
                found: "RtMidi port with another direction",
            });
        }
        Ok(port)
    }
}

impl HostBackend for RtmidiBackend {
    fn info(&self) -> &HostBackendInfo {
        &self.info
    }

    fn enumerate(&self) -> Result<HostDeviceInventory> {
        let devices = self
            .ports
            .iter()
            .map(|port| {
                HostDeviceSpec::new(
                    port.id().clone(),
                    rtmidi_backend_symbol(),
                    StreamMedia::Midi,
                    port.direction(),
                    Symbol::qualified("clock", "rtmidi"),
                    BufferPolicy::bounded(64).expect("valid RtMidi buffer"),
                )
            })
            .collect::<Vec<_>>();
        let ports = self
            .ports
            .iter()
            .map(|port| {
                HostPortSpec::new(
                    Symbol::new(format!("{}/port", port.id())),
                    port.id().clone(),
                    rtmidi_backend_symbol(),
                    StreamMedia::Midi,
                    port.direction(),
                )
            })
            .collect();
        Ok(HostDeviceInventory::new(rtmidi_backend_symbol())
            .with_devices(devices)
            .with_ports(ports))
    }

    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream> {
        if request.backend() != self.info.id() {
            return Err(Error::Eval(format!(
                "RtMidi backend cannot open {} requests",
                request.backend()
            )));
        }
        self.require_port(request.device(), request.direction())?;
        if request.media() != StreamMedia::Midi {
            return Err(Error::TypeMismatch {
                expected: "MIDI stream request",
                found: "non-MIDI stream request",
            });
        }
        let config = HostStreamConfig::from_request(
            request,
            HostLatencyInfo::default(),
            HostClockInfo::new(Symbol::qualified("clock", "rtmidi"), None, false),
        );
        Ok(HostOpenStream::new(config))
    }
}
