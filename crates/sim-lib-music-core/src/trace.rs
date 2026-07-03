use sim_kernel::Symbol;

use crate::{PlayEvent, PlayerDeviceId};

/// The kind of action a player chain took on an event, as traced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceAction {
    /// The event was newly produced by a player.
    Generated,
    /// The event was discarded and not forwarded.
    Dropped,
    /// The event was modified before being forwarded.
    Rewritten,
    /// The event was forwarded to a different device.
    Routed,
}

impl TraceAction {
    /// Returns the stable wire label for this action.
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::Generated => "generated",
            Self::Dropped => "dropped",
            Self::Rewritten => "rewritten",
            Self::Routed => "routed",
        }
    }

    /// Returns the qualified trace symbol for this action.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("music/player-trace", self.wire_label())
    }
}

/// A single trace record describing one action in a player chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainTraceRecord {
    /// Monotonic sequence number ordering records within a trace.
    pub sequence: u64,
    /// The device that produced or handled the event.
    pub device_id: PlayerDeviceId,
    /// The action taken on the event.
    pub action: TraceAction,
    /// The event the action applied to.
    pub event: PlayEvent,
    /// Human-readable detail about the action.
    pub detail: String,
}

impl ChainTraceRecord {
    /// Builds a trace record from its fields, converting `detail` into a string.
    pub fn new(
        sequence: u64,
        device_id: PlayerDeviceId,
        action: TraceAction,
        event: PlayEvent,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            sequence,
            device_id,
            action,
            event,
            detail: detail.into(),
        }
    }
}
