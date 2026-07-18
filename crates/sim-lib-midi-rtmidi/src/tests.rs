use sim_kernel::{Expr, Symbol};
use sim_lib_midi_core::{
    Channel, ChannelMessage, MetaEvent, MidiPayload, MidiSink, MidiSource, RawBytes, SysExEvent,
};
use sim_lib_stream_core::{BufferPolicy, StreamMedia};
use sim_lib_stream_host::{HostBackend, HostDirection, HostStreamConfigRequest};

use crate::{
    RTMIDI_ALSA_SEQ_MIDI_BACKEND_CANDIDATE, RtmidiBackend, RtmidiEvent, RtmidiMidiSink,
    RtmidiTiming, bytes_from_payload, missing_rtmidi_dependency_card, payload_from_bytes,
    rtmidi_backend_symbol, rtmidi_midi_backend_candidates,
};

#[cfg(feature = "rtmidi-hardware")]
use crate::{
    AlsaMidiProvider, CoreMidiProvider, FixtureRtmidiProvider, RtmidiHardwareConfig,
    RtmidiInputSource, RtmidiOutputSink, RtmidiPort, RtmidiProvider, WinMmProvider, input_ring,
};
#[cfg(feature = "rtmidi-hardware")]
use sim_lib_stream_host::{DeviceKind, DeviceProvider, Placement};

#[test]
fn rtmidi_lists_and_opens_fake_ports_without_hardware() {
    let backend = RtmidiBackend::fake();
    let inventory = backend.enumerate().unwrap();
    assert_eq!(inventory.backend(), &rtmidi_backend_symbol());
    assert!(inventory.devices().iter().any(|device| {
        device.media() == StreamMedia::Midi && device.direction() == HostDirection::Input
    }));

    let request = HostStreamConfigRequest::new(
        rtmidi_backend_symbol(),
        Symbol::new("rtmidi/fake-in"),
        StreamMedia::Midi,
        HostDirection::Input,
        BufferPolicy::bounded(8).unwrap(),
    );
    let opened = backend.open(request).unwrap();
    assert_eq!(opened.config().device(), &Symbol::new("rtmidi/fake-in"));
}

#[test]
fn config_probe_candidate_names_rtmidi_backends() {
    let candidates = rtmidi_midi_backend_candidates();
    assert_eq!(candidates[0], RTMIDI_ALSA_SEQ_MIDI_BACKEND_CANDIDATE);
    assert_eq!(candidates, ["alsa-seq", "coremidi", "winmm"]);
}

#[test]
fn rtmidi_input_source_converts_timestamps_and_payloads() {
    let timing = RtmidiTiming::new(480, 500_000).unwrap();
    let backend = RtmidiBackend::fake()
        .with_timing(timing)
        .with_input_events(
            &Symbol::new("rtmidi/fake-in"),
            vec![RtmidiEvent::new(250_000, vec![0x90, 60, 100])],
        )
        .unwrap();
    let mut source = backend
        .open_midi_source(&Symbol::new("rtmidi/fake-in"))
        .unwrap();
    let event = source.next().unwrap().unwrap();

    assert_eq!(event.time.ticks, 240);
    assert_eq!(event.time.tpq, 480);
    assert_eq!(
        event.payload,
        MidiPayload::Channel(ChannelMessage::NoteOn {
            ch: Channel(0),
            key: sim_lib_midi_core::U7(60),
            vel: sim_lib_midi_core::U7(100),
        })
    );
}

#[test]
fn rtmidi_output_sink_is_a_midi_sink() {
    let mut sink = RtmidiMidiSink::new(960).unwrap();
    let mut source = RtmidiBackend::fake()
        .with_input_events(
            &Symbol::new("rtmidi/fake-in"),
            vec![RtmidiEvent::new(0, vec![0x80, 61, 64])],
        )
        .unwrap()
        .open_midi_source(&Symbol::new("rtmidi/fake-in"))
        .unwrap();
    let event = source.next().unwrap().unwrap();

    sink.write(&event).unwrap();
    sink.flush().unwrap();

    assert_eq!(sink.events().len(), 1);
    assert!(sink.flushed());
}

#[test]
fn rtmidi_raw_parser_keeps_unknown_status_as_raw() {
    let payload = payload_from_bytes(&[0xf2, 1, 2]).unwrap();
    assert!(matches!(payload, MidiPayload::Raw(_)));
}

#[test]
fn rtmidi_payload_encoder_emits_channel_and_raw_bytes() {
    let note = MidiPayload::Channel(ChannelMessage::NoteOn {
        ch: Channel(0),
        key: sim_lib_midi_core::U7(60),
        vel: sim_lib_midi_core::U7(100),
    });
    assert_eq!(bytes_from_payload(&note).unwrap(), vec![0x90, 60, 100]);

    let raw = MidiPayload::Raw(RawBytes {
        status: 0xf2,
        data: vec![1, 2],
    });
    assert_eq!(bytes_from_payload(&raw).unwrap(), vec![0xf2, 1, 2]);
}

#[test]
fn rtmidi_sysex_round_trips_as_sysex_payload() {
    let f0 = MidiPayload::SysEx(SysExEvent::F0 {
        data: vec![0x7e, 0x7f],
    });
    let f0_bytes = bytes_from_payload(&f0).unwrap();
    assert_eq!(payload_from_bytes(&f0_bytes).unwrap(), f0);

    let f7 = MidiPayload::SysEx(SysExEvent::F7 {
        data: vec![0x01, 0x02],
    });
    let f7_bytes = bytes_from_payload(&f7).unwrap();
    assert_eq!(payload_from_bytes(&f7_bytes).unwrap(), f7);
}

#[test]
fn rtmidi_payload_encoder_rejects_meta_events() {
    let payload = MidiPayload::Meta(MetaEvent::EndOfTrack);
    assert!(bytes_from_payload(&payload).is_err());
}

#[test]
fn rtmidi_missing_dependency_card_is_structured() {
    let card = missing_rtmidi_dependency_card();
    assert_eq!(
        field(&card, "kind"),
        Some(&Expr::Symbol(Symbol::qualified(
            "stream",
            "host-missing-capability"
        )))
    );
}

#[cfg(feature = "rtmidi-hardware")]
#[test]
fn fixture_providers_enumerate_hardware_placement() {
    let provider = AlsaMidiProvider::from_fixture(fixture_provider("alsa", "ALSA")).unwrap();
    assert_hardware_provider(&provider, "alsa-seq");

    let provider =
        CoreMidiProvider::from_fixture(fixture_provider("coremidi", "CoreMIDI")).unwrap();
    assert_hardware_provider(&provider, "coremidi");

    let provider = WinMmProvider::from_fixture(fixture_provider("winmm", "WinMM")).unwrap();
    assert_hardware_provider(&provider, "winmm");
}

#[cfg(feature = "rtmidi-hardware")]
#[test]
fn fixture_providers_open_stream_eval_sites() {
    let provider = AlsaMidiProvider::from_fixture(fixture_provider("alsa", "ALSA")).unwrap();
    assert_hardware_provider_opens(&provider, "alsa", "ALSA");

    let provider =
        CoreMidiProvider::from_fixture(fixture_provider("coremidi", "CoreMIDI")).unwrap();
    assert_hardware_provider_opens(&provider, "coremidi", "CoreMIDI");

    let provider = WinMmProvider::from_fixture(fixture_provider("winmm", "WinMM")).unwrap();
    assert_hardware_provider_opens(&provider, "winmm", "WinMM");
}

#[cfg(feature = "rtmidi-hardware")]
#[test]
fn fixture_drivers_back_midi_source_and_sink() {
    let timing = RtmidiTiming::default();
    let fixture = FixtureRtmidiProvider::new(vec![
        RtmidiPort::input("rtmidi/alsa/in-0", "ALSA Input 0", 0),
        RtmidiPort::output("rtmidi/alsa/out-0", "ALSA Output 0", 0),
    ]);
    let mut ring = input_ring(timing, 4).unwrap();
    ring.write(&note_on_fixture()).unwrap();

    let input_driver = fixture
        .open_input(&Symbol::new("rtmidi/alsa/in-0"), ring)
        .unwrap();
    let mut source = RtmidiInputSource::new(input_driver, timing);
    assert_eq!(
        source.next().unwrap().unwrap().payload,
        note_on_fixture().payload
    );

    let output_driver = fixture
        .open_output(&Symbol::new("rtmidi/alsa/out-0"))
        .unwrap();
    let mut sink = RtmidiOutputSink::new(output_driver, timing);
    sink.write(&note_on_fixture()).unwrap();
    sink.flush().unwrap();
    assert_eq!(sink.driver().messages(), &[vec![0x90, 60, 100]]);
}

#[cfg(all(feature = "rtmidi-hardware", target_os = "macos"))]
#[test]
fn coremidi_native_constructors_are_target_gated() {
    let _native: fn(RtmidiHardwareConfig) -> crate::NativeRtmidiProvider =
        crate::NativeRtmidiProvider::coremidi;
    let _provider: fn(RtmidiHardwareConfig) -> sim_kernel::Result<CoreMidiProvider> =
        CoreMidiProvider::coremidi;
}

#[cfg(all(feature = "rtmidi-hardware", target_os = "windows"))]
#[test]
fn winmm_native_constructors_are_target_gated() {
    let _native: fn(RtmidiHardwareConfig) -> crate::NativeRtmidiProvider =
        crate::NativeRtmidiProvider::winmm;
    let _provider: fn(RtmidiHardwareConfig) -> sim_kernel::Result<WinMmProvider> =
        WinMmProvider::winmm;
}

#[cfg(all(feature = "rtmidi-hardware", target_os = "linux"))]
#[test]
fn rtmidi_alsa_seq_loopback_smoke() {
    if std::env::var("SIM_RTMIDI_HARDWARE_SMOKE").as_deref() != Ok("1") {
        eprintln!("set SIM_RTMIDI_HARDWARE_SMOKE=1 to open ALSA sequencer ports");
        return;
    }
    let timing = RtmidiTiming::default();
    let input = std::env::var("SIM_RTMIDI_INPUT").unwrap_or_else(|_| "rtmidi/alsa/in-0".to_owned());
    let output =
        std::env::var("SIM_RTMIDI_OUTPUT").unwrap_or_else(|_| "rtmidi/alsa/out-0".to_owned());
    let provider = crate::NativeRtmidiProvider::alsa_seq(RtmidiHardwareConfig::new(timing));
    let ring = input_ring(timing, 64).unwrap();
    let input_driver = provider.open_input(&Symbol::new(input), ring).unwrap();
    let output_driver = provider.open_output(&Symbol::new(output)).unwrap();
    let mut source = RtmidiInputSource::new(input_driver, timing);
    let mut sink = RtmidiOutputSink::new(output_driver, timing);

    sink.write(&note_on_fixture()).unwrap();
    sink.flush().unwrap();
    let _ = source.next().unwrap();
}

#[test]
#[ignore = "hardware smoke test requires operator-provided RtMidi ports"]
fn rtmidi_hardware_smoke_test_is_ignored_by_default() {
    let backend = RtmidiBackend::default();
    let _ports = backend.list_ports();
}

#[cfg(feature = "rtmidi-hardware")]
fn note_on_fixture() -> sim_lib_midi_core::MidiEvent {
    sim_lib_midi_core::MidiEvent {
        time: sim_lib_midi_core::TickTime::new(0, 960).unwrap(),
        origin: sim_lib_midi_core::synthetic_origin(),
        payload: MidiPayload::Channel(ChannelMessage::NoteOn {
            ch: Channel(0),
            key: sim_lib_midi_core::U7(60),
            vel: sim_lib_midi_core::U7(100),
        }),
    }
}

#[cfg(feature = "rtmidi-hardware")]
fn fixture_provider(id_transport: &str, label: &str) -> FixtureRtmidiProvider {
    FixtureRtmidiProvider::new(vec![
        RtmidiPort::input(
            format!("rtmidi/{id_transport}/in-0"),
            format!("{label} Input 0"),
            0,
        ),
        RtmidiPort::output(
            format!("rtmidi/{id_transport}/out-0"),
            format!("{label} Output 0"),
            0,
        ),
    ])
}

#[cfg(feature = "rtmidi-hardware")]
fn assert_hardware_provider(provider: &dyn DeviceProvider, transport: &str) {
    let records = provider.enumerate().unwrap();

    assert_eq!(records.len(), 2);
    for record in records {
        assert_eq!(record.kind, DeviceKind::Midi);
        assert_eq!(
            record.placement,
            Placement::Hardware {
                transport: Symbol::new(transport)
            }
        );
    }
}

#[cfg(feature = "rtmidi-hardware")]
fn assert_hardware_provider_opens(provider: &dyn DeviceProvider, id_transport: &str, label: &str) {
    let input_id = Symbol::new(format!("rtmidi/{id_transport}/in-0"));
    let output_id = Symbol::new(format!("rtmidi/{id_transport}/out-0"));
    let input = provider.open(&input_id).unwrap();
    let output = provider.open(&output_id).unwrap();

    assert_eq!(
        input.device_record().display_name,
        format!("{label} Input 0")
    );
    assert_eq!(
        output.device_record().display_name,
        format!("{label} Output 0")
    );
    assert!(matches!(input.placement(), Placement::Hardware { .. }));
    assert!(matches!(output.placement(), Placement::Hardware { .. }));
    input.close().unwrap();
    output.close().unwrap();
}

fn field<'a>(expr: &'a Expr, name: &str) -> Option<&'a Expr> {
    let Expr::Map(entries) = expr else {
        return None;
    };
    entries.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol) if symbol.namespace.is_none() && symbol.name.as_ref() == name => {
            Some(value)
        }
        _ => None,
    })
}
