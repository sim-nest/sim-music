use sim_citizen_derive::Citizen;
use sim_kernel::{Diagnostic, Error, Result, Symbol};
use sim_lib_midi_core::DEFAULT_US_PER_QUARTER;
use sim_lib_stream_core::StreamValue;

/// Result of a bridge conversion: the produced stream plus any diagnostics.
pub struct BridgeOutput {
    /// Converted stream carrying the resulting MIDI or PCM packets.
    pub stream: StreamValue,
    /// Diagnostics emitted while converting (for example, unrepresentable pitches).
    pub diagnostics: Vec<Diagnostic>,
}

/// Options controlling MIDI-to-PCM rendering through the sound libraries.
///
/// # Examples
///
/// ```
/// use sim_lib_stream_bridge::StreamBridgeRenderOptions;
///
/// let opts = StreamBridgeRenderOptions::default();
/// assert_eq!(opts.channels, 2);
/// assert_eq!(opts.sample_rate, 48_000);
/// ```
#[derive(Clone, Debug, PartialEq, Citizen)]
#[citizen(symbol = "stream-bridge/RenderOptions", version = 1)]
pub struct StreamBridgeRenderOptions {
    /// Output sample rate in hertz.
    pub sample_rate: u32,
    /// Number of interleaved output channels.
    pub channels: u8,
    /// Number of frames carried by each produced PCM packet.
    pub chunk_frames: usize,
}

impl StreamBridgeRenderOptions {
    /// Builds render options and rejects values the renderer cannot honor.
    pub fn new(sample_rate: u32, channels: u8, chunk_frames: usize) -> Result<Self> {
        let options = Self {
            sample_rate,
            channels,
            chunk_frames,
        };
        options.validate()?;
        Ok(options)
    }

    /// Validates the public option fields before rendering.
    pub fn validate(&self) -> Result<()> {
        if self.sample_rate == 0 {
            return Err(Error::Eval(
                "stream/bridge render sample_rate must be greater than zero".to_owned(),
            ));
        }
        if !(1..=2).contains(&self.channels) {
            return Err(Error::Eval(
                "stream/bridge render channels must be 1 or 2".to_owned(),
            ));
        }
        if self.chunk_frames == 0 {
            return Err(Error::Eval(
                "stream/bridge render chunk_frames must be greater than zero".to_owned(),
            ));
        }
        Ok(())
    }
}

impl Default for StreamBridgeRenderOptions {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            channels: 2,
            chunk_frames: 512,
        }
    }
}

/// Options controlling PCM-to-MIDI lifting through the audio-lift libraries.
#[derive(Clone, Debug, PartialEq, Citizen)]
#[citizen(symbol = "stream-bridge/LiftMidiOptions", version = 1)]
pub struct StreamBridgeLiftMidiOptions {
    /// Input sample rate in hertz used to convert sample offsets to time.
    pub sample_rate: u32,
    /// Ticks per quarter note for the lifted MIDI timeline.
    pub tpq: u16,
    /// Microseconds per quarter note used to map seconds onto ticks.
    pub us_per_quarter: u32,
    /// Minimum confidence a lifted note candidate must reach to be emitted.
    pub min_confidence: f64,
    /// Analysis window size, in samples, for the lifter.
    pub window_size: usize,
    /// Hop size, in samples, between successive analysis windows.
    pub hop_size: usize,
    /// Maximum number of MIDI events packed into a single stream packet.
    pub max_events_per_packet: usize,
}

impl StreamBridgeLiftMidiOptions {
    /// Validates the public option fields before PCM-to-MIDI lifting.
    pub fn validate(&self) -> Result<()> {
        if self.sample_rate == 0 {
            return Err(Error::Eval(
                "stream/bridge lift-midi sample_rate must be greater than zero".to_owned(),
            ));
        }
        if self.tpq == 0 {
            return Err(Error::Eval(
                "stream/bridge lift-midi tpq must be greater than zero".to_owned(),
            ));
        }
        if self.us_per_quarter == 0 {
            return Err(Error::Eval(
                "stream/bridge lift-midi us_per_quarter must be greater than zero".to_owned(),
            ));
        }
        if !self.min_confidence.is_finite() || !(0.0..=1.0).contains(&self.min_confidence) {
            return Err(Error::Eval(
                "stream/bridge lift-midi min_confidence must be between 0 and 1".to_owned(),
            ));
        }
        if self.window_size == 0 {
            return Err(Error::Eval(
                "stream/bridge lift-midi window_size must be greater than zero".to_owned(),
            ));
        }
        if self.hop_size == 0 {
            return Err(Error::Eval(
                "stream/bridge lift-midi hop_size must be greater than zero".to_owned(),
            ));
        }
        if self.max_events_per_packet == 0 {
            return Err(Error::Eval(
                "stream/bridge lift-midi max_events_per_packet must be greater than zero"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

impl Default for StreamBridgeLiftMidiOptions {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            tpq: 480,
            us_per_quarter: DEFAULT_US_PER_QUARTER,
            min_confidence: 0.75,
            window_size: 2048,
            hop_size: 512,
            max_events_per_packet: 64,
        }
    }
}

/// Returns the `stream/bridge` symbol naming the bridge function export.
///
/// # Examples
///
/// ```
/// let symbol = sim_lib_stream_bridge::stream_bridge_symbol();
/// assert_eq!(&*symbol.name, "bridge");
/// ```
pub fn stream_bridge_symbol() -> Symbol {
    Symbol::qualified("stream", "bridge")
}

/// Returns the class symbol for [`StreamBridgeRenderOptions`].
pub fn stream_bridge_render_options_class_symbol() -> Symbol {
    Symbol::qualified("stream-bridge", "RenderOptions")
}

/// Returns the class symbol for [`StreamBridgeLiftMidiOptions`].
pub fn stream_bridge_lift_midi_options_class_symbol() -> Symbol {
    Symbol::qualified("stream-bridge", "LiftMidiOptions")
}
