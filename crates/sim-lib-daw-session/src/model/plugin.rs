use sim_kernel::{Result, Symbol};
use sim_lib_plugin_core::{PluginId, PluginState};

use super::symbol;

/// Plugin chain assigned to a track.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PluginChain {
    pub(crate) slots: Vec<PluginSlot>,
}

/// One plugin slot plus its serialized state.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginSlot {
    pub(crate) id: Symbol,
    pub(crate) plugin: PluginId,
    pub(crate) state: PluginState,
    pub(crate) bypassed: bool,
}

impl PluginChain {
    /// Creates a chain from an ordered list of slots.
    pub fn new(slots: Vec<PluginSlot>) -> Self {
        Self { slots }
    }

    /// Returns the chain with one more slot appended.
    pub fn with_slot(mut self, slot: PluginSlot) -> Self {
        self.slots.push(slot);
        self
    }

    /// Returns the ordered plugin slots.
    pub fn slots(&self) -> &[PluginSlot] {
        &self.slots
    }
}

impl PluginSlot {
    /// Creates a non-bypassed plugin slot, rejecting an empty id.
    pub fn new(id: impl Into<String>, plugin: PluginId, state: PluginState) -> Result<Self> {
        Ok(Self {
            id: symbol(id, "plugin slot id")?,
            plugin,
            state,
            bypassed: false,
        })
    }

    /// Returns the slot with its bypass flag set.
    pub fn bypassed(mut self, bypassed: bool) -> Self {
        self.bypassed = bypassed;
        self
    }

    /// Returns the slot id symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the plugin identity for this slot.
    pub fn plugin(&self) -> &PluginId {
        &self.plugin
    }

    /// Returns the serialized plugin state.
    pub fn state(&self) -> &PluginState {
        &self.state
    }

    /// Returns whether the slot is bypassed.
    pub fn is_bypassed(&self) -> bool {
        self.bypassed
    }
}
