//! Component port descriptors: media kind, direction, and rate contract.
//!
//! Defines the signal media a port carries ([`ComponentPortMedia`]), its
//! [`ComponentPortDirection`], and the [`ComponentPortDescriptor`] that bundles
//! a port's id, media, direction, channel count, required flag, and
//! `RateContract`.

use sim_kernel::Symbol;
use sim_lib_audio_graph_core::RateContract;

/// The kind of signal a component port carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentPortMedia {
    /// Audio-rate sample stream.
    AudioRate,
    /// Control-voltage modulation signal.
    ControlVoltage,
    /// Control-rate parameter stream.
    ControlRate,
    /// Discrete event stream (such as MIDI).
    Event,
    /// Side-channel metadata.
    Metadata,
    /// Note gate signal.
    Gate,
    /// Diagnostic trace stream.
    Trace,
}

impl ComponentPortMedia {
    /// Returns the stable lowercase name of the media kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AudioRate => "audio-rate",
            Self::ControlVoltage => "control-voltage",
            Self::ControlRate => "control-rate",
            Self::Event => "event",
            Self::Metadata => "metadata",
            Self::Gate => "gate",
            Self::Trace => "trace",
        }
    }

    /// Parses a media kind from its lowercase name, returning `None` if
    /// unrecognized.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "audio-rate" => Some(Self::AudioRate),
            "control-voltage" => Some(Self::ControlVoltage),
            "control-rate" => Some(Self::ControlRate),
            "event" => Some(Self::Event),
            "metadata" => Some(Self::Metadata),
            "gate" => Some(Self::Gate),
            "trace" => Some(Self::Trace),
            _ => None,
        }
    }

    /// Returns the qualified symbol naming this media kind.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/port-media", self.as_str())
    }
}

/// Whether a component port consumes or produces signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentPortDirection {
    /// Port that consumes signal into the component.
    Input,
    /// Port that produces signal out of the component.
    Output,
}

impl ComponentPortDirection {
    /// Returns the stable lowercase name of the direction.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }

    /// Returns the qualified symbol naming this direction.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/port-direction", self.as_str())
    }
}

/// Describes one component port: id, media, direction, channel count, required
/// flag, and rate contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentPortDescriptor {
    id: Symbol,
    media: ComponentPortMedia,
    direction: ComponentPortDirection,
    channels: u16,
    required: bool,
    rate_contract: RateContract,
}

impl ComponentPortDescriptor {
    /// Builds a required port from its id, media, direction, and channel count
    /// (clamped to at least 1), with the media's default rate contract.
    pub fn new(
        id: Symbol,
        media: ComponentPortMedia,
        direction: ComponentPortDirection,
        channels: u16,
    ) -> Self {
        Self {
            id,
            media,
            direction,
            channels: channels.max(1),
            required: true,
            rate_contract: media.default_rate_contract(),
        }
    }

    /// Marks the port as optional, returning `self` for chaining.
    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    /// Overrides the port's rate contract, returning `self` for chaining.
    pub fn with_rate_contract(mut self, rate_contract: RateContract) -> Self {
        self.rate_contract = rate_contract;
        self
    }

    /// Returns the port's identity symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the port's media kind.
    pub fn media(&self) -> ComponentPortMedia {
        self.media
    }

    /// Returns the port's direction.
    pub fn direction(&self) -> ComponentPortDirection {
        self.direction
    }

    /// Returns the port's channel count.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Returns whether the port is required.
    pub fn required(&self) -> bool {
        self.required
    }

    /// Returns the port's rate contract.
    pub fn rate_contract(&self) -> RateContract {
        self.rate_contract
    }
}

impl ComponentPortMedia {
    /// Returns the rate contract a port of this media kind uses by default.
    pub fn default_rate_contract(self) -> RateContract {
        match self {
            Self::AudioRate => RateContract::sample_exact(None),
            Self::ControlVoltage | Self::ControlRate | Self::Gate | Self::Metadata => {
                RateContract::control()
            }
            Self::Event => RateContract::midi_tick(),
            Self::Trace => RateContract::trace_step(),
        }
    }
}

/// Returns the qualified symbols for all seven [`ComponentPortMedia`] kinds.
pub fn component_port_media_symbols() -> [Symbol; 7] {
    [
        ComponentPortMedia::AudioRate.symbol(),
        ComponentPortMedia::ControlVoltage.symbol(),
        ComponentPortMedia::ControlRate.symbol(),
        ComponentPortMedia::Event.symbol(),
        ComponentPortMedia::Metadata.symbol(),
        ComponentPortMedia::Gate.symbol(),
        ComponentPortMedia::Trace.symbol(),
    ]
}
