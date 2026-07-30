use num_rational::Ratio;

use sim_lib_music_core::{
    Articulation, Channel, Note, ObjectId, Pitch, Staff, StaffNote, StaffVoice, Time,
};

use crate::{
    DelayedNoteOrder, MusicTransformChange, RegisterRange, RegisterTie, RhythmMask, SustainSpan,
    apply_rhythm_mask, expand_staff, parallel_staff, restore_register, separate_delayed_notes,
    sequence_staff, slice_staff, slur_staff, sustain_staff, unwrap_register,
};

fn q() -> Time {
    Ratio::new(1, 4)
}

fn make_note(voice: &ObjectId, path: &str, onset: Time, duration: Time, midi: u8) -> StaffNote {
    StaffNote {
        voice_id: voice.clone(),
        note_id: ObjectId::new(format!("note/{path}")).expect("note id"),
        event_id: ObjectId::new(format!("event/{path}")).expect("event id"),
        onset,
        note: Note::new(
            duration,
            Pitch::from_midi(midi),
            96,
            Channel::new(0).expect("channel"),
            Articulation::Normal,
        )
        .expect("note"),
    }
}

fn one_voice(voice_name: &str, duration: Time, notes: Vec<StaffNote>) -> Staff {
    let voice = notes
        .first()
        .map(|note| note.voice_id.clone())
        .unwrap_or_else(|| ObjectId::new(voice_name).expect("voice id"));
    Staff::new(vec![StaffVoice {
        id: voice,
        name: voice_name.to_owned(),
        duration,
        notes,
    }])
    .expect("staff")
}

#[test]
fn sustain_and_slur_report_exact_duration_and_articulation_edits() {
    let voice = ObjectId::new("voice/lead").expect("voice");
    let staff = one_voice(
        "Lead",
        Time::from_integer(1),
        vec![
            make_note(&voice, "lead/0", Time::from_integer(0), q(), 60),
            make_note(&voice, "lead/1", Ratio::new(1, 2), q(), 62),
        ],
    );
    let sustained = sustain_staff(
        &staff,
        &[SustainSpan::new(Ratio::new(1, 8), Ratio::new(3, 8), None)],
    )
    .expect("sustain");
    assert_eq!(
        sustained.value.voices[0].notes[0].note.duration,
        Ratio::new(3, 8)
    );
    assert!(matches!(
        sustained.changes[0],
        MusicTransformChange::Duration { .. }
    ));

    let slurred = slur_staff(&staff).expect("slur");
    assert_eq!(
        slurred.value.voices[0].notes[0].note.duration,
        Ratio::new(1, 2)
    );
    assert_eq!(
        slurred.value.voices[0].notes[0].note.articulation,
        Articulation::Legato
    );
    assert!(
        slurred
            .changes
            .iter()
            .any(|change| matches!(change, MusicTransformChange::Articulation { .. }))
    );
}

#[test]
fn expansion_is_exactly_invertible_without_float_time() {
    let voice = ObjectId::new("voice/expand").expect("voice");
    let staff = one_voice(
        "Expand",
        Ratio::new(5, 8),
        vec![make_note(
            &voice,
            "expand/0",
            Ratio::new(1, 8),
            Ratio::new(3, 16),
            60,
        )],
    );
    let expanded = expand_staff(&staff, Ratio::new(3, 2)).expect("expand");
    let restored = expand_staff(&expanded.value, Ratio::new(2, 3)).expect("contract");
    assert_eq!(restored.value, staff);
}

#[test]
fn delayed_note_separation_preserves_ids_and_keeps_exact_abutment() {
    let voice = ObjectId::new("voice/delayed").expect("voice");
    let staff = one_voice(
        "Delayed",
        Time::from_integer(1),
        vec![
            make_note(
                &voice,
                "delayed/0",
                Time::from_integer(0),
                Ratio::new(1, 2),
                60,
            ),
            make_note(&voice, "delayed/1", q(), q(), 67),
            make_note(&voice, "delayed/2", Ratio::new(1, 2), q(), 62),
        ],
    );
    let original_ids = staff.object_ids();
    let separated =
        separate_delayed_notes(&staff, DelayedNoteOrder::HighestFirst).expect("separate");

    assert_eq!(separated.value.voices.len(), 2);
    assert_eq!(separated.value.voices[0].notes.len(), 2);
    assert_eq!(separated.value.voices[0].notes[1].onset, Ratio::new(1, 2));
    assert_eq!(separated.preserved, original_ids);
    assert!(
        separated
            .changes
            .iter()
            .any(|change| matches!(change, MusicTransformChange::CreatedVoice { .. }))
    );
}

#[test]
fn register_unwrap_report_restores_original_octaves() {
    let voice = ObjectId::new("voice/register").expect("voice");
    let staff = one_voice(
        "Register",
        Ratio::new(1, 2),
        vec![
            make_note(&voice, "register/0", Time::from_integer(0), q(), 60),
            make_note(&voice, "register/1", q(), q(), 83),
        ],
    );
    let unwrapped = unwrap_register(
        &staff,
        RegisterRange {
            low: Pitch::from_midi(48),
            high: Pitch::from_midi(72),
            tie: RegisterTie::Descending,
        },
    )
    .expect("unwrap");
    assert_eq!(
        unwrapped.value.voices[0].notes[1].note.pitch.to_midi(),
        Some(59)
    );
    assert_eq!(restore_register(&unwrapped).expect("restore").value, staff);
}

#[test]
fn exact_duration_sequence_and_parallel_laws_hold() {
    let voice_a = ObjectId::new("voice/a").expect("voice");
    let voice_b = ObjectId::new("voice/b").expect("voice");
    let first = one_voice(
        "voice/a",
        Ratio::new(3, 4),
        vec![make_note(&voice_a, "a/0", Time::from_integer(0), q(), 60)],
    );
    let second_same_voice = one_voice(
        "voice/a",
        Ratio::new(1, 2),
        vec![make_note(&voice_a, "a/1", Ratio::new(1, 8), q(), 62)],
    );
    let second_parallel = one_voice(
        "voice/b",
        Ratio::new(1, 2),
        vec![make_note(&voice_b, "b/0", Time::from_integer(0), q(), 48)],
    );

    let sequence = sequence_staff(&[first.clone(), second_same_voice]).expect("sequence");
    assert_eq!(sequence.value.duration(), Ratio::new(5, 4));
    assert_eq!(sequence.value.voices[0].notes[1].onset, Ratio::new(7, 8));

    let parallel = parallel_staff(&[first, second_parallel]).expect("parallel");
    assert_eq!(parallel.value.duration(), Ratio::new(3, 4));
}

#[test]
fn slicing_composes_and_rhythm_masks_are_idempotent() {
    let voice = ObjectId::new("voice/laws").expect("voice");
    let staff = one_voice(
        "Laws",
        Time::from_integer(1),
        vec![
            make_note(&voice, "laws/0", Time::from_integer(0), q(), 60),
            make_note(&voice, "laws/1", q(), q(), 62),
            make_note(&voice, "laws/2", Ratio::new(1, 2), q(), 64),
            make_note(&voice, "laws/3", Ratio::new(3, 4), q(), 65),
        ],
    );

    let outer = slice_staff(&staff, Ratio::new(1, 8), Ratio::new(7, 8)).expect("outer");
    let nested = slice_staff(&outer.value, Ratio::new(1, 8), Ratio::new(1, 2)).expect("nested");
    let direct = slice_staff(&staff, q(), Ratio::new(5, 8)).expect("direct");
    assert_eq!(nested.value, direct.value);

    let mask = RhythmMask::new(q(), vec![true, false]).expect("mask");
    let once = apply_rhythm_mask(&staff, &mask).expect("once");
    let twice = apply_rhythm_mask(&once.value, &mask).expect("twice");
    assert_eq!(once.value, twice.value);
    assert_eq!(
        once.value.voices[0]
            .notes
            .iter()
            .map(|note| note.note.pitch.to_midi().expect("midi"))
            .collect::<Vec<_>>(),
        vec![60, 64]
    );
}
