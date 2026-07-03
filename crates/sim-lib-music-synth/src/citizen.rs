use sim_citizen_derive::Citizen;
use sim_kernel::{Expr, Result, Symbol};

use crate::SynthPreset;

/// A citizen descriptor wrapping a [`SynthPreset`] as its serialized expression
/// for storage and exchange across the runtime.
#[derive(Clone, Debug, PartialEq, Citizen)]
#[citizen(symbol = "audio-synth/Preset", version = 1)]
pub struct SynthPresetDescriptor {
    #[citizen(with = "preset_expr")]
    preset: Expr,
}

impl SynthPresetDescriptor {
    /// Creates a descriptor from a preset by encoding it to an expression.
    pub fn new(preset: SynthPreset) -> Self {
        Self {
            preset: preset.to_expr(),
        }
    }

    /// Builds a descriptor from a preset expression, validating that it decodes
    /// to a [`SynthPreset`].
    pub fn from_expr(expr: Expr) -> Result<Self> {
        preset_expr::decode(&expr)?;
        Ok(Self { preset: expr })
    }

    /// Decodes the wrapped expression back into a [`SynthPreset`].
    pub fn preset(&self) -> Result<SynthPreset> {
        SynthPreset::from_expr(&self.preset)
    }

    /// Returns the wrapped preset expression.
    pub fn as_expr(&self) -> &Expr {
        &self.preset
    }
}

impl Default for SynthPresetDescriptor {
    fn default() -> Self {
        Self::new(SynthPreset::default())
    }
}

/// Returns the class symbol naming the synth preset citizen.
pub fn synth_preset_class_symbol() -> Symbol {
    Symbol::qualified("audio-synth", "Preset")
}

pub(crate) mod preset_expr {
    use sim_kernel::{Expr, Result};

    use crate::SynthPreset;

    pub fn encode(expr: &Expr) -> Expr {
        expr.clone()
    }

    pub fn decode(expr: &Expr) -> Result<Expr> {
        SynthPreset::from_expr(expr)?;
        Ok(expr.clone())
    }
}
