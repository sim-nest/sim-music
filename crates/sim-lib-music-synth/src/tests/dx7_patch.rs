use sim_kernel::{Expr, Symbol};
use sim_lib_midi_sysex::Dx7Voice;

use crate::{Dx7Patch, InstrumentPatch};

#[test]
fn dx7_patch_normalizes_voice_and_preserves_raw_buffers() {
    let edit = synthetic_edit_voice("SIMDX7A01");
    let voice = Dx7Voice::from_edit_buffer(edit.clone()).expect("dx7 voice");
    let patch = Dx7Patch::from_voice(&voice);

    assert_eq!(patch.name, "SIMDX7A01");
    assert_eq!(patch.operators.len(), 6);
    assert_eq!(patch.operators[0].output_level, 70);
    assert_eq!(patch.operators[5].output_level, 75);
    assert_eq!(patch.algorithm, 14);
    assert_eq!(patch.feedback, 5);
    assert!(patch.oscillator_sync);
    assert_eq!(patch.raw.edit_buffer, edit);
    assert_eq!(patch.raw.packed_voice, voice.packed_voice());
}

#[test]
fn dx7_patch_exposes_raw_view_as_instrument_patch_data() {
    let voice = Dx7Voice::from_edit_buffer(synthetic_edit_voice("SIMDX7A02")).expect("dx7 voice");
    let patch = Dx7Patch::from_voice(&voice).to_instrument_patch();
    let expr = patch.to_expr();
    let decoded = InstrumentPatch::from_expr(&expr).expect("instrument patch");

    assert_eq!(decoded, patch);
    let raw = decoded.raw_view.expect("raw view");
    assert_eq!(
        raw.format,
        Symbol::qualified("audio-synth/raw", "dx7-voice")
    );
    assert!(raw.fields.iter().any(|(key, value)| {
        key == &Symbol::new("edit-buffer") && value == &Expr::Bytes(voice.edit_buffer().to_vec())
    }));
    assert!(raw.fields.iter().any(|(key, value)| {
        key == &Symbol::new("packed-voice") && value == &Expr::Bytes(voice.packed_voice())
    }));
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
