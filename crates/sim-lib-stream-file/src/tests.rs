use std::{
    convert::TryFrom,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use sim_kernel::{CapabilityName, Cx, DefaultFactory, Error, Expr, NoopEvalPolicy, Symbol, effect};
use sim_lib_midi_core::{
    Channel, ChannelMessage, MemoryMidiSink, MemoryMidiSource, MetaEvent, MidiEvent, MidiPayload,
    TickTime, U7, synthetic_origin,
};
use sim_lib_midi_smf::{SmfFile, SmfFormat, SmfTrack, read_smf, write_smf};
use sim_lib_stream_audio::{MemoryPcmSink, PcmBuffer, PcmSpec, stream_to_pcm_sink};
use sim_lib_stream_core::{
    BufferPolicy, PcmPacket, StreamDirection, StreamItem, StreamMedia, StreamMetadata,
    StreamPacket, StreamValue, TransportProfile,
};
use sim_lib_stream_midi::{midi_source_to_stream, midi_stream_to_sink};

use crate::{
    cassette_expr_to_stream, pcm_buffers_to_wav_bytes, read_smf_stream, read_wav_stream,
    stream_file_read_capability, stream_file_write_capability, stream_to_cassette,
    stream_to_cassette_expr, validate_cassette_fixture_path, write_smf_stream,
};

#[test]
fn smf_file_to_packet_spine_to_memory_sink_round_trips() {
    let temp = TempPath::new("input.mid");
    let file = smf_fixture();
    let bytes = write_smf(&file).unwrap();
    fs::write(temp.path(), &bytes).unwrap();
    let decoded = read_smf(&bytes).unwrap();
    let expected = merged_events(&decoded);
    let mut cx = cx(&[stream_file_read_capability()]);

    let stream =
        read_smf_stream(&mut cx, temp.path(), 2, midi_metadata("stream/smf-read")).unwrap();
    let mut sink = MemoryMidiSink::new(decoded.tpq);
    let count = midi_stream_to_sink(&stream, &mut sink).unwrap();

    assert_eq!(count, expected.len());
    assert_eq!(sink.events(), expected.as_slice());
}

#[test]
fn memory_midi_source_to_smf_to_read_back_round_trips() {
    let temp = TempPath::new("roundtrip.mid");
    let events = midi_events_with_end();
    let mut source = MemoryMidiSource::new(480, events.clone());
    let stream = midi_source_to_stream(&mut source, 3, midi_metadata("stream/smf-write")).unwrap();
    let mut cx = cx(&[
        stream_file_write_capability(),
        stream_file_read_capability(),
    ]);

    let count = write_smf_stream(&mut cx, temp.path(), &stream, 480).unwrap();
    let read_back = read_smf_stream(
        &mut cx,
        temp.path(),
        3,
        midi_metadata("stream/smf-read-back"),
    )
    .unwrap();
    let mut sink = MemoryMidiSink::new(480);
    let read_count = midi_stream_to_sink(&read_back, &mut sink).unwrap();

    assert_eq!(count, events.len());
    assert_eq!(read_count, events.len());
    assert_eq!(sink.events(), events.as_slice());
}

#[test]
fn wav_to_pcm_packet_spine_to_memory_sink_round_trips() {
    let temp = TempPath::new("input.wav");
    let spec = pcm_spec();
    let buffers = vec![pcm_buffer(&[1, -1, 2, -2]), pcm_buffer(&[3, -3])];
    let bytes = pcm_buffers_to_wav_bytes(spec, &buffers).unwrap();
    fs::write(temp.path(), bytes).unwrap();
    let mut cx = cx(&[stream_file_read_capability()]);

    let wav = read_wav_stream(&mut cx, temp.path(), 2, pcm_metadata("stream/wav-read")).unwrap();
    let mut sink = MemoryPcmSink::new(spec);
    let summary = stream_to_pcm_sink(wav.stream(), &mut sink).unwrap();

    assert_eq!(wav.spec(), spec);
    assert_eq!(summary.buffers(), 2);
    assert_eq!(summary.frames(), 3);
    assert_eq!(sink.buffers(), buffers.as_slice());
}

#[test]
fn midi_control_stream_cassette_replays_from_file_expression_format() {
    let events = midi_events_with_end();
    let mut source = MemoryMidiSource::new(480, events.clone());
    let stream =
        midi_source_to_stream(&mut source, 2, midi_metadata("stream/cassette-midi")).unwrap();

    let cassette_expr =
        stream_to_cassette_expr(&stream, TransportProfile::lan_midi_control()).unwrap();
    let replay = cassette_expr_to_stream(&cassette_expr).unwrap();
    let mut sink = MemoryMidiSink::new(480);
    let replayed = midi_stream_to_sink(&replay, &mut sink).unwrap();

    assert_eq!(replayed, events.len());
    assert_eq!(sink.events(), events.as_slice());
}

#[test]
fn buffered_pcm_preview_cassette_replays_as_golden_fixture() {
    let items = vec![
        StreamItem::new(StreamPacket::Pcm(
            PcmPacket::i16(2, 1, vec![1, -1]).unwrap(),
        )),
        StreamItem::new(StreamPacket::Pcm(
            PcmPacket::i16(2, 1, vec![2, -2]).unwrap(),
        )),
    ];
    let stream =
        sim_lib_stream_core::StreamValue::pull(pcm_metadata("stream/cassette-pcm"), items.clone());

    let cassette =
        stream_to_cassette(&stream, TransportProfile::lan_buffered_audio_preview()).unwrap();
    let report = validate_cassette_fixture_path(
        &cassette,
        "fixtures/streams/golden/buffered-preview.simcassette",
    )
    .unwrap();
    let replay = cassette.replay_stream_value().unwrap();

    assert_eq!(report.packet_count, 2);
    assert_eq!(replay.take_packets(4).unwrap(), items);
}

#[test]
fn cassette_fixture_validation_requires_redacted_sensitive_payloads() {
    let payload = Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("path")),
            Expr::String("private-path=session.mid".to_owned()),
        ),
        (
            Expr::Symbol(Symbol::new("token")),
            Expr::String("token=abc123".to_owned()),
        ),
    ]);
    let stream = StreamValue::pull(
        data_metadata("stream/sensitive-cassette"),
        vec![StreamItem::new(StreamPacket::data(
            Symbol::qualified("stream/data", "expr"),
            payload,
        ))],
    );

    let cassette = stream_to_cassette(&stream, TransportProfile::remote_stream_fabric()).unwrap();
    assert!(
        validate_cassette_fixture_path(&cassette, "fixtures/streams/golden/sensitive.simcassette")
            .is_err()
    );
    let redacted = cassette.redacted().unwrap();
    let report =
        validate_cassette_fixture_path(&redacted, "fixtures/streams/golden/sensitive.simcassette")
            .unwrap();

    assert_eq!(report.packet_count, 1);
    assert!(matches!(
        redacted.items().unwrap()[0].packet(),
        StreamPacket::Data(data)
            if data.kind == Symbol::qualified("stream/data", "redacted")
    ));
}

#[test]
fn malformed_file_returns_diagnostic_error() {
    let temp = TempPath::new("bad.mid");
    fs::write(temp.path(), b"not an smf").unwrap();
    let mut cx = cx(&[stream_file_read_capability()]);

    let err = match read_smf_stream(&mut cx, temp.path(), 1, midi_metadata("stream/bad-smf")) {
        Ok(_) => panic!("malformed SMF unexpectedly decoded"),
        Err(err) => err,
    };

    assert!(format!("{err}").contains("malformed SMF file"));
}

#[test]
fn file_effects_and_capabilities_are_recorded() {
    let temp = TempPath::new("cap.mid");
    let events = midi_events_with_end();
    let mut source = MemoryMidiSource::new(480, events);
    let stream = midi_source_to_stream(&mut source, 3, midi_metadata("stream/cap-write")).unwrap();
    let mut write_cx = cx(&[stream_file_write_capability()]);

    write_smf_stream(&mut write_cx, temp.path(), &stream, 480).unwrap();

    let records = write_cx.effect_ledger().records();
    assert_eq!(records.len(), 1);
    assert!(!records[0].aborted);
    let recorded = write_cx.effect_ledger().effect(&records[0].effect).unwrap();
    assert_eq!(recorded.kind, effect::effect_filesystem_kind());
    assert!(recorded.requires.contains(&stream_file_write_capability()));

    let mut read_cx = cx(&[]);
    let err = match read_smf_stream(
        &mut read_cx,
        temp.path(),
        1,
        midi_metadata("stream/cap-read"),
    ) {
        Ok(_) => panic!("missing read capability unexpectedly succeeded"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        Error::CapabilityDenied { capability } if capability == stream_file_read_capability()
    ));
    let denied = read_cx.effect_ledger().records();
    assert_eq!(denied.len(), 1);
    assert!(denied[0].aborted);
}

#[test]
fn compatibility_stream_file_capability_aliases_are_accepted() {
    let temp = TempPath::new("compat-caps.mid");
    let events = midi_events_with_end();
    let mut source = MemoryMidiSource::new(480, events.clone());
    let stream =
        midi_source_to_stream(&mut source, 2, midi_metadata("stream/compat-write")).unwrap();
    let mut cx = cx(&[
        CapabilityName::new("stream.file.write"),
        CapabilityName::new("stream.file.read"),
    ]);

    write_smf_stream(&mut cx, temp.path(), &stream, 480).unwrap();
    let read_back =
        read_smf_stream(&mut cx, temp.path(), 2, midi_metadata("stream/compat-read")).unwrap();
    let mut sink = MemoryMidiSink::new(480);
    let read_count = midi_stream_to_sink(&read_back, &mut sink).unwrap();

    assert_eq!(read_count, events.len());
    assert_eq!(sink.events(), events.as_slice());
}

fn cx(capabilities: &[CapabilityName]) -> Cx {
    let mut cx = Cx::new(Arc::new(NoopEvalPolicy), Arc::new(DefaultFactory));
    for capability in capabilities {
        cx.grant(capability.clone());
    }
    cx
}

fn midi_metadata(id: &str) -> StreamMetadata {
    StreamMetadata::new(
        Symbol::new(id),
        StreamMedia::Midi,
        StreamDirection::Source,
        Symbol::qualified("clock", "midi"),
        BufferPolicy::bounded(16).unwrap(),
    )
}

fn pcm_metadata(id: &str) -> StreamMetadata {
    StreamMetadata::new(
        Symbol::new(id),
        StreamMedia::Pcm,
        StreamDirection::Source,
        Symbol::qualified("clock", "pcm"),
        BufferPolicy::bounded(16).unwrap(),
    )
}

fn data_metadata(id: &str) -> StreamMetadata {
    StreamMetadata::new(
        Symbol::new(id),
        StreamMedia::Data,
        StreamDirection::Source,
        Symbol::qualified("clock", "data"),
        BufferPolicy::bounded(16).unwrap(),
    )
}

fn smf_fixture() -> SmfFile {
    SmfFile {
        format: SmfFormat::SingleTrack,
        tpq: 480,
        tracks: vec![SmfTrack {
            events: midi_events_with_end(),
        }],
    }
}

fn midi_events_with_end() -> Vec<MidiEvent> {
    vec![
        midi_event(
            0,
            MidiPayload::Channel(ChannelMessage::NoteOn {
                ch: Channel::new(0).unwrap(),
                key: U7::try_from(60).unwrap(),
                vel: U7::try_from(100).unwrap(),
            }),
        ),
        midi_event(
            240,
            MidiPayload::Channel(ChannelMessage::NoteOff {
                ch: Channel::new(0).unwrap(),
                key: U7::try_from(60).unwrap(),
                vel: U7::try_from(0).unwrap(),
            }),
        ),
        midi_event(240, MidiPayload::Meta(MetaEvent::EndOfTrack)),
    ]
}

fn midi_event(ticks: i64, payload: MidiPayload) -> MidiEvent {
    MidiEvent {
        time: TickTime::new(ticks, 480).unwrap(),
        origin: synthetic_origin(),
        payload,
    }
}

fn merged_events(file: &SmfFile) -> Vec<MidiEvent> {
    file.merged_events()
        .into_iter()
        .map(|tracked| tracked.event)
        .collect()
}

fn pcm_spec() -> PcmSpec {
    PcmSpec::i16(2, 48_000).unwrap()
}

fn pcm_buffer(samples: &[i16]) -> PcmBuffer {
    PcmBuffer::i16(pcm_spec(), samples.len() / 2, samples.to_vec()).unwrap()
}

struct TempPath {
    path: PathBuf,
}

impl TempPath {
    fn new(suffix: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sim-stream-file-{}-{nanos}-{suffix}",
            std::process::id()
        ));
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
