use sim_citizen_derive::Citizen;
use sim_kernel::{Diagnostic, Symbol};
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
