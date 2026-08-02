use num_rational::Ratio;

// conformance: exact staff transforms preserve identities and satisfy composition laws.

use sim_lib_music_core::{
    Articulation, Channel, Note, ObjectId, Pitch, Staff, StaffNote, StaffVoice, Time,
};

use crate::{
    AssignmentCertificate, DelayedNoteOrder, MusicTransformChange, RegisterRange, RegisterTie,
    RhythmMask, SustainSpan, VoiceCrossingPolicy, VoiceLeadingMetric, VoiceLeadingMotion,
    VoiceLeadingPolicy, apply_rhythm_mask, expand_staff, parallel_staff, progression_multiply,
    progression_overlay, progression_repeat, progression_slice, restore_register,
    separate_delayed_notes, sequence_staff, slice_staff, slur_staff, sustain_staff,
    unwrap_register, verify_voice_leading_path, voice_leading, voice_leading_path,
    voicing_change_palette,
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

fn voicing_staff(name: &str, pitches: &[u8]) -> Staff {
    Staff::new(
        pitches
            .iter()
            .enumerate()
            .map(|(index, pitch)| {
                let voice = ObjectId::new(format!("voice/{name}/{index}")).expect("voice");
                StaffVoice {
                    id: voice.clone(),
                    name: format!("{name} {index}"),
                    duration: Time::from_integer(1),
                    notes: vec![make_note(
                        &voice,
                        &format!("{name}/{index}"),
                        Time::from_integer(0),
                        Time::from_integer(1),
                        *pitch,
                    )],
                }
            })
            .collect(),
    )
    .expect("voicing staff")
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
        &[
            SustainSpan::new(Ratio::new(3, 8), Ratio::new(1, 2), None),
            SustainSpan::new(Ratio::new(1, 8), Ratio::new(3, 8), None),
        ],
    )
    .expect("sustain");
    assert_eq!(
        sustained.value.voices[0].notes[0].note.duration,
        Ratio::new(1, 2)
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

#[test]
fn certified_optimum_beats_sequential_greedy_voice_leading() {
    let source =
        crate::ExactVoicing::from_staff(&voicing_staff("source", &[69, 70]), Time::from_integer(0))
            .expect("source");
    let target =
        crate::ExactVoicing::from_staff(&voicing_staff("target", &[60, 69]), Time::from_integer(0))
            .expect("target");
    let policy = VoiceLeadingPolicy::new(1_000, 1_000);
    let optimal = voice_leading(&source, &target, &policy).expect("optimal");

    let mut unused = vec![true; target.notes.len()];
    let mut greedy = 0_i64;
    for from in &source.notes {
        let (index, cost) = target
            .notes
            .iter()
            .enumerate()
            .filter(|(index, _)| unused[*index])
            .map(|(index, to)| {
                let distance = i64::from(to.pitch.semitone()) - i64::from(from.pitch.semitone());
                (index, distance * distance)
            })
            .min_by_key(|(index, cost)| (*cost, *index))
            .expect("greedy target");
        unused[index] = false;
        greedy += cost;
    }

    assert_eq!(greedy, 100);
    assert_eq!(optimal.assignment.total_cost, 82);
    assert!(matches!(
        optimal.assignment.certificate,
        AssignmentCertificate::MinCostFlow { .. }
    ));
    assert!(optimal.motions.iter().all(|motion| matches!(
        motion,
        VoiceLeadingMotion::Move { source, target, .. }
            if source.note_id.as_str().starts_with("note/source/")
                && target.note_id.as_str().starts_with("note/target/")
    )));
}

#[test]
fn voice_leading_preserves_multiplicity_ties_and_path_certificates() {
    let first =
        crate::ExactVoicing::from_staff(&voicing_staff("first", &[60, 64]), Time::from_integer(0))
            .expect("first");
    let tied =
        crate::ExactVoicing::from_staff(&voicing_staff("tied", &[62, 62]), Time::from_integer(0))
            .expect("tied");
    let last =
        crate::ExactVoicing::from_staff(&voicing_staff("last", &[61, 65]), Time::from_integer(0))
            .expect("last");
    let policy = VoiceLeadingPolicy::new(100, 100)
        .with_metric(VoiceLeadingMetric::SquaredSemitones)
        .with_voice_crossing(VoiceCrossingPolicy::Allow);

    let first_run = voice_leading(&first, &tied, &policy).expect("first run");
    let replay = voice_leading(&first, &tied, &policy).expect("replay");
    assert_eq!(first_run, replay);
    assert_eq!(first_run.assignment.total_cost, 8);
    assert_eq!(
        first_run
            .motions
            .iter()
            .filter_map(|motion| match motion {
                VoiceLeadingMotion::Move { target, .. } => Some(target.event_id.clone()),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2
    );

    let path = voice_leading_path(&[first, tied, last], &policy).expect("path");
    verify_voice_leading_path(&path, &policy).expect("path certificate");
    assert_eq!(path.certificate.leg_costs, vec![8, 10]);
    assert_eq!(path.certificate.total_cost, 18);
}

#[test]
fn unequal_voice_counts_use_doubling_and_exact_palette_edges() {
    let single =
        crate::ExactVoicing::from_staff(&voicing_staff("single", &[60]), Time::from_integer(0))
            .expect("single");
    let pair =
        crate::ExactVoicing::from_staff(&voicing_staff("pair", &[60, 67]), Time::from_integer(0))
            .expect("pair");
    let third =
        crate::ExactVoicing::from_staff(&voicing_staff("third", &[62, 65]), Time::from_integer(0))
            .expect("third");
    let policy = VoiceLeadingPolicy::new(100, 100).with_doubling(10);
    let leading = voice_leading(&single, &pair, &policy).expect("doubling");

    assert_eq!(leading.assignment.total_cost, 59);
    assert!(
        leading
            .motions
            .iter()
            .any(|motion| matches!(motion, VoiceLeadingMotion::Double { cost: 59, .. }))
    );

    let palette = voicing_change_palette(&[single, pair, third], &policy).expect("change palette");
    assert_eq!(palette.changes.len(), 6);
    assert_eq!(
        palette
            .changes
            .iter()
            .map(|change| (change.source, change.target))
            .collect::<Vec<_>>(),
        vec![(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)]
    );
    assert_eq!(palette.outgoing(1).count(), 2);
    assert_eq!(palette.outgoing(99).count(), 0);
}

#[test]
fn exact_progression_algebra_retains_or_reports_every_identity() {
    let first = voicing_staff("algebra-a", &[60]);
    let second = voicing_staff("algebra-b", &[67]);

    let multiplied =
        progression_multiply(&first, Ratio::new(3, 2)).expect("duration multiplication");
    assert_eq!(multiplied.value.duration(), Ratio::new(3, 2));
    assert_eq!(multiplied.preserved, first.object_ids());

    let overlaid = progression_overlay(&[first.clone(), second]).expect("overlay");
    assert_eq!(overlaid.value.voices.len(), 2);

    let sliced = progression_slice(&multiplied.value, q(), Time::from_integer(1)).expect("slice");
    assert_eq!(sliced.value.duration(), Ratio::new(3, 4));

    let repeated = progression_repeat(&first, 3).expect("repeat");
    assert_eq!(repeated.value.duration(), Time::from_integer(3));
    assert_eq!(repeated.value.voices[0].notes.len(), 3);
    assert_eq!(
        repeated.value.voices[0].notes[1].note_id.as_str(),
        "note/algebra-a/0/repeat/1"
    );
    assert_eq!(
        repeated
            .changes
            .iter()
            .filter(|change| matches!(change, MusicTransformChange::RepeatedIdentity { .. }))
            .count(),
        2
    );
    assert_eq!(repeated.preserved, first.object_ids());

    let empty = progression_repeat(&first, 0).expect("zero repeats");
    assert!(empty.value.voices.is_empty());
}
