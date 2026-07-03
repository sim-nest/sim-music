use sim_kernel::{Diagnostic, Error, Result, Severity, Symbol};
use sim_lib_midi_core::{
    Channel, ChannelMessage, MemoryMidiSource, MidiEvent, MidiPayload, TickTime, U7,
    synthetic_origin,
};
use sim_lib_sound_audio_lift::{
    AudioLiftOptions, AudioLifter, AudioNoteCandidate, HarmonicCombLifter,
};
use sim_lib_sound_tuning::EqualTemperament;
use sim_lib_stream_core::{
    BufferOverflowPolicy, BufferPolicy, StreamDiagnostic, StreamDirection, StreamItem, StreamMedia,
    StreamMetadata, StreamPacket, StreamValue,
};
use sim_lib_stream_midi::packetize_midi_source;

use crate::{BridgeOutput, StreamBridgeLiftMidiOptions};

/// Drains a PCM stream and lifts it into a MIDI stream of detected notes.
///
/// Convenience wrapper that pulls every packet from `stream` and forwards to
/// [`lift_pcm_items_to_midi`].
pub fn lift_pcm_stream_to_midi(
    stream: &StreamValue,
    options: StreamBridgeLiftMidiOptions,
) -> Result<BridgeOutput> {
    lift_pcm_items_to_midi(take_stream_items(stream)?, options)
}

/// Lifts a collection of PCM stream items into a MIDI stream of detected notes.
///
/// Down-mixes the PCM packets to mono, runs the harmonic-comb lifter, and
/// packetizes the detected notes as a pull-mode MIDI [`StreamValue`]. Returns an
/// error when any option in `options` is zero or the input is not PCM.
pub fn lift_pcm_items_to_midi(
    items: Vec<StreamItem>,
    options: StreamBridgeLiftMidiOptions,
) -> Result<BridgeOutput> {
    if options.sample_rate == 0 {
        return Err(Error::Eval(
            "stream/bridge lift-midi sample_rate must be greater than zero".to_owned(),
        ));
    }
    if options.tpq == 0 {
        return Err(Error::Eval(
            "stream/bridge lift-midi tpq must be greater than zero".to_owned(),
        ));
    }
    if options.max_events_per_packet == 0 {
        return Err(Error::Eval(
            "stream/bridge lift-midi max_events_per_packet must be greater than zero".to_owned(),
        ));
    }
    let samples = downmixed_pcm(items)?;
    let lifter = HarmonicCombLifter {
        opts: AudioLiftOptions {
            window_size: options.window_size,
            hop_size: options.hop_size,
            min_note_confidence: options.min_confidence,
            ..AudioLiftOptions::default()
        },
    };
    let report = lifter
        .lift_report(&samples, options.sample_rate, &EqualTemperament::default())
        .map_err(|err| Error::Eval(format!("stream/bridge lift-midi error: {err}")))?;
    let mut diagnostics = report.diagnostics;
    let mut packet_items = midi_items(&report.value.notes, &options, &mut diagnostics)?;
    packet_items.extend(diagnostic_items(&diagnostics));
    Ok(BridgeOutput {
        stream: StreamValue::pull(lift_metadata()?, packet_items),
        diagnostics,
    })
}

fn take_stream_items(stream: &StreamValue) -> Result<Vec<StreamItem>> {
    let mut out = Vec::new();
    while let Some(item) = stream.next_packet()? {
        out.push(item);
    }
    Ok(out)
}

fn downmixed_pcm(items: Vec<StreamItem>) -> Result<Vec<f32>> {
    let mut channels = None;
    let mut samples = Vec::new();
    for item in items {
        match item.packet() {
            StreamPacket::Pcm(packet) => {
                if channels
                    .replace(packet.channels())
                    .is_some_and(|prev| prev != packet.channels())
                {
                    return Err(Error::Eval(
                        "stream/bridge lift-midi requires one PCM channel count".to_owned(),
                    ));
                }
                for frame in packet.samples_i16().chunks(packet.channels()) {
                    let sum = frame.iter().map(|sample| f32::from(*sample)).sum::<f32>();
                    samples.push(sum / frame.len() as f32 / f32::from(i16::MAX));
                }
            }
            StreamPacket::Diagnostic(_) => {}
            StreamPacket::Midi(_) => {
                return Err(Error::Eval(
                    "stream/bridge lift-midi expects PCM stream packets".to_owned(),
                ));
            }
            StreamPacket::Data(_) => {
                return Err(Error::Eval(
                    "stream/bridge lift-midi expects PCM stream packets".to_owned(),
                ));
            }
        }
    }
    Ok(samples)
}

fn midi_items(
    notes: &[AudioNoteCandidate],
    options: &StreamBridgeLiftMidiOptions,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<StreamItem>> {
    let mut events = Vec::new();
    for note in notes {
        let Some(key) = note.pitch.to_midi() else {
            diagnostics.push(warning(format!(
                "lifted pitch {} cannot be represented as MIDI",
                note.pitch.semitone()
            )));
            continue;
        };
        let Ok(key) = U7::try_from(u16::from(key)) else {
            diagnostics.push(warning(format!("lifted pitch {key} is outside MIDI range")));
            continue;
        };
        let on = samples_to_ticks(note.onset_sample, options);
        let off = samples_to_ticks(note.onset_sample + note.duration_samples, options).max(on + 1);
        let velocity = U7(((note.confidence * 127.0).round() as u8).clamp(1, 127));
        events.push(midi_event(
            on,
            u32::from(options.tpq),
            MidiPayload::Channel(ChannelMessage::NoteOn {
                ch: Channel(0),
                key,
                vel: velocity,
            }),
        )?);
        events.push(midi_event(
            off,
            u32::from(options.tpq),
            MidiPayload::Channel(ChannelMessage::NoteOff {
                ch: Channel(0),
                key,
                vel: U7(0),
            }),
        )?);
        for message in &note.diagnostics {
            diagnostics.push(warning(message.clone()));
        }
    }
    let mut source = MemoryMidiSource::new(u32::from(options.tpq), events);
    packetize_midi_source(&mut source, options.max_events_per_packet)?
        .into_iter()
        .map(|packet| Ok(StreamItem::new(StreamPacket::Midi(packet))))
        .collect()
}

fn diagnostic_items(diagnostics: &[Diagnostic]) -> Vec<StreamItem> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            StreamItem::new(StreamPacket::Diagnostic(StreamDiagnostic::new(
                diagnostic
                    .code
                    .clone()
                    .unwrap_or_else(|| Symbol::qualified("stream/bridge", "diagnostic")),
                diagnostic.message.clone(),
            )))
        })
        .collect()
}

fn lift_metadata() -> Result<StreamMetadata> {
    Ok(StreamMetadata::new(
        Symbol::qualified("stream/bridge", "lift-midi"),
        StreamMedia::Midi,
        StreamDirection::Source,
        Symbol::qualified("stream/clock", "midi"),
        BufferPolicy::bounded_with_overflow(64, BufferOverflowPolicy::Error)?,
    ))
}

fn samples_to_ticks(samples: usize, options: &StreamBridgeLiftMidiOptions) -> i64 {
    let seconds = samples as f64 / f64::from(options.sample_rate);
    let quarters = seconds * 1_000_000.0 / f64::from(options.us_per_quarter);
    (quarters * f64::from(options.tpq)).round() as i64
}

fn midi_event(ticks: i64, tpq: u32, payload: MidiPayload) -> Result<MidiEvent> {
    Ok(MidiEvent {
        time: TickTime::new(ticks, tpq)
            .map_err(|err| Error::Eval(format!("invalid lifted MIDI tick: {err}")))?,
        origin: synthetic_origin(),
        payload,
    })
}

fn warning(message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        message: message.into(),
        source: None,
        span: None,
        code: Some(Symbol::qualified("stream/bridge", "audio-lift")),
        related: Vec::new(),
    }
}
