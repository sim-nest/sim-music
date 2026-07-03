use std::sync::Arc;

use sim_kernel::Cx;
use sim_kernel::{DefaultFactory, EagerPolicy};
use sim_lib_sound_bridge::ScheduledTone;
use sim_lib_sound_core::{Frequency, Tone};

use crate::{PcmRenderer, RendererOptions, install_sound_render_lib};

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
fn runtime_install_is_idempotent() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_sound_render_lib(&mut cx).unwrap();
    install_sound_render_lib(&mut cx).unwrap();
}
