//! Open serial realizer registry.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::{
    ChromaticSerialRealizer, RealizationContext, RealizerId, SerialPlan, SerialRealization,
    SerialRealizer, StrictRealizationError,
};

/// Id-addressed registry of serial realizers with stable sorted listing.
#[derive(Clone, Default)]
pub struct SerialRealizerRegistry {
    entries: BTreeMap<RealizerId, Arc<dyn SerialRealizer>>,
}

impl SerialRealizerRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts one realizer, rejecting duplicate ids unless the caller uses `replace`.
    pub fn register(&mut self, realizer: Arc<dyn SerialRealizer>) -> Result<(), RealizerId> {
        if self.entries.contains_key(realizer.id()) {
            return Err(realizer.id().clone());
        }
        self.entries.insert(realizer.id().clone(), realizer);
        Ok(())
    }

    /// Inserts one realizer and returns the replaced registration, if any.
    pub fn replace(
        &mut self,
        realizer: Arc<dyn SerialRealizer>,
    ) -> Option<Arc<dyn SerialRealizer>> {
        self.entries.insert(realizer.id().clone(), realizer)
    }

    /// Returns one registered realizer by id.
    pub fn get(&self, id: &RealizerId) -> Option<&Arc<dyn SerialRealizer>> {
        self.entries.get(id)
    }

    /// Returns all registered ids in stable sorted order.
    pub fn ids(&self) -> Vec<RealizerId> {
        self.entries.keys().cloned().collect()
    }

    /// Realizes one plan through the registered realizer named by `id`.
    pub fn realize(
        &self,
        id: &RealizerId,
        plan: &SerialPlan,
        context: &RealizationContext,
    ) -> Result<SerialRealization, StrictRealizationError> {
        let Some(realizer) = self.get(id) else {
            return Err(StrictRealizationError::UnknownRealizer(id.clone()));
        };
        realizer.realize(plan, context)
    }
}

/// Returns the built-in registry with the strict chromatic realizer installed.
pub fn default_realizer_registry() -> SerialRealizerRegistry {
    let mut registry = SerialRealizerRegistry::new();
    registry.replace(Arc::new(ChromaticSerialRealizer::default()));
    registry
}
