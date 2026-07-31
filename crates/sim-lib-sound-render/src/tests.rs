use std::sync::Arc;

use sim_kernel::Cx;
use sim_kernel::{DefaultFactory, EagerPolicy};
use sim_lib_sound_bridge::ScheduledTone;
use sim_lib_sound_core::{Frequency, Tone};
use sim_lib_sound_timbre::pure_sine;

use crate::{PcmRenderer, RendererOptions, SoundRenderError, install_sound_render_lib};

// conformance: sound rendering reuse produces deterministic offline PCM.

#[test]
fn render_tone_produces_non_zero_samples_for_sine() {
    let renderer = PcmRenderer::new(RendererOptions::default()).unwrap();
    let tone = Tone::sine(Frequency(440.0), std::time::Duration::from_millis(25));
    let rendered = renderer.render_tone(&tone);
    assert!(rendered.iter().any(|sample| sample.abs() > 0.0));
}

#[test]
fn write_wav_emits_valid_riff_wave_header() {
    let renderer = PcmRenderer::new(RendererOptions::default()).unwrap();
    let tone = Tone::sine(Frequency(440.0), std::time::Duration::from_millis(5));
    let rendered = renderer.render_tone(&tone);
    let wav = renderer.write_wav(&rendered, Vec::new()).unwrap();
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
}

#[test]
fn write_wav_rejects_channel_misaligned_samples() {
    let renderer = PcmRenderer::new(RendererOptions::default()).unwrap();

    let err = renderer.write_wav(&[0.0], Vec::new()).unwrap_err();

    assert_eq!(err, SoundRenderError::ChannelMisalignedSamples);
}

#[test]
fn write_wav_uses_checked_header_arithmetic() {
    let renderer = PcmRenderer::new(RendererOptions::new(u32::MAX, 2).unwrap()).unwrap();

    let err = renderer.write_wav(&[], Vec::new()).unwrap_err();

    assert_eq!(err, SoundRenderError::BufferTooLarge);
}

#[test]
fn pcm_renderer_exposes_validated_options_through_accessors() {
    let renderer = PcmRenderer::new(RendererOptions::new(22_050, 1).unwrap()).unwrap();

    assert_eq!(renderer.sample_rate(), 22_050);
    assert_eq!(renderer.channels(), 1);
}

#[test]
fn render_mix_respects_scheduled_start_and_pan() {
    let renderer = PcmRenderer::new(RendererOptions::default()).unwrap();
    let tones = vec![
        ScheduledTone {
            start: std::time::Duration::ZERO,
            tone: Tone::sine(Frequency(220.0), std::time::Duration::from_millis(10)),
            pan: -1.0,
            channel: 0,
            key: 57,
        },
        ScheduledTone {
            start: std::time::Duration::from_millis(5),
            tone: Tone::sine(Frequency(440.0), std::time::Duration::from_millis(10)),
            pan: 1.0,
            channel: 1,
            key: 69,
        },
    ];
    let mix = renderer.render_mix(&tones);
    assert!(mix.len() > renderer.render_tone(&tones[0].tone).len());
    assert!(mix.iter().any(|sample| sample.abs() > 0.0));
}

#[test]
fn render_timbre_preview_uses_pcm_renderer() {
    let renderer = PcmRenderer::new(RendererOptions::new(8_000, 1).unwrap()).unwrap();
    let samples = renderer
        .render_timbre_preview(
            &pure_sine(),
            Frequency(440.0),
            std::time::Duration::from_millis(10),
        )
        .expect("preview");
    assert_eq!(samples.len(), 80);
}

#[test]
fn catalog_timbres_render_deterministically_to_offline_pcm() {
    use sim_lib_sound_timbre::{fm_pair, harmonic_expansion, karplus_strong};

    let renderer = PcmRenderer::new(RendererOptions::new(8_000, 1).unwrap()).unwrap();
    for timbre in [
        harmonic_expansion(6, 0.5, 0.0),
        karplus_strong(0.8),
        fm_pair(2.0, 1.5),
    ] {
        let first = renderer
            .render_timbre_preview(
                &timbre,
                Frequency(220.0),
                std::time::Duration::from_millis(20),
            )
            .expect("first preview");
        let second = renderer
            .render_timbre_preview(
                &timbre,
                Frequency(220.0),
                std::time::Duration::from_millis(20),
            )
            .expect("second preview");
        assert_eq!(first, second);
        assert!(first.iter().all(|sample| sample.is_finite()));
        assert!(first.iter().any(|sample| sample.abs() > 0.0));
    }
}

#[test]
fn runtime_install_is_idempotent() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_sound_render_lib(&mut cx).unwrap();
    install_sound_render_lib(&mut cx).unwrap();
}
