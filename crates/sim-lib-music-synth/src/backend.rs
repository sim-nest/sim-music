use sim_kernel::{Error, Result, Symbol};

use crate::{ComponentParamDescriptor, ComponentPortDescriptor};

/// The synthesis backend a component runs: a fast algorithmic implementation
/// or a circuit-accurate model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComponentBackend {
    /// A fast, idealized algorithmic implementation.
    Algorithmic,
    /// A circuit-accurate modeled implementation.
    Modeled,
}

impl ComponentBackend {
    /// Returns the stable lowercase token for this backend.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Algorithmic => "algorithmic",
            Self::Modeled => "modeled",
        }
    }

    /// Returns the namespaced symbol naming this backend.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/backend", self.as_str())
    }
}

/// The port and parameter surface a component exposes for a given backend.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentBackendSurface {
    backend: ComponentBackend,
    ports: Vec<ComponentPortDescriptor>,
    params: Vec<ComponentParamDescriptor>,
}

impl ComponentBackendSurface {
    /// Creates a surface pairing a backend with its ports and parameters.
    pub fn new(
        backend: ComponentBackend,
        ports: Vec<ComponentPortDescriptor>,
        params: Vec<ComponentParamDescriptor>,
    ) -> Self {
        Self {
            backend,
            ports,
            params,
        }
    }

    /// Returns the backend this surface describes.
    pub fn backend(&self) -> ComponentBackend {
        self.backend
    }

    /// Returns the port descriptors.
    pub fn ports(&self) -> &[ComponentPortDescriptor] {
        &self.ports
    }

    /// Returns the parameter descriptors.
    pub fn params(&self) -> &[ComponentParamDescriptor] {
        &self.params
    }
}

/// Asserts that two backend surfaces expose identical ports and parameters,
/// erroring when either surface differs.
pub fn assert_backend_surface_identity(
    left: &ComponentBackendSurface,
    right: &ComponentBackendSurface,
) -> Result<()> {
    if left.ports != right.ports {
        return Err(Error::Eval(format!(
            "component backend port surface differs: {} vs {}",
            left.backend.as_str(),
            right.backend.as_str()
        )));
    }
    if left.params != right.params {
        return Err(Error::Eval(format!(
            "component backend parameter surface differs: {} vs {}",
            left.backend.as_str(),
            right.backend.as_str()
        )));
    }
    Ok(())
}
