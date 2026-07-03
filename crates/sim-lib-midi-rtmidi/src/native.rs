use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use sim_kernel::{Error, Result, Symbol};
use sim_lib_midi_core::{MidiEvent, MidiSink, MidiSource, synthetic_origin};
use sim_lib_midi_live::RingMidiBuffer;

use crate::{RtmidiBackend, RtmidiPort, RtmidiTiming, bytes_from_payload, payload_from_bytes};

type SharedRing = Arc<Mutex<RingMidiBuffer>>;

/// Hardware-provider configuration for RtMidi enumeration and opening.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RtmidiHardwareConfig {
    timing: RtmidiTiming,
}

impl RtmidiHardwareConfig {
    /// Creates a hardware configuration using the provided timestamp policy.
    pub fn new(timing: RtmidiTiming) -> Self {
        Self { timing }
    }

    /// Returns the timestamp conversion policy used by opened streams.
    pub fn timing(self) -> RtmidiTiming {
        self.timing
    }
}

/// Open RtMidi input driver handle.
pub struct RtmidiInputDriver {
    port: Symbol,
    queue: SharedRing,
    _connection: InputConnection,
}

impl fmt::Debug for RtmidiInputDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RtmidiInputDriver")
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

impl RtmidiInputDriver {
    /// Builds a fixture input driver over a caller-owned event queue.
    pub fn fixture(port: Symbol, queue: RingMidiBuffer) -> Result<Self> {
        Ok(Self {
            port,
            queue: Arc::new(Mutex::new(queue)),
            _connection: InputConnection::Fixture,
        })
    }

    #[cfg(all(
        feature = "rtmidi-hardware",
        any(target_os = "linux", target_os = "macos", target_os = "windows")
    ))]
    fn native(
        port: Symbol,
        queue: SharedRing,
        connection: midir::MidiInputConnection<SharedRing>,
    ) -> Self {
        Self {
            port,
            queue,
            _connection: InputConnection::Native {
                _connection: connection,
            },
        }
    }

    /// Returns the opened port id.
    pub fn port(&self) -> &Symbol {
        &self.port
    }

    /// Returns a snapshot of the currently buffered input events.
    pub fn queue_snapshot(&self) -> Result<Vec<MidiEvent>> {
        Ok(lock_ring(&self.queue)?.snapshot())
    }

    fn queue_handle(&self) -> SharedRing {
        self.queue.clone()
    }
}

/// MIDI input source backed by an RtMidi input driver.
pub struct RtmidiInputSource {
    ring: SharedRing,
    driver: RtmidiInputDriver,
    tpq: u32,
}

impl fmt::Debug for RtmidiInputSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RtmidiInputSource")
            .field("driver", &self.driver)
            .field("tpq", &self.tpq)
            .finish()
    }
}

impl RtmidiInputSource {
    /// Builds a source from an opened input driver and timing policy.
    pub fn new(driver: RtmidiInputDriver, timing: RtmidiTiming) -> Self {
        Self {
            ring: driver.queue_handle(),
            driver,
            tpq: timing.tpq(),
        }
    }

    /// Returns the opened driver handle.
    pub fn driver(&self) -> &RtmidiInputDriver {
        &self.driver
    }
}

impl MidiSource for RtmidiInputSource {
    type Err = Error;

    fn tpq(&self) -> u32 {
        self.tpq
    }

    fn next(&mut self) -> std::result::Result<Option<MidiEvent>, Self::Err> {
        lock_ring(&self.ring)?
            .next()
            .map_err(|never| match never {})
    }
}

/// Open RtMidi output driver handle.
pub struct RtmidiOutputDriver {
    port: Symbol,
    connection: OutputConnection,
}

impl fmt::Debug for RtmidiOutputDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RtmidiOutputDriver")
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

impl RtmidiOutputDriver {
    /// Builds a fixture output driver that records sent byte messages.
    pub fn fixture(port: Symbol) -> Result<Self> {
        Ok(Self {
            port,
            connection: OutputConnection::Fixture {
                messages: Vec::new(),
            },
        })
    }

    #[cfg(all(
        feature = "rtmidi-hardware",
        any(target_os = "linux", target_os = "macos", target_os = "windows")
    ))]
    fn native(port: Symbol, connection: midir::MidiOutputConnection) -> Self {
        Self {
            port,
            connection: OutputConnection::Native(connection),
        }
    }

    /// Returns the opened port id.
    pub fn port(&self) -> &Symbol {
        &self.port
    }

    /// Sends an outbound raw MIDI message.
    pub fn send(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() {
            return Err(Error::Eval(
                "RtMidi output message must include a status byte".to_owned(),
            ));
        }
        match &mut self.connection {
            OutputConnection::Fixture { messages } => {
                messages.push(bytes.to_vec());
                Ok(())
            }
            #[cfg(all(
                feature = "rtmidi-hardware",
                any(target_os = "linux", target_os = "macos", target_os = "windows")
            ))]
            OutputConnection::Native(connection) => connection
                .send(bytes)
                .map_err(|error| Error::Eval(format!("RtMidi output send failed: {error}"))),
        }
    }

    /// Flushes the opened output driver.
    pub fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    /// Returns recorded outbound messages for fixture drivers.
    pub fn messages(&self) -> &[Vec<u8>] {
        match &self.connection {
            OutputConnection::Fixture { messages } => messages,
            #[cfg(all(
                feature = "rtmidi-hardware",
                any(target_os = "linux", target_os = "macos", target_os = "windows")
            ))]
            OutputConnection::Native(_) => &[],
        }
    }
}

/// MIDI output sink backed by an RtMidi output driver.
pub struct RtmidiOutputSink {
    driver: RtmidiOutputDriver,
    tpq: u32,
}

impl fmt::Debug for RtmidiOutputSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RtmidiOutputSink")
            .field("driver", &self.driver)
            .field("tpq", &self.tpq)
            .finish()
    }
}

impl RtmidiOutputSink {
    /// Builds an output sink from an opened output driver and timing policy.
    pub fn new(driver: RtmidiOutputDriver, timing: RtmidiTiming) -> Self {
        Self {
            driver,
            tpq: timing.tpq(),
        }
    }

    /// Returns the opened driver handle.
    pub fn driver(&self) -> &RtmidiOutputDriver {
        &self.driver
    }
}

impl MidiSink for RtmidiOutputSink {
    type Err = Error;

    fn tpq(&self) -> u32 {
        self.tpq
    }

    fn write(&mut self, event: &MidiEvent) -> std::result::Result<(), Self::Err> {
        self.driver.send(&bytes_from_payload(&event.payload)?)
    }

    fn flush(&mut self) -> std::result::Result<(), Self::Err> {
        self.driver.flush()
    }
}

enum InputConnection {
    Fixture,
    #[cfg(all(
        feature = "rtmidi-hardware",
        any(target_os = "linux", target_os = "macos", target_os = "windows")
    ))]
    Native {
        _connection: midir::MidiInputConnection<SharedRing>,
    },
}

enum OutputConnection {
    Fixture {
        messages: Vec<Vec<u8>>,
    },
    #[cfg(all(
        feature = "rtmidi-hardware",
        any(target_os = "linux", target_os = "macos", target_os = "windows")
    ))]
    Native(midir::MidiOutputConnection),
}

/// Provider seam for RtMidi-compatible ports and driver handles.
pub trait RtmidiProvider: Send + Sync {
    /// Lists provider-visible MIDI ports.
    fn list_ports(&self) -> Result<Vec<RtmidiPort>>;

    /// Opens an input port into the provided ring queue.
    fn open_input(&self, port: &Symbol, queue: RingMidiBuffer) -> Result<RtmidiInputDriver>;

    /// Opens an output port.
    fn open_output(&self, port: &Symbol) -> Result<RtmidiOutputDriver>;

    /// Enumerates the provider as a host backend using default timing.
    fn enumerate_backend(&self) -> Result<RtmidiBackend> {
        Ok(RtmidiBackend::new(self.list_ports()?))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeRtmidiTransport {
    #[cfg(target_os = "linux")]
    AlsaSeq,
    #[cfg(target_os = "macos")]
    CoreMidi,
    #[cfg(target_os = "windows")]
    WinMm,
}

impl NativeRtmidiTransport {
    fn input_prefix(self) -> &'static str {
        match self {
            #[cfg(target_os = "linux")]
            Self::AlsaSeq => "rtmidi/alsa/in-",
            #[cfg(target_os = "macos")]
            Self::CoreMidi => "rtmidi/coremidi/in-",
            #[cfg(target_os = "windows")]
            Self::WinMm => "rtmidi/winmm/in-",
        }
    }

    fn output_prefix(self) -> &'static str {
        match self {
            #[cfg(target_os = "linux")]
            Self::AlsaSeq => "rtmidi/alsa/out-",
            #[cfg(target_os = "macos")]
            Self::CoreMidi => "rtmidi/coremidi/out-",
            #[cfg(target_os = "windows")]
            Self::WinMm => "rtmidi/winmm/out-",
        }
    }

    fn input_id(self, index: usize) -> String {
        format!("{}{index}", self.input_prefix())
    }

    fn output_id(self, index: usize) -> String {
        format!("{}{index}", self.output_prefix())
    }
}

/// Native RtMidi provider backed by platform MIDI enumeration APIs.
#[derive(Clone, Debug)]
pub struct NativeRtmidiProvider {
    config: RtmidiHardwareConfig,
    transport: NativeRtmidiTransport,
}

impl NativeRtmidiProvider {
    /// Creates a Linux ALSA sequencer provider.
    #[cfg(target_os = "linux")]
    pub fn alsa_seq(config: RtmidiHardwareConfig) -> Self {
        Self {
            config,
            transport: NativeRtmidiTransport::AlsaSeq,
        }
    }

    /// Creates a macOS CoreMIDI provider.
    #[cfg(target_os = "macos")]
    pub fn coremidi(config: RtmidiHardwareConfig) -> Self {
        Self {
            config,
            transport: NativeRtmidiTransport::CoreMidi,
        }
    }

    /// Creates a Windows multimedia MIDI provider.
    #[cfg(target_os = "windows")]
    pub fn winmm(config: RtmidiHardwareConfig) -> Self {
        Self {
            config,
            transport: NativeRtmidiTransport::WinMm,
        }
    }

    /// Enumerates native ports as a host backend.
    pub fn enumerate_backend(&self) -> Result<RtmidiBackend> {
        let ports = self.list_ports()?;
        if ports.is_empty() {
            return Err(Error::Eval(
                "RtMidi reported no usable MIDI ports".to_owned(),
            ));
        }
        Ok(RtmidiBackend::new(ports).with_timing(self.config.timing()))
    }

    #[cfg(not(all(
        feature = "rtmidi-hardware",
        any(target_os = "linux", target_os = "macos", target_os = "windows")
    )))]
    fn unavailable() -> Error {
        Error::Eval("RtMidi native hardware is unavailable on this target".to_owned())
    }
}

impl RtmidiProvider for NativeRtmidiProvider {
    fn list_ports(&self) -> Result<Vec<RtmidiPort>> {
        list_native_ports(self.transport)
    }

    fn open_input(&self, port: &Symbol, queue: RingMidiBuffer) -> Result<RtmidiInputDriver> {
        open_native_input(port, queue, self.config.timing(), self.transport)
    }

    fn open_output(&self, port: &Symbol) -> Result<RtmidiOutputDriver> {
        open_native_output(port, self.transport)
    }

    fn enumerate_backend(&self) -> Result<RtmidiBackend> {
        NativeRtmidiProvider::enumerate_backend(self)
    }
}

#[cfg(all(
    feature = "rtmidi-hardware",
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
fn list_native_ports(transport: NativeRtmidiTransport) -> Result<Vec<RtmidiPort>> {
    let input = midir::MidiInput::new("sim rtmidi input")
        .map_err(|error| Error::Eval(format!("RtMidi input init failed: {error}")))?;
    let output = midir::MidiOutput::new("sim rtmidi output")
        .map_err(|error| Error::Eval(format!("RtMidi output init failed: {error}")))?;

    let mut ports = Vec::new();
    for (index, port) in input.ports().iter().enumerate() {
        let name = input
            .port_name(port)
            .unwrap_or_else(|_| format!("RtMidi input {index}"));
        ports.push(RtmidiPort::input(transport.input_id(index), name, index));
    }
    for (index, port) in output.ports().iter().enumerate() {
        let name = output
            .port_name(port)
            .unwrap_or_else(|_| format!("RtMidi output {index}"));
        ports.push(RtmidiPort::output(transport.output_id(index), name, index));
    }

    if ports.is_empty() {
        return Err(Error::Eval(
            "RtMidi reported no usable MIDI ports".to_owned(),
        ));
    }
    Ok(ports)
}

#[cfg(not(all(
    feature = "rtmidi-hardware",
    any(target_os = "linux", target_os = "macos", target_os = "windows")
)))]
fn list_native_ports(_transport: NativeRtmidiTransport) -> Result<Vec<RtmidiPort>> {
    Err(NativeRtmidiProvider::unavailable())
}

#[cfg(all(
    feature = "rtmidi-hardware",
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
fn open_native_input(
    port: &Symbol,
    queue: RingMidiBuffer,
    timing: RtmidiTiming,
    transport: NativeRtmidiTransport,
) -> Result<RtmidiInputDriver> {
    let mut input = midir::MidiInput::new("sim rtmidi input")
        .map_err(|error| Error::Eval(format!("RtMidi input init failed: {error}")))?;
    input.ignore(midir::Ignore::None);
    let ports = input.ports();
    let index = port_index(port, transport.input_prefix())?;
    let Some(midir_port) = ports.get(index) else {
        return Err(Error::Eval(format!(
            "RtMidi input port {port} was not found"
        )));
    };
    let queue = Arc::new(Mutex::new(queue));
    let callback_queue = queue.clone();
    let connection = input
        .connect(
            midir_port,
            "sim-rtmidi-input",
            move |timestamp, bytes, ring| {
                let Ok(payload) = payload_from_bytes(bytes) else {
                    return;
                };
                let event = MidiEvent {
                    time: timing.timestamp_to_ticks(timestamp),
                    origin: synthetic_origin(),
                    payload,
                };
                if let Ok(mut ring) = ring.lock() {
                    let _ = ring.write(&event);
                }
            },
            callback_queue,
        )
        .map_err(|error| Error::Eval(format!("RtMidi input open failed: {error}")))?;
    Ok(RtmidiInputDriver::native(port.clone(), queue, connection))
}

#[cfg(not(all(
    feature = "rtmidi-hardware",
    any(target_os = "linux", target_os = "macos", target_os = "windows")
)))]
fn open_native_input(
    _port: &Symbol,
    _queue: RingMidiBuffer,
    _timing: RtmidiTiming,
    _transport: NativeRtmidiTransport,
) -> Result<RtmidiInputDriver> {
    Err(NativeRtmidiProvider::unavailable())
}

#[cfg(all(
    feature = "rtmidi-hardware",
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
fn open_native_output(
    port: &Symbol,
    transport: NativeRtmidiTransport,
) -> Result<RtmidiOutputDriver> {
    let output = midir::MidiOutput::new("sim rtmidi output")
        .map_err(|error| Error::Eval(format!("RtMidi output init failed: {error}")))?;
    let ports = output.ports();
    let index = port_index(port, transport.output_prefix())?;
    let Some(midir_port) = ports.get(index) else {
        return Err(Error::Eval(format!(
            "RtMidi output port {port} was not found"
        )));
    };
    let connection = output
        .connect(midir_port, "sim-rtmidi-output")
        .map_err(|error| Error::Eval(format!("RtMidi output open failed: {error}")))?;
    Ok(RtmidiOutputDriver::native(port.clone(), connection))
}

#[cfg(not(all(
    feature = "rtmidi-hardware",
    any(target_os = "linux", target_os = "macos", target_os = "windows")
)))]
fn open_native_output(
    _port: &Symbol,
    _transport: NativeRtmidiTransport,
) -> Result<RtmidiOutputDriver> {
    Err(NativeRtmidiProvider::unavailable())
}

#[cfg(all(
    feature = "rtmidi-hardware",
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
fn port_index(port: &Symbol, prefix: &str) -> Result<usize> {
    let Some(rest) = port.name.strip_prefix(prefix) else {
        return Err(Error::Eval(format!(
            "RtMidi port {port} does not use prefix {prefix}"
        )));
    };
    rest.parse::<usize>()
        .map_err(|error| Error::Eval(format!("RtMidi port {port} has invalid index: {error}")))
}

fn lock_ring(ring: &SharedRing) -> Result<MutexGuard<'_, RingMidiBuffer>> {
    ring.lock()
        .map_err(|_| Error::Eval("RtMidi input queue lock was poisoned".to_owned()))
}

fn live_error(error: sim_lib_midi_live::LiveMidiError) -> Error {
    Error::Eval(format!("RtMidi live buffer error: {error}"))
}

/// Builds a ring buffer for opened RtMidi input streams.
pub fn input_ring(timing: RtmidiTiming, capacity: usize) -> Result<RingMidiBuffer> {
    RingMidiBuffer::new(timing.tpq(), capacity).map_err(live_error)
}
