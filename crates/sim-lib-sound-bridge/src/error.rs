use thiserror::Error;

/// Error raised by the MIDI-to-sound bridge and its voice pool.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SoundBridgeError {
    /// The configured polyphony limit was zero.
    #[error("polyphony must be positive")]
    ZeroPolyphony,
    /// An event arrived with a tick earlier than the last processed event.
    #[error("bridge clock moved backwards")]
    NonMonotonicTime,
}
