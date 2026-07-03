use sim_kernel::{Error, Result, Symbol};
use sim_lib_music_core::Arranger;

use super::{non_empty, symbol};

/// Deterministic clip metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct DawClip {
    pub(crate) id: Symbol,
    pub(crate) start_frame: u64,
    pub(crate) frames: u64,
    pub(crate) source: ClipSource,
    pub(crate) gain: f32,
}

/// Clip signal source used by offline preview rendering.
#[derive(Clone, Debug, PartialEq)]
pub enum ClipSource {
    /// Emits no signal.
    Silence,
    /// Emits a constant sample value across the clip.
    Constant(f32),
    /// References an audio graph patch node by id (not rendered offline).
    PatchNode(String),
    /// Plays a music-core arranger (not rendered offline).
    Arranger(Arranger),
}

impl DawClip {
    /// Creates a constant-value clip with unity gain.
    pub fn constant(
        id: impl Into<String>,
        start_frame: u64,
        frames: u64,
        value: f32,
    ) -> Result<Self> {
        Self::new(id, start_frame, frames, ClipSource::Constant(value), 1.0)
    }

    /// Creates a silent clip with unity gain.
    pub fn silence(id: impl Into<String>, start_frame: u64, frames: u64) -> Result<Self> {
        Self::new(id, start_frame, frames, ClipSource::Silence, 1.0)
    }

    /// Creates a clip backed by an arranger with unity gain.
    pub fn arranger(
        id: impl Into<String>,
        start_frame: u64,
        frames: u64,
        arranger: Arranger,
    ) -> Result<Self> {
        Self::new(id, start_frame, frames, ClipSource::Arranger(arranger), 1.0)
    }

    /// Creates a clip with an explicit source and gain.
    ///
    /// Fails if `frames` is zero, the source is invalid, or `gain` is not
    /// finite.
    pub fn new(
        id: impl Into<String>,
        start_frame: u64,
        frames: u64,
        source: ClipSource,
        gain: f32,
    ) -> Result<Self> {
        if frames == 0 {
            return Err(Error::Eval(
                "DAW clip frame count must be greater than zero".to_owned(),
            ));
        }
        source.validate()?;
        if !gain.is_finite() {
            return Err(Error::Eval("DAW clip gain must be finite".to_owned()));
        }
        Ok(Self {
            id: symbol(id, "clip id")?,
            start_frame,
            frames,
            source,
            gain,
        })
    }

    /// Returns the clip id symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the clip start position in frames.
    pub fn start_frame(&self) -> u64 {
        self.start_frame
    }

    /// Returns the clip length in frames.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Returns the clip signal source.
    pub fn source(&self) -> &ClipSource {
        &self.source
    }

    /// Returns the clip gain multiplier.
    pub fn gain(&self) -> f32 {
        self.gain
    }
}

impl ClipSource {
    /// Builds a [`ClipSource::PatchNode`] source, rejecting an empty node id.
    pub fn patch_node(id: impl Into<String>) -> Result<Self> {
        Ok(Self::PatchNode(non_empty(id.into(), "clip patch node")?))
    }

    /// Validates the source: constant values must be finite and patch node ids
    /// must be non-empty.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Silence => Ok(()),
            Self::Constant(value) if value.is_finite() => Ok(()),
            Self::Constant(_) => Err(Error::Eval(
                "DAW clip constant source must be finite".to_owned(),
            )),
            Self::PatchNode(id) if !id.trim().is_empty() => Ok(()),
            Self::PatchNode(_) => Err(Error::Eval(
                "DAW clip patch node id must not be empty".to_owned(),
            )),
            Self::Arranger(_) => Ok(()),
        }
    }
}
