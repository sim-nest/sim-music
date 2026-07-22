use sim_kernel::{Error, Result, Symbol};
mod bluez;
mod packet;

pub use packet::{decode_ble_midi_packet, encode_ble_midi_event};

use sim_lib_midi_core::{MidiEvent, MidiSink, MidiSource};
use sim_lib_midi_live::{LiveMidiError, RingMidiBuffer};
use sim_lib_stream_host::{
    CatalogDeviceProvider, DeviceDirection, DeviceKind, DeviceRecord, Placement, StreamEvalSite,
};

use bluez::{discover_bluez_dbus, write_bluez_characteristic};

use crate::{BleMidiDevice, BleMidiDiscoveryReport, missing_bluez_dependency_card};

/// Default BLE-MIDI stream resolution used by hardware fixtures.
pub const DEFAULT_TPQ: u32 = 960;

/// Default fixed event capacity for BLE-MIDI input buffers.
pub const DEFAULT_RING_CAPACITY: usize = 64;

/// Discovery contract for BLE-MIDI hardware providers.
pub trait BleMidiDiscoveryProvider: Send + Sync {
    /// Discovers BLE-MIDI devices and companion browse cards.
    fn discover(&self) -> Result<BleMidiDiscoveryReport>;
}

/// Deterministic BLE-MIDI discovery provider for tests and examples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureBleMidiProvider {
    devices: Vec<BleMidiDevice>,
}

impl FixtureBleMidiProvider {
    /// Creates a fixture provider from known BLE-MIDI devices.
    pub fn new(devices: Vec<BleMidiDevice>) -> Self {
        Self { devices }
    }
}

impl BleMidiDiscoveryProvider for FixtureBleMidiProvider {
    fn discover(&self) -> Result<BleMidiDiscoveryReport> {
        Ok(BleMidiDiscoveryReport::new(
            self.devices.clone(),
            Vec::new(),
        ))
    }
}

/// Hardware-provider configuration for BLE-MIDI streams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BleMidiHardwareConfig {
    tpq: u32,
    ring_capacity: usize,
}

impl Default for BleMidiHardwareConfig {
    fn default() -> Self {
        Self {
            tpq: DEFAULT_TPQ,
            ring_capacity: DEFAULT_RING_CAPACITY,
        }
    }
}

impl BleMidiHardwareConfig {
    /// Creates a hardware configuration with a stream resolution and input capacity.
    pub fn new(tpq: u32, ring_capacity: usize) -> Result<Self> {
        if tpq == 0 {
            return Err(Error::Eval(
                "BLE-MIDI stream TPQ must be greater than zero".to_owned(),
            ));
        }
        if ring_capacity == 0 {
            return Err(Error::Eval(
                "BLE-MIDI input ring capacity must be greater than zero".to_owned(),
            ));
        }
        Ok(Self { tpq, ring_capacity })
    }

    /// Returns the stream resolution in ticks per quarter note.
    pub fn tpq(self) -> u32 {
        self.tpq
    }

    /// Returns the input ring capacity.
    pub fn ring_capacity(self) -> usize {
        self.ring_capacity
    }
}

/// Linux BlueZ discovery provider for BLE-MIDI devices.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BluezBleMidiProvider {
    config: BleMidiHardwareConfig,
}

impl BluezBleMidiProvider {
    /// Creates a BlueZ provider with the given stream configuration.
    pub fn new(config: BleMidiHardwareConfig) -> Self {
        Self { config }
    }

    /// Returns the stream configuration used by opened devices.
    pub fn config(&self) -> BleMidiHardwareConfig {
        self.config
    }
}

impl BleMidiDiscoveryProvider for BluezBleMidiProvider {
    fn discover(&self) -> Result<BleMidiDiscoveryReport> {
        let _ = self.config;
        #[cfg(target_os = "linux")]
        {
            bluez_discovery_report(discover_bluez_dbus())
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(Error::Eval(
                "BlueZ BLE-MIDI discovery is available on Linux".to_owned(),
            ))
        }
    }
}

pub(crate) fn bluez_discovery_report(
    devices: Result<Vec<BleMidiDevice>>,
) -> Result<BleMidiDiscoveryReport> {
    let devices = devices?;
    let cards = if devices.is_empty() {
        vec![missing_bluez_dependency_card()]
    } else {
        Vec::new()
    };
    Ok(BleMidiDiscoveryReport::new(devices, cards))
}

/// Open BLE-MIDI GATT I/O characteristic session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BleMidiGattSession {
    device: BleMidiDevice,
    io: BleMidiGattIo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BleMidiGattIo {
    Fixture { written_packets: Vec<Vec<u8>> },
    Bluez { characteristic_path: String },
}

impl BleMidiGattSession {
    /// Creates a fixture GATT session for a deterministic BLE-MIDI device.
    pub fn fixture(device: BleMidiDevice) -> Self {
        Self {
            device,
            io: BleMidiGattIo::Fixture {
                written_packets: Vec::new(),
            },
        }
    }

    /// Opens a GATT session for a discovered BLE-MIDI device.
    pub fn open(device: &BleMidiDevice) -> Result<Self> {
        if device.address().starts_with("fixture:") {
            Ok(Self::fixture(device.clone()))
        } else if let Some(path) = device.gatt_path() {
            Ok(Self {
                device: device.clone(),
                io: BleMidiGattIo::Bluez {
                    characteristic_path: path.to_owned(),
                },
            })
        } else {
            Err(Error::Eval(format!(
                "BleMidiGattSession: hardware GATT open requires operator BlueZ setup for '{}'",
                device.id()
            )))
        }
    }

    /// Returns the device associated with this session.
    pub fn device(&self) -> &BleMidiDevice {
        &self.device
    }

    /// Writes a BLE-MIDI packet to the I/O characteristic.
    pub fn write_io_characteristic(&mut self, packet: &[u8]) -> Result<()> {
        if packet.is_empty() {
            return Err(Error::Eval(
                "BLE-MIDI packet must contain a timestamp header".to_owned(),
            ));
        }
        match &mut self.io {
            BleMidiGattIo::Fixture { written_packets } => {
                written_packets.push(packet.to_vec());
                Ok(())
            }
            BleMidiGattIo::Bluez {
                characteristic_path,
            } => write_bluez_characteristic(characteristic_path, packet),
        }
    }

    /// Flushes the opened GATT session.
    pub fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    /// Returns the packets written by a fixture session.
    pub fn written_packets(&self) -> &[Vec<u8>] {
        match &self.io {
            BleMidiGattIo::Fixture { written_packets } => written_packets,
            BleMidiGattIo::Bluez { .. } => &[],
        }
    }
}

/// MIDI input source backed by a BLE-MIDI GATT input ring.
#[derive(Clone, Debug)]
pub struct BleMidiInputSource {
    ring: RingMidiBuffer,
    session: BleMidiGattSession,
    tpq: u32,
}

impl BleMidiInputSource {
    /// Builds an empty BLE-MIDI source from an opened GATT session.
    pub fn new(session: BleMidiGattSession, tpq: u32, capacity: usize) -> Result<Self> {
        Ok(Self {
            ring: input_ring(tpq, capacity)?,
            session,
            tpq,
        })
    }

    /// Builds a source and primes it with one BLE-MIDI packet.
    pub fn from_packet(
        session: BleMidiGattSession,
        packet: &[u8],
        tpq: u32,
        capacity: usize,
    ) -> Result<Self> {
        let mut source = Self::new(session, tpq, capacity)?;
        let _ = source.push_packet(packet)?;
        Ok(source)
    }

    /// Decodes a BLE-MIDI packet and appends its events to the input ring.
    pub fn push_packet(&mut self, packet: &[u8]) -> Result<usize> {
        let events = decode_ble_midi_packet(packet, self.tpq)?;
        let count = events.len();
        for event in events {
            self.ring.write(&event).map_err(|never| match never {})?;
        }
        Ok(count)
    }

    /// Returns the opened GATT session.
    pub fn session(&self) -> &BleMidiGattSession {
        &self.session
    }
}

impl MidiSource for BleMidiInputSource {
    type Err = Error;

    fn tpq(&self) -> u32 {
        self.tpq
    }

    fn next(&mut self) -> std::result::Result<Option<MidiEvent>, Self::Err> {
        self.ring.next().map_err(|never| match never {})
    }
}

/// MIDI output sink backed by a BLE-MIDI GATT output characteristic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BleMidiOutputSink {
    session: BleMidiGattSession,
    tpq: u32,
}

impl BleMidiOutputSink {
    /// Builds an output sink from an opened GATT session.
    pub fn new(session: BleMidiGattSession, tpq: u32) -> Self {
        Self { session, tpq }
    }

    /// Returns the opened GATT session.
    pub fn session(&self) -> &BleMidiGattSession {
        &self.session
    }
}

impl MidiSink for BleMidiOutputSink {
    type Err = Error;

    fn tpq(&self) -> u32 {
        self.tpq
    }

    fn write(&mut self, event: &MidiEvent) -> std::result::Result<(), Self::Err> {
        let mut event = event.clone();
        if event.time.tpq != self.tpq {
            event.time = event.time.quantize(self.tpq);
        }
        let packet = encode_ble_midi_event(&event)?;
        self.session.write_io_characteristic(&packet)
    }

    fn flush(&mut self) -> std::result::Result<(), Self::Err> {
        self.session.flush()
    }
}

/// Stream-host provider that maps discovered BLE-MIDI devices to hardware placement rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BleMidiProvider {
    devices: Vec<BleMidiDevice>,
    config: BleMidiHardwareConfig,
}

impl BleMidiProvider {
    /// Builds a provider from a discovery pass using the default stream configuration.
    pub fn from_discovery(provider: &dyn BleMidiDiscoveryProvider) -> Result<Self> {
        Self::from_discovery_with_config(provider, BleMidiHardwareConfig::default())
    }

    /// Builds a provider from a discovery pass with a caller-supplied configuration.
    pub fn from_discovery_with_config(
        provider: &dyn BleMidiDiscoveryProvider,
        config: BleMidiHardwareConfig,
    ) -> Result<Self> {
        let report = provider.discover()?;
        Ok(Self {
            devices: report.devices().to_vec(),
            config,
        })
    }

    /// Builds a provider from known devices.
    pub fn from_devices(devices: Vec<BleMidiDevice>, config: BleMidiHardwareConfig) -> Self {
        Self { devices, config }
    }

    fn record(device: &BleMidiDevice) -> DeviceRecord {
        DeviceRecord {
            id: device.id().clone(),
            display_name: device.name().to_owned(),
            kind: DeviceKind::Midi,
            direction: DeviceDirection::Duplex,
            placement: Placement::Hardware {
                transport: Symbol::new("ble-midi"),
            },
        }
    }
}

impl CatalogDeviceProvider for BleMidiProvider {
    fn enumerate(&self) -> Result<Vec<DeviceRecord>> {
        Ok(self.devices.iter().map(Self::record).collect())
    }

    fn open(&self, id: &Symbol) -> Result<Box<dyn StreamEvalSite>> {
        let device = self
            .devices
            .iter()
            .find(|device| device.id() == id)
            .ok_or_else(|| Error::Eval(format!("BleMidiProvider: unknown device '{id}'")))?;
        let record = Self::record(device);
        let session = BleMidiGattSession::open(device)?;
        let input = BleMidiInputSource::new(
            session.clone(),
            self.config.tpq(),
            self.config.ring_capacity(),
        )?;
        let output = BleMidiOutputSink::new(session, self.config.tpq());
        Ok(Box::new(BleMidiDuplexEvalSite {
            record,
            input,
            output,
        }))
    }
}

/// Duplex stream evaluation site for an opened BLE-MIDI device.
#[derive(Clone, Debug)]
pub struct BleMidiDuplexEvalSite {
    record: DeviceRecord,
    input: BleMidiInputSource,
    output: BleMidiOutputSink,
}

impl BleMidiDuplexEvalSite {
    /// Returns the input source owned by this eval site.
    pub fn input(&self) -> &BleMidiInputSource {
        &self.input
    }

    /// Returns the output sink owned by this eval site.
    pub fn output(&self) -> &BleMidiOutputSink {
        &self.output
    }
}

impl StreamEvalSite for BleMidiDuplexEvalSite {
    fn placement(&self) -> &Placement {
        &self.record.placement
    }

    fn device_record(&self) -> &DeviceRecord {
        &self.record
    }

    fn close(mut self: Box<Self>) -> Result<()> {
        self.output.flush()
    }
}

fn input_ring(tpq: u32, capacity: usize) -> Result<RingMidiBuffer> {
    RingMidiBuffer::new(tpq, capacity).map_err(live_error)
}

fn live_error(error: LiveMidiError) -> Error {
    Error::Eval(format!("BLE-MIDI ring buffer error: {error}"))
}
