use sim_lib_midi_core::SysExEvent;

use crate::{
    DX7_BANK_VOICE_COUNT, DX7_SINGLE_VOICE_FORMAT, DX7_VOICE_BANK_FORMAT, Dx7Bulk, Dx7Voice,
    Dx7VoiceBank, MIDI_TUNING_STANDARD_SUB_ID, MtsMessage, MtsMessageKind, SysExViewError,
    UniversalRealm, UniversalSysEx, YAMAHA_MANUFACTURER_ID, YamahaSysEx,
};

#[test]
fn universal_sysex_round_trips_f0_payload() {
    let event = SysExEvent::F0 {
        data: vec![0x7f, 0x7f, MIDI_TUNING_STANDARD_SUB_ID, 0x02, 0x00, 0x3c],
    };
    let universal = UniversalSysEx::from_event(&event).expect("universal sysex");
    assert_eq!(universal.realm, UniversalRealm::RealTime);
    assert_eq!(universal.device_id, 0x7f);
    assert_eq!(universal.sub_id1, MIDI_TUNING_STANDARD_SUB_ID);
    assert_eq!(universal.sub_id2, 0x02);
    assert_eq!(universal.data, vec![0x00, 0x3c]);
    assert_eq!(universal.to_event().expect("encode"), event);
}

#[test]
fn mts_view_recognizes_tuning_standard_messages() {
    let event = SysExEvent::F0 {
        data: vec![0x7e, 0x10, MIDI_TUNING_STANDARD_SUB_ID, 0x00, 0x05],
    };
    let message = MtsMessage::from_event(&event).expect("mts message");
    assert_eq!(message.realm(), UniversalRealm::NonRealTime);
    assert_eq!(message.device_id(), 0x10);
    assert_eq!(message.kind(), MtsMessageKind::BulkTuningDumpRequest);
    assert_eq!(message.payload(), &[0x05]);
    assert_eq!(message.to_event().expect("encode"), event);
}

#[test]
fn mts_constructor_preserves_unknown_sub_id2() {
    let message = MtsMessage::new(
        UniversalRealm::RealTime,
        0x7f,
        MtsMessageKind::Unknown(0x7d),
        vec![0x01, 0x02],
    )
    .expect("mts message");
    assert_eq!(message.kind(), MtsMessageKind::Unknown(0x7d));
    assert_eq!(
        message.to_event().expect("encode"),
        SysExEvent::F0 {
            data: vec![0x7f, 0x7f, MIDI_TUNING_STANDARD_SUB_ID, 0x7d, 0x01, 0x02]
        }
    );
}

#[test]
fn typed_views_reject_f7_and_non_universal_payloads() {
    let f7 = SysExEvent::F7 { data: vec![0x01] };
    assert_eq!(UniversalSysEx::from_event(&f7), Err(SysExViewError::NotF0));
    let manufacturer = SysExEvent::F0 {
        data: vec![0x7d, 0x01, 0x02, 0x03],
    };
    assert_eq!(
        UniversalSysEx::from_event(&manufacturer),
        Err(SysExViewError::NotUniversal { id: 0x7d })
    );
}

#[test]
fn typed_views_validate_7_bit_sysex_data() {
    let event = SysExEvent::F0 {
        data: vec![0x7e, 0x10, MIDI_TUNING_STANDARD_SUB_ID, 0x00, 0x80],
    };
    assert_eq!(
        UniversalSysEx::from_event(&event),
        Err(SysExViewError::InvalidDataByte {
            index: 4,
            value: 0x80
        })
    );
}

#[test]
fn dx7_single_voice_sysex_parses_and_round_trips() {
    let voice = Dx7Voice::from_edit_buffer(synthetic_edit_voice("SIMDX7A01")).expect("dx7 voice");
    let event = YamahaSysEx::new(0x00, DX7_SINGLE_VOICE_FORMAT, voice.edit_buffer().to_vec())
        .expect("yamaha sysex")
        .to_event()
        .expect("event");

    let Dx7Bulk::SingleVoice(parsed) = Dx7Bulk::from_event(&event).expect("dx7 bulk") else {
        panic!("expected single voice");
    };

    assert_eq!(parsed.name(), "SIMDX7A01");
    assert_eq!(parsed.common().algorithm, 14);
    assert_eq!(parsed.operators()[0].output_level, 70);
    assert_eq!(
        Dx7Bulk::SingleVoice(parsed)
            .to_yamaha(0x00)
            .expect("encode")
            .to_event()
            .expect("event"),
        event
    );
}

#[test]
fn dx7_voice_bank_sysex_parses_32_packed_voices() {
    let voices = (0..DX7_BANK_VOICE_COUNT)
        .map(|index| {
            Dx7Voice::from_edit_buffer(synthetic_edit_voice(&format!("SIMDX7{index:03}")))
                .expect("dx7 voice")
        })
        .collect::<Vec<_>>();
    let bank = Dx7VoiceBank::from_voices(voices).expect("voice bank");
    let event = Dx7Bulk::VoiceBank(bank.clone())
        .to_yamaha(0x00)
        .expect("yamaha sysex")
        .to_event()
        .expect("event");

    let Dx7Bulk::VoiceBank(parsed) = Dx7Bulk::from_event(&event).expect("dx7 bank") else {
        panic!("expected voice bank");
    };

    assert_eq!(parsed.voices().len(), 32);
    assert_eq!(parsed.voices()[0].name(), "SIMDX7000");
    assert_eq!(parsed.voices()[31].name(), "SIMDX7031");
    assert_eq!(parsed.raw_packed_bank(), bank.raw_packed_bank());
    assert_eq!(parsed.to_packed_bank(), bank.to_packed_bank());
}

#[test]
fn dx7_packed_voice_round_trip_preserves_operator_order() {
    let voice = Dx7Voice::from_edit_buffer(synthetic_edit_voice("SIMDX7A03")).expect("dx7 voice");
    let packed = voice.packed_voice();
    let unpacked = Dx7Voice::from_packed_voice(packed.clone()).expect("packed voice");
    let operators = unpacked.operators();

    assert_eq!(unpacked.raw_packed_voice(), Some(packed.as_slice()));
    assert_eq!(unpacked.packed_voice(), packed);
    assert_eq!(
        operators
            .iter()
            .map(|operator| operator.output_level)
            .collect::<Vec<_>>(),
        vec![70, 71, 72, 73, 74, 75]
    );
}

#[test]
fn dx7_sysex_rejects_bad_checksum_non_7_bit_payload_and_wrong_lengths() {
    let voice = Dx7Voice::from_edit_buffer(synthetic_edit_voice("SIMDX7A04")).expect("dx7 voice");
    let SysExEvent::F0 { mut data } =
        YamahaSysEx::new(0x00, DX7_SINGLE_VOICE_FORMAT, voice.edit_buffer().to_vec())
            .expect("yamaha sysex")
            .to_event()
            .expect("event")
    else {
        panic!("expected F0");
    };
    let checksum_index = data.len() - 1;
    data[checksum_index] = (data[checksum_index] + 1) & 0x7f;
    assert!(matches!(
        Dx7Bulk::from_event(&SysExEvent::F0 { data }),
        Err(SysExViewError::InvalidChecksum { .. })
    ));

    let mut data = YamahaSysEx::new(0x00, DX7_SINGLE_VOICE_FORMAT, voice.edit_buffer().to_vec())
        .expect("yamaha sysex")
        .to_f0_payload()
        .expect("payload");
    data[5] = 0x80;
    assert_eq!(
        Dx7Bulk::from_event(&SysExEvent::F0 { data }),
        Err(SysExViewError::InvalidDataByte {
            index: 5,
            value: 0x80
        })
    );

    assert_eq!(
        Dx7Bulk::from_event(&SysExEvent::F0 {
            data: vec![YAMAHA_MANUFACTURER_ID, 0x00]
        }),
        Err(SysExViewError::TooShort { len: 2 })
    );
    assert_eq!(
        Dx7Bulk::from_event(&SysExEvent::F0 {
            data: vec![
                YAMAHA_MANUFACTURER_ID,
                0x00,
                0x00,
                0x00,
                0x01,
                0x00,
                0x00,
                0x00
            ]
        }),
        Err(SysExViewError::InvalidByteCount {
            expected: 1,
            actual: 2
        })
    );
    assert_eq!(
        Dx7Bulk::from_event(&SysExEvent::F0 {
            data: vec![0x7d, 0x00, DX7_VOICE_BANK_FORMAT, 0x00, 0x00, 0x00]
        }),
        Err(SysExViewError::NotYamaha { id: 0x7d })
    );

    let short_voice = YamahaSysEx::new(
        0x00,
        DX7_SINGLE_VOICE_FORMAT,
        voice.edit_buffer()[..154].to_vec(),
    )
    .expect("short yamaha sysex")
    .to_event()
    .expect("short event");
    assert_eq!(
        Dx7Bulk::from_event(&short_voice),
        Err(SysExViewError::InvalidDx7Length {
            context: "single voice",
            expected: 155,
            actual: 154
        })
    );

    let mut long_data = voice.edit_buffer().to_vec();
    long_data.push(0);
    let long_voice = YamahaSysEx::new(0x00, DX7_SINGLE_VOICE_FORMAT, long_data)
        .expect("long yamaha sysex")
        .to_event()
        .expect("long event");
    assert_eq!(
        Dx7Bulk::from_event(&long_voice),
        Err(SysExViewError::InvalidDx7Length {
            context: "single voice",
            expected: 155,
            actual: 156
        })
    );
}

fn synthetic_edit_voice(name: &str) -> Vec<u8> {
    let mut voice = vec![0; 155];
    for operator in 0..6 {
        let base = operator * 21;
        voice[base..base + 4].copy_from_slice(&[
            40 + operator as u8,
            41 + operator as u8,
            42 + operator as u8,
            43 + operator as u8,
        ]);
        voice[base + 4..base + 8].copy_from_slice(&[99, 80, 60, 0]);
        voice[base + 8] = 30 + operator as u8;
        voice[base + 9] = 10 + operator as u8;
        voice[base + 10] = 11 + operator as u8;
        voice[base + 11] = (operator % 4) as u8;
        voice[base + 12] = ((operator + 1) % 4) as u8;
        voice[base + 13] = operator as u8;
        voice[base + 14] = (operator % 4) as u8;
        voice[base + 15] = (operator % 7) as u8;
        voice[base + 16] = 70 + operator as u8;
        voice[base + 17] = (operator % 2) as u8;
        voice[base + 18] = 1 + operator as u8;
        voice[base + 19] = 20 + operator as u8;
        voice[base + 20] = 7 + operator as u8;
    }
    voice[126..130].copy_from_slice(&[50, 51, 52, 53]);
    voice[130..134].copy_from_slice(&[99, 80, 70, 60]);
    voice[134] = 14;
    voice[135] = 5;
    voice[136] = 1;
    voice[137] = 35;
    voice[138] = 12;
    voice[139] = 44;
    voice[140] = 13;
    voice[141] = 1;
    voice[142] = 3;
    voice[143] = 5;
    voice[144] = 24;
    let mut name_bytes = [b' '; 10];
    name_bytes[..name.len()].copy_from_slice(name.as_bytes());
    voice[145..155].copy_from_slice(&name_bytes);
    voice
}
