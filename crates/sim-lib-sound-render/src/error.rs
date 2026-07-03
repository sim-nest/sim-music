use thiserror::Error;

/// Error raised by sound rendering and WAV encoding.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SoundRenderError {
    /// The sample rate was zero.
    #[error("sample rate must be positive")]
    InvalidSampleRate,
    /// The channel count was neither mono (1) nor stereo (2).
    #[error("renderer only supports mono or stereo output")]
    InvalidChannelCount,
    /// The audio buffer exceeded the size encodable in a WAV header.
    #[error("audio buffer too large to encode")]
    BufferTooLarge,
}
