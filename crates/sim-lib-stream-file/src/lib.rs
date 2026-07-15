#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! File-backed source and sink adapters for STREAM 6.
//!
//! The crate keeps host filesystem access behind explicit stream file
//! capabilities: reads require `fs/read`, writes require `fs/write`, and
//! compatibility `stream.file.*` aliases are accepted by the gates. Each read or
//! write is recorded as a KERNEL 6 filesystem effect. SMF support reuses the
//! in-tree MIDI file codec; WAV support is limited to canonical little-endian
//! PCM16 RIFF/WAVE because that is the PCM packet format implemented by the
//! stream audio layer.

mod cap;
mod cassette;
mod effect_io;
mod midi;
mod wav;

pub use cap::{stream_file_read_capability, stream_file_write_capability};
pub use cassette::{
    cassette_expr_to_stream, cassette_to_stream, stream_to_cassette, stream_to_cassette_expr,
    validate_cassette_fixture_path,
};
pub use midi::{
    read_smf_stream, smf_bytes_to_stream, smf_file_to_stream, stream_to_smf_bytes,
    stream_to_smf_file, write_smf_stream,
};
pub use wav::{
    WavStream, pcm_buffers_to_wav_bytes, read_wav_stream, stream_to_wav_bytes, wav_bytes_to_stream,
    write_wav_stream,
};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
