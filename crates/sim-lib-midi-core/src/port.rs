use std::collections::{BTreeMap, VecDeque};

/// Stable provider-neutral MIDI endpoint identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MidiPortId(pub String);

/// Direction advertised by a MIDI endpoint Card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiDirection {
    /// Receives messages.
    Input,
    /// Sends messages.
    Output,
    /// Sends and receives messages.
    Duplex,
}

/// Source of timestamps attached to received MIDI messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiTimestampSource {
    /// Timestamp supplied by the MIDI device.
    Device,
    /// Timestamp supplied by the capsule monotonic clock.
    Monotonic,
    /// Timestamp supplied as deterministic model data.
    Modeled,
}

/// Provider-neutral discovery Card for one MIDI endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiPortCard {
    /// Stable endpoint identity.
    pub id: MidiPortId,
    /// Human-facing label.
    pub label: String,
    /// Supported traffic direction.
    pub direction: MidiDirection,
    /// Capsule-owned transport name.
    pub transport: String,
    /// Timestamp authority.
    pub timestamp_source: MidiTimestampSource,
    /// Whether the provider reports hotplug changes.
    pub hotplug: bool,
}

/// A byte-exact MIDI message at a provider timestamp.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiPortMessage {
    /// Timestamp in the Card's declared source domain.
    pub timestamp: u64,
    /// Complete MIDI wire message, byte for byte.
    pub bytes: Vec<u8>,
}

impl MidiPortMessage {
    /// Constructs a message after validating its MIDI framing.
    pub fn new(timestamp: u64, bytes: Vec<u8>) -> Result<Self, MidiPortRefusal> {
        if bytes.is_empty() || bytes[0] < 0x80 || bytes.iter().skip(1).any(|byte| *byte >= 0x80) {
            return Err(MidiPortRefusal::InvalidMessage);
        }
        Ok(Self { timestamp, bytes })
    }
}

/// Typed refusal returned at the physical MIDI boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MidiPortRefusal {
    /// Backend or endpoint is not supplied by this capsule.
    Unsupported,
    /// Authority denied access.
    Denied,
    /// Known endpoint is temporarily unavailable.
    Unavailable,
    /// Bytes are not a complete MIDI message.
    InvalidMessage,
    /// Discovery exceeded the caller's finite bound.
    DiscoveryLimit,
    /// A bounded queue cannot accept more work.
    Backpressure,
    /// An open physical device disappeared.
    DeviceLost,
    /// The finite reconnect budget was exhausted.
    ReconnectLimit,
    /// The connection was already closed.
    AlreadyClosed,
}

/// Bounded discovery and buffering policy selected before native access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MidiPortPolicy {
    /// Maximum Cards accepted from one discovery pass.
    pub max_devices: usize,
    /// Maximum queued messages per connection.
    pub queue_capacity: usize,
    /// Maximum reconnect attempts.
    pub reconnect_attempts: u16,
}

impl MidiPortPolicy {
    /// Creates a finite nonzero discovery and queue policy.
    pub fn bounded(
        max_devices: usize,
        queue_capacity: usize,
        reconnect_attempts: u16,
    ) -> Result<Self, MidiPortRefusal> {
        if max_devices == 0 || queue_capacity == 0 {
            Err(MidiPortRefusal::DiscoveryLimit)
        } else {
            Ok(Self {
                max_devices,
                queue_capacity,
                reconnect_attempts,
            })
        }
    }
}

/// Open byte-exact MIDI connection. Implementations own cleanup and native handles.
pub trait MidiConnection: Send {
    /// Returns the Card used to open this connection.
    fn card(&self) -> &MidiPortCard;
    /// Receives the next message in provider order.
    fn receive(&mut self) -> Result<Option<MidiPortMessage>, MidiPortRefusal>;
    /// Sends one byte-exact message.
    fn send(&mut self, message: MidiPortMessage) -> Result<(), MidiPortRefusal>;
    /// Reconnects within the policy budget.
    fn reconnect(&mut self) -> Result<(), MidiPortRefusal>;
    /// Releases all connection resources.
    fn close(&mut self) -> Result<(), MidiPortRefusal>;
}

/// Physical MIDI port boundary. Domain crates never select or probe a backend.
pub trait MidiPort: Send + Sync {
    /// Discovers Cards within the caller's finite bound.
    fn cards(&self, policy: MidiPortPolicy) -> Result<Vec<MidiPortCard>, MidiPortRefusal>;
    /// Opens an exact known endpoint without probing alternatives.
    fn open(
        &self,
        id: &MidiPortId,
        policy: MidiPortPolicy,
    ) -> Result<Box<dyn MidiConnection>, MidiPortRefusal>;
}

/// Deterministic host-free implementation of [`MidiPort`].
#[derive(Default)]
pub struct ModelMidiPort {
    cards: BTreeMap<MidiPortId, MidiPortCard>,
    input: BTreeMap<MidiPortId, VecDeque<MidiPortMessage>>,
}

impl ModelMidiPort {
    /// Adds one Card and its ordered input script.
    pub fn add(&mut self, card: MidiPortCard, input: impl IntoIterator<Item = MidiPortMessage>) {
        self.input
            .insert(card.id.clone(), input.into_iter().collect());
        self.cards.insert(card.id.clone(), card);
    }
    /// Removes one Card, modeling hot-unplug.
    pub fn remove(&mut self, id: &MidiPortId) {
        self.cards.remove(id);
    }
}

impl MidiPort for ModelMidiPort {
    fn cards(&self, policy: MidiPortPolicy) -> Result<Vec<MidiPortCard>, MidiPortRefusal> {
        if self.cards.len() > policy.max_devices {
            return Err(MidiPortRefusal::DiscoveryLimit);
        }
        Ok(self.cards.values().cloned().collect())
    }
    fn open(
        &self,
        id: &MidiPortId,
        policy: MidiPortPolicy,
    ) -> Result<Box<dyn MidiConnection>, MidiPortRefusal> {
        let card = self
            .cards
            .get(id)
            .ok_or(MidiPortRefusal::Unsupported)?
            .clone();
        let input = self.input.get(id).cloned().unwrap_or_default();
        if input.len() > policy.queue_capacity {
            return Err(MidiPortRefusal::Backpressure);
        }
        Ok(Box::new(ModelMidiConnection {
            card,
            input,
            output: Vec::new(),
            closed: false,
            reconnects: 0,
            reconnect_limit: policy.reconnect_attempts,
        }))
    }
}

struct ModelMidiConnection {
    card: MidiPortCard,
    input: VecDeque<MidiPortMessage>,
    output: Vec<MidiPortMessage>,
    closed: bool,
    reconnects: u16,
    reconnect_limit: u16,
}

impl MidiConnection for ModelMidiConnection {
    fn card(&self) -> &MidiPortCard {
        &self.card
    }
    fn receive(&mut self) -> Result<Option<MidiPortMessage>, MidiPortRefusal> {
        if self.closed {
            Err(MidiPortRefusal::AlreadyClosed)
        } else {
            Ok(self.input.pop_front())
        }
    }
    fn send(&mut self, message: MidiPortMessage) -> Result<(), MidiPortRefusal> {
        if self.closed {
            Err(MidiPortRefusal::AlreadyClosed)
        } else {
            self.output.push(message);
            Ok(())
        }
    }
    fn reconnect(&mut self) -> Result<(), MidiPortRefusal> {
        if self.reconnects >= self.reconnect_limit {
            Err(MidiPortRefusal::ReconnectLimit)
        } else {
            self.reconnects += 1;
            self.closed = false;
            Ok(())
        }
    }
    fn close(&mut self) -> Result<(), MidiPortRefusal> {
        if self.closed {
            Err(MidiPortRefusal::AlreadyClosed)
        } else {
            self.closed = true;
            Ok(())
        }
    }
}
