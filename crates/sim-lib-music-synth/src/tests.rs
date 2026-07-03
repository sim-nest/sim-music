use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol};
use sim_lib_audio_graph_core::{
    BlockArena, BlockEvent, NullEventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};
use sim_lib_audio_graph_live::{LiveGraphConfig, LiveGraphRunner};

mod cv;
mod daw;
mod dx7;
mod dx7_modeled;
mod dx7_operator;
mod dx7_patch;
mod editor;
mod fixed;
mod graph_host;
mod modulator;
mod patch;
mod poly;
mod ps3300;
mod ps3300_cell;
mod ps3300_control;
mod ps3300_tone;
mod ps3300_wrapper;
mod registry;
mod system55;
mod system55_control;
mod system55_filter;
mod system55_oscillator;
mod system55_wrapper;
mod system700;
mod system700_control;
mod system700_shaper;
mod system700_wrapper;

use crate::{
    AdsrEnvelope, AdsrSettings, AdsrStage, ClockDomain, ComponentBackend, ComponentParamUnit,
    ComponentPortMedia, ComponentPrepareConfig, DiscreteComponent, LatencyClass, Lfo, LfoSettings,
    ModSource, ModTarget, ModulationInput, ModulationMatrix, ModulationRoute, Oscillator,
    OscillatorKind, PhaseOscillator, SubtractiveSynth, SynthPreset, SynthPresetDescriptor,
    SynthVoice, TempoSync, VoiceAllocator, VoiceState, assert_backend_surface_identity,
    audio_synth_stream_profile_symbol, audio_synth_symbols, component_port_media_symbols,
    install_audio_synth_lib, midi_key_to_hz, r31_synth_note_fixture, render_synth_offline,
    subtractive_synth_backend_surfaces, subtractive_synth_component_id, subtractive_synth_params,
    subtractive_synth_ports,
};

fn assert_processor<T: Processor>() {}

fn assert_oscillator<T: Oscillator>() {}

fn assert_discrete_component<T: DiscreteComponent>() {}

#[test]
fn public_synth_types_implement_graph_traits() {
    assert_processor::<SubtractiveSynth>();
    assert_oscillator::<PhaseOscillator>();
    assert_discrete_component::<SubtractiveSynth>();
}

#[test]
fn discrete_component_surface_declares_ports_and_params() {
    let ports = subtractive_synth_ports();
    assert_eq!(
        ports.iter().map(|port| port.media()).collect::<Vec<_>>(),
        vec![
            ComponentPortMedia::Event,
            ComponentPortMedia::Gate,
            ComponentPortMedia::ControlRate,
            ComponentPortMedia::Metadata,
            ComponentPortMedia::AudioRate,
            ComponentPortMedia::Trace,
        ]
    );
    assert_eq!(
        ports[0].rate_contract().clock_domain(),
        ClockDomain::MidiTick
    );
    assert_eq!(
        ports[0].rate_contract().latency_class(),
        LatencyClass::Interactive
    );
    assert_eq!(ports[4].rate_contract().clock_domain(), ClockDomain::Sample);
    assert_eq!(
        ports[4].rate_contract().latency_class(),
        LatencyClass::SampleExact
    );
    assert_eq!(
        component_port_media_symbols()
            .into_iter()
            .map(|symbol| symbol.as_qualified_str())
            .collect::<Vec<_>>(),
        vec![
            "audio-synth/port-media/audio-rate",
            "audio-synth/port-media/control-voltage",
            "audio-synth/port-media/control-rate",
            "audio-synth/port-media/event",
            "audio-synth/port-media/metadata",
            "audio-synth/port-media/gate",
            "audio-synth/port-media/trace",
        ]
    );

    let params = subtractive_synth_params();
    let amp_gain = params
        .iter()
        .find(|param| param.id().as_qualified_str() == "audio-synth/param/amp-gain")
        .expect("amp gain param");
    assert_eq!(amp_gain.unit(), ComponentParamUnit::Unitless);
    assert_eq!(amp_gain.range().unwrap().min(), 0.0);
    assert_eq!(amp_gain.range().unwrap().max(), 1.0);
    assert_eq!(amp_gain.normalized_default(), 0.25);

    let oscillator = params
        .iter()
        .find(|param| param.id().as_qualified_str() == "audio-synth/param/oscillator")
        .expect("oscillator param");
    assert_eq!(oscillator.enum_values().len(), 7);

    let voices = params
        .iter()
        .find(|param| param.id().as_qualified_str() == "audio-synth/param/max-voices")
        .expect("max voices param");
    assert_eq!(voices.unit(), ComponentParamUnit::RawInteger);
    assert_eq!(voices.raw_default(), Some(8));
    assert_eq!(voices.normalize_raw(1), Some(0.0));
}

#[test]
fn component_descriptor_reports_domain_latency_and_pin() {
    let component = SubtractiveSynth::default();
    let descriptor = DiscreteComponent::descriptor(&component);

    assert_eq!(descriptor.component(), &subtractive_synth_component_id());
    assert_eq!(descriptor.backend(), ComponentBackend::Algorithmic);
    assert_eq!(descriptor.clock_domain(), ClockDomain::Sample);
    assert_eq!(descriptor.latency_class(), LatencyClass::BlockLocal);
    assert!(descriptor.realtime_pin());
    assert_eq!(descriptor.ports().len(), subtractive_synth_ports().len());
    assert_eq!(descriptor.params().len(), subtractive_synth_params().len());
}

#[test]
fn component_backends_share_ports_and_params() {
    let [algorithmic, modeled] = subtractive_synth_backend_surfaces();
    assert_eq!(algorithmic.backend(), ComponentBackend::Algorithmic);
    assert_eq!(modeled.backend(), ComponentBackend::Modeled);
    assert_backend_surface_identity(&algorithmic, &modeled).unwrap();
    assert_eq!(algorithmic.ports(), modeled.ports());
    assert_eq!(algorithmic.params(), modeled.params());
}

#[test]
fn discrete_component_render_is_deterministic_after_reset() {
    let preset = SynthPreset {
        oscillator: OscillatorKind::Sine,
        amp_envelope: AdsrSettings {
            attack_s: 0.0,
            decay_s: 0.0,
            sustain_level: 1.0,
            release_s: 0.0,
        },
        amp_gain: 0.5,
        filter_cutoff_hz: 20_000.0,
        ..SynthPreset::default()
    };
    let events = [BlockEvent::NoteOn {
        offset: 0,
        channel: 0,
        key: 69,
        velocity: 1.0,
    }];
    let mut component = SubtractiveSynth::new(preset);
    let first = render_discrete_component(&mut component, &events, 32);
    DiscreteComponent::reset(&mut component);
    let second = render_discrete_component(&mut component, &events, 32);
    assert_eq!(interleave(&first), interleave(&second));

    let inspection = component.inspect();
    assert_eq!(inspection.component(), &subtractive_synth_component_id());
    assert_eq!(inspection.backend(), ComponentBackend::Algorithmic);
    assert!(inspection.active());
    assert!(component.trace().unwrap().records().len() >= 2);
}

#[test]
fn f32_offline_render_helper_stays_compatible_with_component_surface() {
    let fixture = r31_synth_note_fixture();
    let output = render_synth_offline(&fixture);
    let peak = peak(&output[0]);
    assert!(peak >= fixture.expected_peak_range.0 && peak <= fixture.expected_peak_range.1);
    assert_eq!(
        subtractive_synth_component_id().as_qualified_str(),
        "audio-synth/SubtractiveSynth"
    );
}

#[test]
fn oscillator_family_is_bounded_and_deterministic() {
    for kind in [
        OscillatorKind::Sine,
        OscillatorKind::Saw,
        OscillatorKind::Square,
        OscillatorKind::Triangle,
        OscillatorKind::PolyBlepSaw,
        OscillatorKind::PolyBlepSquare,
        OscillatorKind::Wavetable,
    ] {
        let mut osc = if kind == OscillatorKind::Wavetable {
            PhaseOscillator::wavetable(100.0, vec![-1.0, 0.0, 1.0, 0.0])
        } else {
            PhaseOscillator::new(kind, 100.0)
        };
        osc.set_sample_rate(1_000.0);
        let first = (0..16).map(|_| osc.next_sample()).collect::<Vec<_>>();
        osc.reset();
        let second = (0..16).map(|_| osc.next_sample()).collect::<Vec<_>>();
        assert_eq!(round6(&first), round6(&second), "{kind:?}");
        assert!(
            first
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() <= 1.0001),
            "{kind:?}"
        );
    }
}

#[test]
fn adsr_envelope_runs_all_stages() {
    let mut env = AdsrEnvelope::new(AdsrSettings {
        attack_s: 0.2,
        decay_s: 0.2,
        sustain_level: 0.5,
        release_s: 0.2,
    });
    env.set_sample_rate(10.0);
    env.note_on();
    assert_eq!(
        round6(&[env.next_sample(), env.next_sample()]),
        vec![0.5, 1.0]
    );
    assert_eq!(env.stage(), AdsrStage::Decay);
    assert_eq!(
        round6(&[env.next_sample(), env.next_sample()]),
        vec![0.75, 0.5]
    );
    assert_eq!(env.stage(), AdsrStage::Sustain);
    env.note_off();
    assert_eq!(
        round6(&[env.next_sample(), env.next_sample()]),
        vec![0.25, 0.0]
    );
    assert!(env.is_idle());
}

#[test]
fn lfo_can_sync_to_tempo() {
    let mut lfo = Lfo::new(LfoSettings {
        waveform: OscillatorKind::Sine,
        rate_hz: 0.25,
        depth: 1.0,
        tempo_sync: Some(TempoSync {
            beats_per_cycle: 1.0,
        }),
    });
    lfo.set_sample_rate(8.0);
    lfo.set_tempo_bpm(120.0);
    assert_eq!(
        round6(&[lfo.next_sample(), lfo.next_sample()]),
        vec![0.0, 1.0]
    );
}

#[test]
fn modulation_matrix_routes_sources_to_targets() {
    let matrix = ModulationMatrix::new(vec![
        ModulationRoute {
            source: ModSource::Lfo1,
            target: ModTarget::OscPitchSemitones,
            amount: 2.0,
        },
        ModulationRoute {
            source: ModSource::Velocity,
            target: ModTarget::AmpGain,
            amount: 0.25,
        },
    ]);
    let output = matrix.apply(ModulationInput {
        lfo1: 0.5,
        velocity: 0.8,
        constant: 1.0,
        ..ModulationInput::default()
    });
    assert_eq!(output.osc_pitch_semitones, 1.0);
    assert_eq!(output.amp_gain, 0.2);
}

#[test]
fn voice_allocator_tracks_release_and_stealing() {
    let preset = SynthPreset {
        max_voices: 2,
        ..SynthPreset::default()
    };
    let mut allocator = VoiceAllocator::new();
    let mut voices = vec![SynthVoice::idle(), SynthVoice::idle()];
    allocator.note_on(&mut voices, 0, 60, 1.0, &preset, 48_000.0);
    allocator.note_on(&mut voices, 0, 64, 1.0, &preset, 48_000.0);
    allocator.note_on(&mut voices, 0, 67, 1.0, &preset, 48_000.0);

    assert_eq!(
        voices
            .iter()
            .filter(|voice| voice.state() == VoiceState::Active)
            .count(),
        2
    );
    assert!(voices.iter().any(|voice| voice.key() == 67));

    allocator.note_off(&mut voices, 0, 64);
    assert!(
        voices
            .iter()
            .any(|voice| voice.state() == VoiceState::Released)
    );
}

#[test]
fn midi_key_frequency_uses_pitch_core_mapping() {
    assert_eq!(round6(&[midi_key_to_hz(69)]), vec![440.0]);
}

#[test]
fn presets_round_trip_as_expr() {
    let mut modulation = ModulationMatrix::default();
    modulation.push(ModulationRoute {
        source: ModSource::Lfo1,
        target: ModTarget::FilterCutoffHz,
        amount: 250.0,
    });
    let preset = SynthPreset {
        name: "expr-round-trip".to_owned(),
        oscillator: OscillatorKind::Wavetable,
        wavetable: vec![-1.0, -0.25, 0.5, 1.0],
        lfo: LfoSettings {
            waveform: OscillatorKind::Triangle,
            rate_hz: 3.0,
            depth: 0.5,
            tempo_sync: Some(TempoSync {
                beats_per_cycle: 2.0,
            }),
        },
        modulation,
        max_voices: 3,
        ..SynthPreset::default()
    };
    let expr = preset.to_expr();
    assert_eq!(SynthPreset::from_expr(&expr).expect("preset"), preset);
}

#[test]
fn citizen_synth_preset_descriptor_round_trips_and_fails_closed() {
    let descriptor = SynthPresetDescriptor::new(SynthPreset::mono_polyblep_lead());
    assert_eq!(
        descriptor.preset().unwrap(),
        SynthPreset::mono_polyblep_lead()
    );

    let mut expr = descriptor.as_expr().clone();
    let sim_kernel::Expr::Map(entries) = &mut expr else {
        panic!("preset descriptor should be a map");
    };
    for (key, value) in entries {
        if key
            == &sim_kernel::Expr::Symbol(sim_kernel::Symbol::qualified("audio-synth", "oscillator"))
        {
            *value = sim_kernel::Expr::Symbol(sim_kernel::Symbol::qualified(
                "audio-synth",
                "not-an-oscillator",
            ));
        }
    }
    let err = SynthPresetDescriptor::from_expr(expr).unwrap_err();
    assert!(format!("{err}").contains("unknown audio synth oscillator"));
}

#[test]
fn subtractive_synth_responds_to_note_on_and_note_off() {
    let preset = SynthPreset {
        oscillator: OscillatorKind::Sine,
        amp_envelope: AdsrSettings {
            attack_s: 0.0,
            decay_s: 0.0,
            sustain_level: 1.0,
            release_s: 0.0,
        },
        amp_gain: 0.5,
        filter_cutoff_hz: 20_000.0,
        ..SynthPreset::default()
    };
    let events = [
        BlockEvent::NoteOn {
            offset: 0,
            channel: 0,
            key: 69,
            velocity: 1.0,
        },
        BlockEvent::NoteOff {
            offset: 16,
            channel: 0,
            key: 69,
            velocity: 0.0,
        },
    ];
    let mut synth = SubtractiveSynth::new(preset);
    let output = process_synth(&mut synth, &events, 32);

    assert!(peak(&output[0][..16]) > 0.1);
    assert!(output[0][16..].iter().all(|sample| sample.abs() <= 0.0001));
}

#[test]
fn offline_render_fixture_is_deterministic() {
    let fixture = r31_synth_note_fixture();
    let first = render_synth_offline(&fixture);
    let second = render_synth_offline(&fixture);
    assert_eq!(round6(&first[0]), round6(&second[0]));
    let peak = peak(&first[0]);
    assert!(
        peak >= fixture.expected_peak_range.0 && peak <= fixture.expected_peak_range.1,
        "{peak}"
    );
}

#[test]
fn live_runner_and_offline_render_use_same_synth_code() {
    let preset = SynthPreset {
        oscillator: OscillatorKind::Sine,
        amp_envelope: AdsrSettings {
            attack_s: 0.0,
            decay_s: 0.0,
            sustain_level: 1.0,
            release_s: 0.0,
        },
        amp_gain: 0.25,
        filter_cutoff_hz: 20_000.0,
        ..SynthPreset::default()
    };
    let mut offline_synth = SubtractiveSynth::new(preset.clone());
    let offline = process_synth(
        &mut offline_synth,
        &[BlockEvent::Midi {
            offset: 0,
            bytes: [0x90, 60, 100],
            len: 3,
        }],
        32,
    );

    let mut runner = LiveGraphRunner::new(
        SubtractiveSynth::new(preset),
        LiveGraphConfig::stereo(48_000, 32).unwrap(),
    )
    .unwrap();
    runner.enqueue_midi_short(0, &[0x90, 60, 100]).unwrap();
    let mut live_output = [0.0; 64];
    runner
        .process_interleaved_f32(None, &mut live_output, 32, Transport::default())
        .unwrap();

    assert_eq!(interleave(&offline), round6(&live_output));
}

#[test]
fn install_audio_synth_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::assert_lib_exports(
        &mut cx,
        install_audio_synth_lib,
        &Symbol::new("audio-synth"),
        &audio_synth_symbols(),
    );
}

#[test]
fn synth_runtime_cards_advertise_realtime_audio_profile() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_audio_synth_lib(&mut cx).expect("install");
    let value = cx
        .registry()
        .value_by_symbol(&Symbol::qualified("audio-synth", "SubtractiveSynth"))
        .cloned()
        .expect("subtractive synth card");
    let expr = value.object().as_expr(&mut cx).unwrap();

    assert_eq!(
        table_value(&expr, "stream-profile"),
        Some(&Expr::Symbol(audio_synth_stream_profile_symbol()))
    );
}

fn process_synth(
    synth: &mut SubtractiveSynth,
    events: &[BlockEvent<'_>],
    frames: usize,
) -> Vec<Vec<f32>> {
    Processor::prepare(synth, PrepareConfig::new(48_000, frames as u32, 0, 2));
    let mut output = vec![vec![0.0; frames]; 2];
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(Vec::as_mut_slice).collect();
    let mut sink = NullEventSink;
    let mut scratch = BlockArena::with_f32_capacity(frames * 2);
    let mut block = ProcessBlock {
        frames: frames as u32,
        in_audio: &[],
        out_audio: &mut output_refs,
        in_events: events,
        out_events: &mut sink,
        transport: Transport::default(),
        scratch: &mut scratch,
    };
    synth.process(&mut block);
    output
}

fn render_discrete_component(
    component: &mut dyn DiscreteComponent,
    events: &[BlockEvent<'_>],
    frames: usize,
) -> Vec<Vec<f32>> {
    component.prepare(ComponentPrepareConfig::new(48_000, frames as u32, 0, 2));
    let mut output = vec![vec![0.0; frames]; 2];
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(Vec::as_mut_slice).collect();
    let mut sink = NullEventSink;
    let mut scratch = BlockArena::with_f32_capacity(frames * 2);
    let mut block = ProcessBlock {
        frames: frames as u32,
        in_audio: &[],
        out_audio: &mut output_refs,
        in_events: events,
        out_events: &mut sink,
        transport: Transport::default(),
        scratch: &mut scratch,
    };
    component.render(&mut block);
    output
}

fn table_value<'a>(expr: &'a Expr, key: &str) -> Option<&'a Expr> {
    let Expr::Map(entries) = expr else {
        return None;
    };
    entries
        .iter()
        .find_map(|(entry_key, value)| match entry_key {
            Expr::Symbol(symbol) if symbol.namespace.is_none() && symbol.name.as_ref() == key => {
                Some(value)
            }
            _ => None,
        })
}

fn round6(values: &[f32]) -> Vec<f32> {
    values
        .iter()
        .map(|value| (value * 1_000_000.0).round() / 1_000_000.0)
        .collect()
}

fn peak(values: &[f32]) -> f32 {
    values.iter().copied().map(f32::abs).fold(0.0, f32::max)
}

fn interleave(output: &[Vec<f32>]) -> Vec<f32> {
    let frames = output.first().map_or(0, Vec::len);
    let mut interleaved = Vec::with_capacity(frames * output.len());
    for frame in 0..frames {
        for channel in output {
            interleaved.push((channel[frame] * 1_000_000.0).round() / 1_000_000.0);
        }
    }
    interleaved
}
