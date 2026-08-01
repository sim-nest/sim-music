//! Compatibility wrapper for the default strict chromatic serial realizer.

use crate::{
    RealizationContext, SerialPlan, SerialRealization, StrictRealizationError,
    default_realizer_registry, strict_chromatic_realizer_id,
};

/// Realizes one immutable serial plan into exact note events and rests.
pub fn realize_strict(
    plan: &SerialPlan,
    context: &RealizationContext,
) -> Result<SerialRealization, StrictRealizationError> {
    default_realizer_registry().realize(&strict_chromatic_realizer_id(), plan, context)
}
