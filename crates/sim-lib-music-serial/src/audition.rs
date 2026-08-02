//! Serial audition and export adapters over existing score, lowering, and notation owners.

use sim_lib_music_core::{Score, SmfFile};
use sim_lib_music_lower::{LowerError, LowerOpts, lower_score, write_smf};
use thiserror::Error;

use crate::{SerialRealization, SerialRenderOptions, StrictRealizationError, render_serial_score};

/// Error surfaced while routing realized serial music through existing owners.
#[derive(Debug, Error)]
pub enum SerialSurfaceError {
    /// Rendering to the canonical score failed.
    #[error(transparent)]
    Realization(#[from] StrictRealizationError),
    /// MIDI lowering failed in the existing lowering owner.
    #[error(transparent)]
    Lower(#[from] LowerError),
}

/// Renders a serial realization into the existing score surface for audition.
pub fn render_serial_audition_score(
    realization: &SerialRealization,
    options: &SerialRenderOptions,
) -> Result<Score, SerialSurfaceError> {
    Ok(render_serial_score(realization, options)?)
}

/// Lowers a rendered serial score through the existing MIDI owner.
pub fn lower_serial_score(
    realization: &SerialRealization,
    render: &SerialRenderOptions,
    lower: &LowerOpts,
) -> Result<SmfFile, SerialSurfaceError> {
    let score = render_serial_audition_score(realization, render)?;
    Ok(lower_score(&score, lower)?)
}

/// Serializes a rendered serial score to SMF bytes through the existing MIDI owner.
pub fn write_serial_smf(
    realization: &SerialRealization,
    render: &SerialRenderOptions,
    lower: &LowerOpts,
) -> Result<Vec<u8>, SerialSurfaceError> {
    let score = render_serial_audition_score(realization, render)?;
    Ok(write_smf(&score, lower)?)
}
