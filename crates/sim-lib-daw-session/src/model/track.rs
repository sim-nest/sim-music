use sim_kernel::{Error, Result, Symbol};

use super::{DawClip, PluginChain, non_empty, symbol};

/// Session track role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DawTrackKind {
    /// Audio track rendered to the mix.
    Audio,
    /// MIDI track driving instruments.
    Midi,
    /// Arranger track hosting arrangement clips.
    Arranger,
    /// Auxiliary/send track.
    Aux,
}

/// Audio/MIDI/aux track metadata plus clips and plugin chain.
#[derive(Clone, Debug, PartialEq)]
pub struct DawTrack {
    pub(crate) id: Symbol,
    pub(crate) name: String,
    pub(crate) kind: DawTrackKind,
    pub(crate) channels: u16,
    pub(crate) bus: Option<Symbol>,
    pub(crate) clips: Vec<DawClip>,
    pub(crate) plugin_chain: PluginChain,
    pub(crate) armed: bool,
    pub(crate) muted: bool,
    pub(crate) solo: bool,
}

impl DawTrackKind {
    /// Returns the stable lowercase name for this track kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Midi => "midi",
            Self::Arranger => "arranger",
            Self::Aux => "aux",
        }
    }

    /// Parses a track kind from its stable name.
    pub fn parse_name(text: &str) -> Result<Self> {
        match text {
            "audio" => Ok(Self::Audio),
            "midi" => Ok(Self::Midi),
            "arranger" => Ok(Self::Arranger),
            "aux" => Ok(Self::Aux),
            _ => Err(Error::Eval(format!("unknown DAW track kind: {text}"))),
        }
    }
}

impl DawTrack {
    /// Creates an audio track routed to the `master` bus.
    pub fn audio(id: impl Into<String>, name: impl Into<String>, channels: u16) -> Result<Self> {
        Self::new(id, name, DawTrackKind::Audio, channels)
    }

    /// Creates a track of the given kind, rejecting an empty id/name or zero
    /// channel count. New tracks default to the `master` bus and no clips.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        kind: DawTrackKind,
        channels: u16,
    ) -> Result<Self> {
        if channels == 0 {
            return Err(Error::Eval(
                "DAW track channel count must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            id: symbol(id, "track id")?,
            name: non_empty(name.into(), "track name")?,
            kind,
            channels,
            bus: Some(Symbol::new("master")),
            clips: Vec::new(),
            plugin_chain: PluginChain::default(),
            armed: false,
            muted: false,
            solo: false,
        })
    }

    /// Returns the track with its output bus replaced (or detached with `None`).
    pub fn with_bus(mut self, bus: Option<Symbol>) -> Self {
        self.bus = bus;
        self
    }

    /// Returns the track with one more clip appended.
    pub fn with_clip(mut self, clip: DawClip) -> Self {
        self.clips.push(clip);
        self
    }

    /// Returns the track with its plugin chain replaced.
    pub fn with_plugin_chain(mut self, plugin_chain: PluginChain) -> Self {
        self.plugin_chain = plugin_chain;
        self
    }

    /// Returns the track with its record-arm flag set.
    pub fn armed(mut self, armed: bool) -> Self {
        self.armed = armed;
        self
    }

    /// Returns the track with its mute flag set.
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// Returns the track with its solo flag set.
    pub fn solo(mut self, solo: bool) -> Self {
        self.solo = solo;
        self
    }

    /// Returns the track id symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the track name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the track kind.
    pub fn kind(&self) -> DawTrackKind {
        self.kind
    }

    /// Returns the track channel count.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Returns the output bus, if the track is routed to one.
    pub fn bus(&self) -> Option<&Symbol> {
        self.bus.as_ref()
    }

    /// Returns the track clips.
    pub fn clips(&self) -> &[DawClip] {
        &self.clips
    }

    /// Returns the track plugin chain.
    pub fn plugin_chain(&self) -> &PluginChain {
        &self.plugin_chain
    }

    /// Returns whether the track is record-armed.
    pub fn is_armed(&self) -> bool {
        self.armed
    }

    /// Returns whether the track is muted.
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Returns whether the track is soloed.
    pub fn is_solo(&self) -> bool {
        self.solo
    }
}
