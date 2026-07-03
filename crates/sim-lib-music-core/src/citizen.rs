use sim_kernel::{Expr, Symbol};

use crate::MusicComponentDescriptor;

/// Runtime citizen wrapper around a [`MusicComponentDescriptor`].
///
/// Pairs a component descriptor with the object surface the runtime expects,
/// exposing it as a first-class music citizen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicComponentCitizenDescriptor {
    descriptor: MusicComponentDescriptor,
}

impl MusicComponentCitizenDescriptor {
    /// Wraps `descriptor` as a citizen descriptor.
    pub fn new(descriptor: MusicComponentDescriptor) -> Self {
        Self { descriptor }
    }

    /// Borrows the underlying component descriptor.
    pub fn descriptor(&self) -> &MusicComponentDescriptor {
        &self.descriptor
    }

    /// Renders the citizen as its descriptor expression.
    pub fn as_expr(&self) -> Expr {
        self.descriptor.to_expr()
    }
}

/// Returns the class symbol for music component descriptors.
pub fn music_component_descriptor_class_symbol() -> Symbol {
    Symbol::qualified("music", "ComponentDescriptor")
}

/// Returns the class symbol for the music component registry.
pub fn music_component_registry_class_symbol() -> Symbol {
    Symbol::qualified("music", "ComponentRegistry")
}

/// Returns the class symbol for music component cards.
pub fn music_component_card_class_symbol() -> Symbol {
    Symbol::qualified("music", "ComponentCard")
}
