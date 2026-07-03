use std::sync::Arc;

use sim_kernel::Cx;
use sim_kernel::{DefaultFactory, EagerPolicy};
use sim_lib_midi_core::{
    Channel, ChannelMessage, MemoryMidiSource, MidiEvent, MidiPayload, MidiSink, TickTime, U7, U14,
    pump, synthetic_origin,
};
use sim_lib_sound_timbre::pure_sine;
use sim_lib_sound_tuning::TuningDescriptor;

use crate::{
    BridgeOptions, MidiToSoundBridge, SoundBridgeError, TimbreBank, VoicePhase, VoicePool,
    install_sound_bridge_lib,
};

fn channel(value: u8) -> Channel {
    Channel::new(value).unwrap()
}

fn event(ticks: i64, payload: MidiPayload) -> MidiEvent {
    MidiEvent {
        time: TickTime::new(ticks, 480).unwrap(),
        origin: synthetic_origin(),
        payload,
    }
}

fn bank() -> TimbreBank {
    TimbreBank::new(pure_sine())
}

fn tuning() -> Box<dyn sim_lib_sound_tuning::Tuning> {
    TuningDescriptor::EqualTemperament {
        divisions: 12,
        reference_midi: 69,
        reference_hz: 440.0,
    }
    .to_tuning()
    .unwrap()
}

#[test]
fn note_on_then_note_off_emits_one_released_tone() {
    let mut bridge =
        MidiToSoundBridge::new(480, bank(), tuning(), BridgeOptions::default()).unwrap();
    bridge
        .write(&event(
            0,
            MidiPayload::Channel(ChannelMessage::NoteOn {
                ch: channel(0),
                key: U7(69),
                vel: U7(100),
            }),
        ))
        .unwrap();
    bridge
        .write(&event(
            480,
            MidiPayload::Channel(ChannelMessage::NoteOff {
                ch: channel(0),
                key: U7(69),
                vel: U7(0),
            }),
        ))
        .unwrap();
    bridge.flush().unwrap();
    let tones = bridge.drain_tones();
    assert_eq!(tones.len(), 1);
    assert_eq!(tones[0].channel, 0);
    assert_eq!(tones[0].key, 69);
    assert!(tones[0].tone.duration.as_secs_f64() > 0.5);
}

#[test]
fn sustain_delays_release_until_pedal_off() {
    let mut bridge =
        MidiToSoundBridge::new(480, bank(), tuning(), BridgeOptions::default()).unwrap();
    bridge
        .write(&event(
            0,
            MidiPayload::Channel(ChannelMessage::ControlChange {
                ch: channel(0),
                cc: sim_lib_midi_core::CC_SUSTAIN_PEDAL,
                value: U7(127),
            }),
        ))
        .unwrap();
    bridge
        .write(&event(
            0,
            MidiPayload::Channel(ChannelMessage::NoteOn {
                ch: channel(0),
                key: U7(60),
                vel: U7(96),
            }),
        ))
        .unwrap();
    bridge
        .write(&event(
            240,
            MidiPayload::Channel(ChannelMessage::NoteOff {
                ch: channel(0),
                key: U7(60),
                vel: U7(0),
            }),
        ))
        .unwrap();
    assert_eq!(
        bridge.voice_pool().voices()[0].phase,
        VoicePhase::SustainHeld
    );
    bridge
        .write(&event(
            480,
            MidiPayload::Channel(ChannelMessage::ControlChange {
                ch: channel(0),
                cc: sim_lib_midi_core::CC_SUSTAIN_PEDAL,
                value: U7(0),
            }),
        ))
        .unwrap();
    bridge.flush().unwrap();
    assert_eq!(bridge.drain_tones().len(), 1);
}

#[test]
fn pitch_bend_changes_rendered_frequency() {
    let mut bridge =
        MidiToSoundBridge::new(480, bank(), tuning(), BridgeOptions::default()).unwrap();
    bridge
        .write(&event(
            0,
            MidiPayload::Channel(ChannelMessage::PitchBend {
                ch: channel(0),
                value: U14(16_383),
            }),
        ))
        .unwrap();
    bridge
        .write(&event(
            0,
            MidiPayload::Channel(ChannelMessage::NoteOn {
                ch: channel(0),
                key: U7(69),
                vel: U7(100),
            }),
        ))
        .unwrap();
    bridge.flush().unwrap();
    let tone = bridge.drain_tones().pop().unwrap();
    assert!(tone.tone.partials[0].frequency.0 > 440.0);
}

#[test]
fn voice_pool_respects_polyphony_limit() {
    let mut bridge = MidiToSoundBridge::new(
        480,
        bank(),
        tuning(),
        BridgeOptions::new(1, BridgeOptions::default().bend_range_cents).unwrap(),
    )
    .unwrap();
    bridge
        .write(&event(
            0,
            MidiPayload::Channel(ChannelMessage::NoteOn {
                ch: channel(0),
                key: U7(60),
                vel: U7(100),
            }),
        ))
        .unwrap();
    bridge
        .write(&event(
            120,
            MidiPayload::Channel(ChannelMessage::NoteOn {
                ch: channel(0),
                key: U7(64),
                vel: U7(100),
            }),
        ))
        .unwrap();
    assert_eq!(bridge.voice_pool().active_voice_count(), 1);
    assert_eq!(bridge.voice_pool().emitted().len(), 1);
    assert_eq!(bridge.stolen_voice_count(), 1);
}

#[test]
fn bridge_implements_midi_sink_and_pump() {
    let mut source = MemoryMidiSource::new(
        480,
        vec![
            event(
                0,
                MidiPayload::Channel(ChannelMessage::NoteOn {
                    ch: channel(0),
                    key: U7(60),
                    vel: U7(100),
                }),
            ),
            event(
                480,
                MidiPayload::Channel(ChannelMessage::NoteOff {
                    ch: channel(0),
                    key: U7(60),
                    vel: U7(0),
                }),
            ),
        ],
    );
    let mut bridge =
        MidiToSoundBridge::new(480, bank(), tuning(), BridgeOptions::default()).unwrap();
    assert_eq!(pump(&mut source, &mut bridge).unwrap(), 2);
    assert_eq!(bridge.drain_tones().len(), 1);
}

#[test]
fn zero_polyphony_is_rejected() {
    assert_eq!(
        VoicePool::new(0).unwrap_err(),
        SoundBridgeError::ZeroPolyphony
    );
}

#[test]
fn runtime_install_is_idempotent() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_sound_bridge_lib(&mut cx).unwrap();
    install_sound_bridge_lib(&mut cx).unwrap();
}
