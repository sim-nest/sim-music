use super::*;

use sim_lib_music_core::{Articulation, Channel, Music, Note, Score, Time, parse_pitch};
use sim_lib_stream_core::{
    ClockDomain, PcmPacket, StreamDirection, StreamEnvelope, StreamMedia, StreamPacket,
    TransportProfile,
};

#[test]
fn render_music_file_produces_audio_and_tables() {
    let score = Score::new(
        120,
        (4, 4),
        Some("C".to_owned()),
        Music::Note(
            Note::new(
                Time::new(1, 4),
                parse_pitch("C4").unwrap(),
                100,
                Channel(0),
                Articulation::Normal,
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let text = sim_lib_music_shapes::encode_music_file(&score).unwrap();
    let report = render_music_file(&text).unwrap();
    assert!(!report.audio.wav.is_empty());
    assert!(!report.intervals.is_empty());
    assert!(!report.dissonance.is_empty());
}

#[test]
fn web_audio_preview_accepts_buffered_pcm_envelopes() {
    let envelope = pcm_envelope(
        TransportProfile::lan_buffered_audio_preview(),
        StreamPacket::Pcm(PcmPacket::f32(2, 2, vec![0.0, 0.5, -0.5, 1.0]).unwrap()),
    );

    let preview = web_audio_preview_from_buffered_pcm(&envelope, 48_000).unwrap();

    assert_eq!(preview.stream_id, "stream/web-audio");
    assert_eq!(preview.profile, "stream/profile/lan-buffered-audio-preview");
    assert_eq!(preview.sample_rate, 48_000);
    assert_eq!(preview.channels, 2);
    assert_eq!(preview.frame_count, 2);
    assert_eq!(preview.samples, vec![0.0, 0.5, -0.5, 1.0]);
}

#[test]
fn web_audio_preview_converts_i16_samples_to_f32() {
    let envelope = pcm_envelope(
        TransportProfile::buffered_pcm_preview(),
        StreamPacket::Pcm(PcmPacket::i16(1, 3, vec![0, i16::MAX, i16::MIN]).unwrap()),
    );

    let preview = web_audio_preview_from_buffered_pcm(&envelope, 44_100).unwrap();

    assert_eq!(preview.samples, vec![0.0, 1.0, -1.0]);
}

#[test]
fn web_audio_preview_rejects_non_preview_profiles() {
    let envelope = pcm_envelope(
        TransportProfile::realtime_local_audio(),
        StreamPacket::Pcm(PcmPacket::f32(1, 1, vec![0.0]).unwrap()),
    );

    let err = web_audio_preview_from_buffered_pcm(&envelope, 48_000).unwrap_err();

    assert!(err.to_string().contains("not a buffered PCM preview"));
}

#[test]
fn sound_wasm_entry_points_are_stable() {
    let entries = sound_wasm_engine_entry_points();

    assert_eq!(entries.render_demo, "sound-wasm-render-demo");
    assert_eq!(entries.preview_pcm, "sound-wasm-preview-pcm");
    assert_eq!(entries.audio_worklet, "sound-audio-worklet-preview");
}

fn pcm_envelope(profile: TransportProfile, packet: StreamPacket) -> StreamEnvelope {
    StreamEnvelope::new(
        sim_kernel::Symbol::qualified("stream", "web-audio"),
        sim_kernel::Symbol::qualified("stream/web-packet", "0"),
        StreamMedia::Pcm,
        StreamDirection::Source,
        0,
        Vec::new(),
        ClockDomain::BrowserFrame,
        profile,
        Vec::new(),
        packet,
    )
    .unwrap()
}
