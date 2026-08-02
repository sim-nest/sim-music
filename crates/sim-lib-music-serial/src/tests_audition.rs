use sim_lib_music_core::MidiPayload;
use sim_lib_music_lower::LowerOpts;

use crate::tests::{strict_context, strict_plan};
use crate::{
    SerialRenderOptions, lower_serial_score, realize_strict, render_serial_audition_score,
    write_serial_smf,
};

#[test]
fn serial_audition_lowers_deterministically_through_existing_midi_owner() {
    let realization = realize_strict(&strict_plan(), &strict_context()).expect("realization");
    let options = SerialRenderOptions::default();

    let first = lower_serial_score(&realization, &options, &LowerOpts::default()).expect("lower");
    let replay = lower_serial_score(&realization, &options, &LowerOpts::default()).expect("replay");
    assert_eq!(first, replay);

    let bytes = write_serial_smf(&realization, &options, &LowerOpts::default()).expect("bytes");
    assert_eq!(&bytes[..4], b"MThd");
}

#[test]
fn serial_audition_preserves_stable_equal_onset_ordering() {
    let realization = realize_strict(&strict_plan(), &strict_context()).expect("realization");
    let score =
        render_serial_audition_score(&realization, &SerialRenderOptions::default()).expect("score");
    let file = sim_lib_music_lower::lower_score(&score, &LowerOpts::default()).expect("lower");
    let note_ons = file.tracks[0]
        .events
        .iter()
        .filter_map(|event| match event.payload {
            MidiPayload::Channel(sim_lib_music_core::ChannelMessage::NoteOn { key, .. }) => {
                Some((event.time.ticks, key.0))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    let replay = sim_lib_music_lower::lower_score(&score, &LowerOpts::default()).expect("replay");
    let replay_note_ons = replay.tracks[0]
        .events
        .iter()
        .filter_map(|event| match event.payload {
            MidiPayload::Channel(sim_lib_music_core::ChannelMessage::NoteOn { key, .. }) => {
                Some((event.time.ticks, key.0))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(note_ons, replay_note_ons);
}
