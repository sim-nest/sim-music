use std::sync::Arc;
use std::time::Duration;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol, Value, read_construct_capability};
use sim_lib_sound_audio_lift::{
    AudioLiftFrame, AudioLiftOptions, AudioNoteCandidate, PitchCandidate,
};
use sim_lib_sound_bridge::{BridgeOptions, TimbreBank};
use sim_lib_sound_core::{
    Amplitude, Envelope, EnvelopeShape, Frequency, Partial, PartialTag, Phase, Tone,
};
use sim_lib_sound_dissonance::DissonanceModelDescriptor;
use sim_lib_sound_render::RendererOptions;
use sim_lib_sound_spectrum::{Spectrum, SpectrumSource};
use sim_lib_sound_timbre::{
    AttackKind, Filter, Timbre, TimbreMeta, TimbreRecipe, bell_inharmonic, fm_pair, karplus_strong,
    organ_pipe, pure_sine, sawtooth, square, triangle,
};
use sim_lib_sound_tuning::{PitchClassN, TuningDescriptor, default_just_intonation};

use crate::{
    SoundEnvelopeDescriptor, SoundPartialDescriptor, SoundSpectrumDescriptor,
    SoundTimbreDescriptor, SoundToneDescriptor, SoundTuningDescriptor, decode_amplitude,
    decode_attack_kind, decode_audio_lift_frame, decode_audio_lift_options,
    decode_audio_note_candidate, decode_bridge_options, decode_dissonance_model_descriptor,
    decode_envelope, decode_envelope_shape, decode_filter, decode_frequency, decode_partial,
    decode_phase, decode_pitch_candidate, decode_pitch_class_n, decode_renderer_options,
    decode_spectrum, decode_spectrum_source, decode_timbre, decode_timbre_bank, decode_timbre_meta,
    decode_timbre_recipe, decode_tone, decode_tuning_descriptor, encode_amplitude,
    encode_attack_kind, encode_audio_lift_frame, encode_audio_lift_options,
    encode_audio_note_candidate, encode_bridge_options, encode_dissonance_model_descriptor,
    encode_envelope, encode_envelope_shape, encode_filter, encode_frequency, encode_partial,
    encode_phase, encode_pitch_candidate, encode_pitch_class_n, encode_renderer_options,
    encode_spectrum, encode_spectrum_source, encode_timbre, encode_timbre_bank, encode_timbre_meta,
    encode_timbre_recipe, encode_tone, encode_tuning_descriptor, install_sound_shapes_lib,
    sound_envelope_class_symbol, sound_partial_class_symbol, sound_spectrum_class_symbol,
    sound_timbre_class_symbol, sound_tone_class_symbol, sound_tuning_descriptor_class_symbol,
};

#[test]
fn every_public_type_round_trips() {
    let frequency = Frequency(440.0);
    assert_eq!(
        decode_frequency(&encode_frequency(frequency)).unwrap(),
        frequency
    );

    let amplitude = Amplitude(0.5);
    assert_eq!(
        decode_amplitude(&encode_amplitude(amplitude)).unwrap(),
        amplitude
    );

    let phase = Phase(1.25);
    assert_eq!(decode_phase(&encode_phase(phase)).unwrap(), phase);

    let partial = Partial {
        frequency,
        amplitude,
        phase,
        tag: PartialTag::Harmonic(3),
    };
    assert_eq!(decode_partial(&encode_partial(&partial)).unwrap(), partial);

    let shape = EnvelopeShape::Exponential(2.0);
    assert_eq!(
        decode_envelope_shape(&encode_envelope_shape(&shape)).unwrap(),
        shape
    );

    let envelope = Envelope::new(
        Duration::from_millis(10),
        Duration::from_millis(30),
        0.7,
        Duration::from_millis(80),
        EnvelopeShape::Custom("slow".to_owned()),
    )
    .unwrap();
    assert_eq!(
        decode_envelope(&encode_envelope(&envelope)).unwrap(),
        envelope
    );

    let tone =
        Tone::from_partials(vec![partial], envelope.clone(), Duration::from_secs(1)).unwrap();
    assert_eq!(decode_tone(&encode_tone(&tone)).unwrap(), tone);

    let source = SpectrumSource::FromPcm {
        window_size: 64,
        sample_rate: 8_000,
    };
    assert_eq!(
        decode_spectrum_source(&encode_spectrum_source(&source)).unwrap(),
        source
    );

    let spectrum = Spectrum {
        bins: vec![(frequency, amplitude)],
        source,
    };
    assert_eq!(
        decode_spectrum(&encode_spectrum(&spectrum)).unwrap(),
        spectrum
    );

    let attack = AttackKind::Struck;
    assert_eq!(
        decode_attack_kind(&encode_attack_kind(attack)).unwrap(),
        attack
    );

    let meta = TimbreMeta {
        brightness: 3.0,
        roughness: 0.2,
        attack_kind: AttackKind::Plucked,
        category: "string".to_owned(),
    };
    assert_eq!(
        decode_timbre_meta(&encode_timbre_meta(&meta)).unwrap(),
        meta
    );

    let filter = Filter::BandPass {
        center: frequency,
        q: 1.5,
        gain: amplitude,
    };
    assert_eq!(decode_filter(&encode_filter(&filter)).unwrap(), filter);

    let recipe = TimbreRecipe::Layered {
        primary: Box::new(TimbreRecipe::PureSine),
        secondary: Box::new(TimbreRecipe::BellInharmonic {
            ratios: vec![1.0, 2.7],
        }),
        mix: 0.4,
    };
    assert_eq!(
        decode_timbre_recipe(&encode_timbre_recipe(&recipe)).unwrap(),
        recipe
    );

    let timbre = Timbre {
        name: "combo".to_owned(),
        recipe,
        default_envelope: envelope,
        metadata: meta,
        filters: vec![filter],
    };
    assert_eq!(decode_timbre(&encode_timbre(&timbre)).unwrap(), timbre);

    let pitch_class_n = PitchClassN::new(19, 7).unwrap();
    assert_eq!(
        decode_pitch_class_n(&encode_pitch_class_n(pitch_class_n)).unwrap(),
        pitch_class_n
    );

    let just = default_just_intonation();
    let tuning = TuningDescriptor::JustIntonation {
        root: just.root.value(),
        ratios: just.ratios,
        reference_midi: 69,
        reference_hz: 440.0,
    };
    assert_eq!(
        decode_tuning_descriptor(&encode_tuning_descriptor(&tuning)).unwrap(),
        tuning
    );

    let model = DissonanceModelDescriptor::HarmonicEntropy { spread: 18.0 };
    assert_eq!(
        decode_dissonance_model_descriptor(&encode_dissonance_model_descriptor(&model)).unwrap(),
        model
    );

    let bridge = BridgeOptions::new(24, 300.0).unwrap();
    assert_eq!(
        decode_bridge_options(&encode_bridge_options(&bridge)).unwrap(),
        bridge
    );

    let renderer = RendererOptions::new(48_000, 2).unwrap();
    assert_eq!(
        decode_renderer_options(&encode_renderer_options(&renderer)).unwrap(),
        renderer
    );

    let mut bank = TimbreBank::new(pure_sine());
    bank.insert(0, 0, 40, triangle(6));
    assert_eq!(
        decode_timbre_bank(&encode_timbre_bank(&bank)).unwrap(),
        bank
    );

    let lift_opts = AudioLiftOptions::default();
    assert_eq!(
        decode_audio_lift_options(&encode_audio_lift_options(&lift_opts)).unwrap(),
        lift_opts
    );

    let candidate = PitchCandidate {
        pitch: sim_lib_pitch_core::Pitch::from_midi(69),
        frequency,
        amplitude,
        confidence: 0.88,
        cents_error: 3.5,
        harmonic_count: 2,
    };
    assert_eq!(
        decode_pitch_candidate(&encode_pitch_candidate(&candidate)).unwrap(),
        candidate
    );

    let frame = AudioLiftFrame {
        index: 0,
        onset_sample: 256,
        duration_samples: 1024,
        spectrum: spectrum.clone(),
        pitch_candidates: vec![candidate.clone()],
        diagnostics: vec!["soft warning".to_owned()],
    };
    assert_eq!(
        decode_audio_lift_frame(&encode_audio_lift_frame(&frame)).unwrap(),
        frame
    );

    let note = AudioNoteCandidate {
        track: 1,
        onset_sample: 512,
        duration_samples: 2048,
        sample_rate: 48_000,
        pitch: sim_lib_pitch_core::Pitch::from_midi(69),
        mean_frequency: frequency,
        mean_amplitude: amplitude,
        confidence: 0.81,
        diagnostics: vec!["single-frame estimate".to_owned()],
    };
    assert_eq!(
        decode_audio_note_candidate(&encode_audio_note_candidate(&note)).unwrap(),
        note
    );
}

#[test]
fn partial_read_construct_rejects_invalid_semantics() {
    assert!(decode_frequency("#(Frequency hz=nan)").is_err());
    assert!(decode_amplitude("#(Amplitude linear=-0.1)").is_err());
    assert!(decode_phase("#(Phase radians=inf)").is_err());
    assert!(
        decode_partial(
            "#(Partial frequency=#(Frequency hz=440) amplitude=#(Amplitude linear=1) phase=#(Phase radians=0) tag=#(PartialTag kind=harmonic index=0))"
        )
        .is_err()
    );
    assert!(
        decode_partial(
            "#(Partial frequency=#(Frequency hz=440) amplitude=#(Amplitude linear=1) phase=#(Phase radians=0) tag=#(PartialTag kind=source index=1))"
        )
        .is_err()
    );
    assert!(
        decode_tuning_descriptor(
            "#(TuningDescriptor kind=EqualTemperament divisions=19 reference_midi=69 reference_hz=-440)"
        )
        .is_err()
    );
}

#[test]
fn builtins_encode_and_decode() {
    let builtins = vec![
        pure_sine(),
        sawtooth(8),
        square(8),
        triangle(8),
        organ_pipe(&[1.0, 2.0, 4.0]),
        karplus_strong(0.8),
        fm_pair(2.0, 1.25),
        bell_inharmonic(&[1.0, 2.7, 5.8]),
    ];
    for timbre in builtins {
        assert_eq!(decode_timbre(&encode_timbre(&timbre)).unwrap(), timbre);
    }
}

#[test]
fn install_sound_shapes_lib_registers_audio_lift_shapes() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_sound_shapes_lib(&mut cx).unwrap();
    install_sound_shapes_lib(&mut cx).unwrap();
    assert!(
        cx.registry()
            .shape_by_symbol(&Symbol::qualified("sound", "AudioLiftFrame"))
            .is_some()
    );
    assert!(
        cx.registry()
            .shape_by_symbol(&Symbol::qualified("sound", "AudioLifter"))
            .is_some()
    );
}

#[test]
fn sound_runtime_shapes_reject_bad_domain_forms() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_sound_shapes_lib(&mut cx).unwrap();

    let frequency = registered_sound_shape(&cx, "Frequency");
    assert_shape_accepts(&mut cx, &frequency, &encode_frequency(Frequency(440.0)));
    assert!(!frequency.object().as_shape().unwrap().is_total());
    assert_shape_rejects(&mut cx, &frequency, "#(Frequency)");
    assert_shape_rejects(&mut cx, &frequency, "#(Amplitude linear=1.0)");
    assert_shape_rejects(
        &mut cx,
        &frequency,
        "#(Frequency hz=#(Amplitude linear=1.0))",
    );

    let amplitude = registered_sound_shape(&cx, "Amplitude");
    assert_shape_accepts(&mut cx, &amplitude, &encode_amplitude(Amplitude(0.5)));
    assert_shape_rejects(&mut cx, &amplitude, "#(Amplitude)");
    assert_shape_rejects(&mut cx, &amplitude, "#(Frequency hz=440.0)");
    assert_shape_rejects(
        &mut cx,
        &amplitude,
        "#(Amplitude linear=#(Frequency hz=440.0))",
    );
}

#[test]
fn sound_citizens_accept_legacy_text_and_read_construct() {
    let frequency = Frequency(440.0);
    let amplitude = Amplitude(1.0);
    let partial = Partial {
        frequency,
        amplitude,
        phase: Phase(0.0),
        tag: PartialTag::Undertone(2),
    };
    let envelope = Envelope::new(
        Duration::from_millis(10),
        Duration::from_millis(30),
        0.8,
        Duration::from_millis(80),
        EnvelopeShape::Linear,
    )
    .unwrap();
    let tone =
        Tone::from_partials(vec![partial], envelope.clone(), Duration::from_secs(1)).unwrap();
    let spectrum = Spectrum {
        bins: vec![(frequency, amplitude)],
        source: SpectrumSource::Synthetic,
    };
    let timbre = pure_sine();
    let tuning = TuningDescriptor::EqualTemperament {
        divisions: 12,
        reference_midi: 69,
        reference_hz: 440.0,
    };
    let mut cx = cx_with_citizens();

    let tone_text = encode_tone(&tone);
    let tone_descriptor =
        read_construct::<SoundToneDescriptor>(&mut cx, sound_tone_class_symbol(), &tone_text);
    assert_eq!(tone_descriptor.tone().unwrap(), tone);
    assert_eq!(
        SoundToneDescriptor::read_construct_expr_from_text(&tone_text).unwrap(),
        read_construct_expr(sound_tone_class_symbol(), tone_descriptor.as_text())
    );

    let partial_text = encode_partial(&partial);
    let partial_descriptor = read_construct::<SoundPartialDescriptor>(
        &mut cx,
        sound_partial_class_symbol(),
        &partial_text,
    );
    assert_eq!(partial_descriptor.partial().unwrap(), partial);

    let envelope_text = encode_envelope(&envelope);
    let envelope_descriptor = read_construct::<SoundEnvelopeDescriptor>(
        &mut cx,
        sound_envelope_class_symbol(),
        &envelope_text,
    );
    assert_eq!(envelope_descriptor.envelope().unwrap(), envelope);

    let spectrum_text = encode_spectrum(&spectrum);
    let spectrum_descriptor = read_construct::<SoundSpectrumDescriptor>(
        &mut cx,
        sound_spectrum_class_symbol(),
        &spectrum_text,
    );
    assert_eq!(spectrum_descriptor.spectrum().unwrap(), spectrum);

    let timbre_text = encode_timbre(&timbre);
    let timbre_descriptor =
        read_construct::<SoundTimbreDescriptor>(&mut cx, sound_timbre_class_symbol(), &timbre_text);
    assert_eq!(timbre_descriptor.timbre().unwrap(), timbre);

    let tuning_text = encode_tuning_descriptor(&tuning);
    let tuning_descriptor = read_construct::<SoundTuningDescriptor>(
        &mut cx,
        sound_tuning_descriptor_class_symbol(),
        &tuning_text,
    );
    assert_eq!(tuning_descriptor.tuning().unwrap(), tuning);
}

fn cx_with_citizens() -> Cx {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    cx.load_lib(&sim_citizen::CitizenLib::all()).unwrap();
    cx.grant(read_construct_capability());
    cx
}

fn registered_sound_shape(cx: &Cx, name: &'static str) -> Value {
    cx.registry()
        .shape_by_symbol(&Symbol::qualified("sound", name))
        .expect("registered sound shape")
        .clone()
}

fn assert_shape_accepts(cx: &mut Cx, shape: &Value, text: &str) {
    let expr = Expr::String(text.to_owned());
    let matched = shape
        .object()
        .as_shape()
        .expect("shape protocol")
        .check_expr(cx, &expr)
        .unwrap();
    assert!(
        matched.accepted,
        "{text} rejected: {:?}",
        matched.diagnostics
    );
}

fn assert_shape_rejects(cx: &mut Cx, shape: &Value, text: &str) {
    let expr = Expr::String(text.to_owned());
    let matched = shape
        .object()
        .as_shape()
        .expect("shape protocol")
        .check_expr(cx, &expr)
        .unwrap();
    assert!(
        !matched.accepted,
        "{text} unexpectedly matched with score {:?}",
        matched.score
    );
}

fn read_construct<T>(cx: &mut Cx, class: Symbol, form: &str) -> T
where
    T: Clone + 'static,
{
    let args = [
        Expr::Symbol(Symbol::new("v1")),
        Expr::String(form.to_owned()),
    ]
    .iter()
    .map(|expr| sim_citizen::value_from_expr(cx, expr))
    .collect::<sim_kernel::Result<Vec<_>>>()
    .unwrap();
    cx.read_construct(&class, args)
        .unwrap()
        .object()
        .downcast_ref::<T>()
        .unwrap()
        .clone()
}

fn read_construct_expr(class: Symbol, form: &str) -> Expr {
    Expr::Extension {
        tag: Symbol::qualified("citizen", "read-construct"),
        payload: Box::new(Expr::Vector(vec![
            Expr::Symbol(class),
            Expr::Symbol(Symbol::new("v1")),
            Expr::String(form.to_owned()),
        ])),
    }
}
