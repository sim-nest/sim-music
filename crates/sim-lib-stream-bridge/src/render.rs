use sim_kernel::{Error, Result, Symbol};
use sim_lib_sound_bridge::{BridgeOptions, MidiToSoundBridge};
use sim_lib_sound_gm::general_midi_bank;
use sim_lib_sound_render::{PcmRenderer, RendererOptions};
use sim_lib_sound_tuning::TuningDescriptor;
use sim_lib_stream_core::{
    BufferOverflowPolicy, BufferPolicy, PcmPacket, StreamDirection, StreamItem, StreamMedia,
    StreamMetadata, StreamPacket, StreamValue,
};
use sim_lib_stream_midi::{midi_packet_to_events, write_midi_packets_to_sink};

use crate::{BridgeOutput, StreamBridgeRenderOptions};

/// Drains a MIDI stream and renders it into a PCM stream.
///
/// Convenience wrapper that pulls every packet from `stream` and forwards to
/// [`render_midi_items_to_pcm`].
pub fn render_midi_stream_to_pcm(
    stream: &StreamValue,
    options: StreamBridgeRenderOptions,
) -> Result<BridgeOutput> {
    render_midi_items_to_pcm(take_stream_items(stream)?, options)
}

/// Renders a collection of MIDI stream items into a PCM stream.
///
/// Feeds the MIDI packets through the general-MIDI sound bridge and PCM
/// renderer, then chunks the resulting samples into PCM packets of
/// `options.chunk_frames` frames. Returns an error when `chunk_frames` is zero
/// or the input is not MIDI.
pub fn render_midi_items_to_pcm(
    items: Vec<StreamItem>,
    options: StreamBridgeRenderOptions,
) -> Result<BridgeOutput> {
    if options.chunk_frames == 0 {
        return Err(Error::Eval(
            "stream/bridge render chunk_frames must be greater than zero".to_owned(),
        ));
    }
    let packets = midi_packets(items)?;
    let tpq = packets
        .first()
        .map(|packet| u32::from(packet.tpq()))
        .unwrap_or(480);
    let tuning = TuningDescriptor::EqualTemperament {
        divisions: 12,
        reference_midi: 69,
        reference_hz: 440.0,
    }
    .to_tuning()
    .map_err(|err| Error::Eval(format!("stream/bridge render tuning error: {err}")))?;
    let mut bridge =
        MidiToSoundBridge::new(tpq, general_midi_bank(), tuning, BridgeOptions::default())
            .map_err(|err| Error::Eval(format!("stream/bridge render setup error: {err}")))?;
    write_midi_packets_to_sink(&packets, &mut bridge)?;
    let tones = bridge.drain_tones();
    let renderer_options = RendererOptions::new(options.sample_rate, options.channels)
        .map_err(|err| Error::Eval(format!("stream/bridge render setup error: {err}")))?;
    let renderer = PcmRenderer::new(renderer_options)
        .map_err(|err| Error::Eval(format!("stream/bridge render setup error: {err}")))?;
    let samples = renderer
        .render_mix(&tones)
        .into_iter()
        .map(float_to_i16)
        .collect::<Vec<_>>();
    let items = pcm_items(
        &samples,
        usize::from(options.channels),
        options.chunk_frames,
    )?;
    Ok(BridgeOutput {
        stream: StreamValue::pull(render_metadata()?, items),
        diagnostics: Vec::new(),
    })
}

fn take_stream_items(stream: &StreamValue) -> Result<Vec<StreamItem>> {
    let mut out = Vec::new();
    while let Some(item) = stream.next_packet()? {
        out.push(item);
    }
    Ok(out)
}

fn midi_packets(items: Vec<StreamItem>) -> Result<Vec<sim_lib_stream_core::MidiPacket>> {
    let mut packets = Vec::new();
    for item in items {
        match item.packet() {
            StreamPacket::Midi(packet) => {
                let _ = midi_packet_to_events(packet)?;
                packets.push(packet.clone());
            }
            StreamPacket::Diagnostic(_) => {}
            StreamPacket::Pcm(_) => {
                return Err(Error::Eval(
                    "stream/bridge render expects MIDI stream packets".to_owned(),
                ));
            }
            StreamPacket::Data(_) => {
                return Err(Error::Eval(
                    "stream/bridge render expects MIDI stream packets".to_owned(),
                ));
            }
        }
    }
    Ok(packets)
}

fn pcm_items(samples: &[i16], channels: usize, chunk_frames: usize) -> Result<Vec<StreamItem>> {
    let samples_per_chunk = channels
        .checked_mul(chunk_frames)
        .ok_or_else(|| Error::Eval("stream/bridge render chunk size overflow".to_owned()))?;
    samples
        .chunks(samples_per_chunk)
        .map(|chunk| {
            let frames = chunk.len() / channels;
            PcmPacket::i16(channels, frames, chunk.to_vec())
                .map(StreamPacket::Pcm)
                .map(StreamItem::new)
        })
        .collect()
}

fn render_metadata() -> Result<StreamMetadata> {
    Ok(StreamMetadata::new(
        Symbol::qualified("stream/bridge", "render-pcm"),
        StreamMedia::Pcm,
        StreamDirection::Source,
        Symbol::qualified("stream/clock", "frames"),
        BufferPolicy::bounded_with_overflow(64, BufferOverflowPolicy::Error)?,
    ))
}

fn float_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}
