use num_rational::Ratio;
use std::any::Any;

mod filter;

use sim_lib_midi_core::{ChannelMessage, MidiPayload};
use sim_lib_music_core::{
    Articulation, AtomRef, Channel, Melody, MelodyItem, Music, MusicObject, Note, PianoRoll, Rest,
    Time, TimedAtom, TimedNote,
};
use sim_lib_music_lower::{LowerOpts, lower};
use sim_lib_pitch_chord::Chord as PitchChord;
use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_pitch_scale::{Key, Mode, Scale};

use crate::{
    CallablePitchMap, CustomPitchAxis, FunctionMap, IntMatrix, InvertTransform, PatternLockSet,
    PatternMutatorConfig, PitchAxis, PitchDelta, PitchRemap, RetrogradeMode, RetrogradeTransform,
    StretchPolicy, TimeMapPoint, TransformChain, TransformDiagnosticCode, TransformError,
    TransformReport, TransformStep, TransposeTransform, TuningRemap, WarpMarker, augment, loop_n,
    map_to_function, mutate_pattern, pitch_invert, player_mutator, quarter, retrograde_with_mode,
    shift_octave, simple_melody, slice, transpose, transpose_diatonic,
};

fn note(midi: u8, duration: Ratio<i64>) -> Note {
    Note::new(
        duration,
        Pitch::from_midi(midi),
        100,
        Channel::new(0).expect("channel"),
        Articulation::Normal,
    )
    .expect("note")
}

fn melody_fixture() -> Melody {
    Melody::new(vec![
        MelodyItem::Note(note(60, quarter())),
        MelodyItem::Note(note(64, quarter())),
        MelodyItem::Rest(Rest::new(quarter()).expect("rest")),
        MelodyItem::Note(note(67, quarter())),
    ])
    .expect("melody")
}

fn roll_midis(music: &Music) -> Vec<u8> {
    let Music::PianoRoll(roll) = music else {
        panic!("piano roll");
    };
    roll.items
        .iter()
        .map(|item| item.note.pitch.to_midi().expect("midi pitch"))
        .collect()
}

fn roll_span(music: &Music) -> Time {
    let Music::PianoRoll(roll) = music else {
        panic!("piano roll");
    };
    roll.items
        .iter()
        .map(|item| item.onset + item.note.duration)
        .max()
        .unwrap_or_else(|| Time::from_integer(0))
}

fn roll_items(music: &Music) -> Vec<TimedNote> {
    let Music::PianoRoll(roll) = music else {
        panic!("piano roll");
    };
    roll.items.clone()
}

fn lowered_note_events(object: &dyn MusicObject) -> Vec<(i64, bool, u8, u8, u8)> {
    lower(object, &LowerOpts::default()).expect("lower").tracks[0]
        .events
        .iter()
        .filter_map(|event| match &event.payload {
            MidiPayload::Channel(ChannelMessage::NoteOn { ch, key, vel }) => {
                Some((event.time.ticks, true, ch.0, key.0, vel.0))
            }
            MidiPayload::Channel(ChannelMessage::NoteOff { ch, key, vel }) => {
                Some((event.time.ticks, false, ch.0, key.0, vel.0))
            }
            _ => None,
        })
        .collect()
}

fn transform(result: Result<Music, TransformError>) -> Music {
    result.expect("transform")
}

fn transform_report(result: Result<TransformReport, TransformError>) -> TransformReport {
    result.expect("transform report")
}

#[test]
fn pattern_mutator_covers_order_pitch_and_scale_ops() {
    let phrase = simple_melody(&[(60, quarter()), (62, quarter()), (64, quarter())]);

    let reverse =
        transform(PatternMutatorConfig::new(vec![crate::MutationOp::Reverse]).apply(&phrase));
    let rotate = transform(
        PatternMutatorConfig::new(vec![crate::MutationOp::Rotate { steps: 1 }]).apply(&phrase),
    );
    let transpose = PatternMutatorConfig::new(vec![crate::MutationOp::Transpose { semitones: 2 }])
        .apply(&phrase)
        .expect("transpose mutator");
    let invert = PatternMutatorConfig::new(vec![crate::MutationOp::Invert {
        axis: Pitch::from_midi(60),
    }])
    .apply(&phrase)
    .expect("invert mutator");
    let scale_phrase = simple_melody(&[(61, quarter()), (63, quarter()), (66, quarter())]);
    let scale = PatternMutatorConfig::new(vec![crate::MutationOp::ScaleConform {
        scale: Scale::major(PitchClass::C),
    }])
    .apply(&scale_phrase)
    .expect("scale mutator");

    assert_eq!(roll_midis(&reverse), vec![64, 62, 60]);
    assert_eq!(roll_midis(&rotate), vec![64, 60, 62]);
    assert_eq!(roll_midis(&transpose), vec![62, 64, 66]);
    assert_eq!(roll_midis(&invert), vec![60, 58, 56]);
    assert_eq!(roll_midis(&scale), vec![60, 62, 65]);
}

#[test]
fn pattern_mutator_covers_shuffle_density_velocity_and_rhythm_ops() {
    let phrase = simple_melody(&[
        (60, quarter()),
        (62, quarter()),
        (64, quarter()),
        (65, quarter()),
    ]);

    let seeded = PatternMutatorConfig::new(vec![
        crate::MutationOp::ShuffleWithinBeat {
            beat: Time::from_integer(1),
        },
        crate::MutationOp::VelocityRemap { low: 40, high: 92 },
        crate::MutationOp::RhythmDisplace { offset: quarter() },
    ])
    .with_seed(41);
    let first = transform(seeded.apply(&phrase));
    let replay = transform(seeded.apply(&phrase));
    let original = transform(PatternMutatorConfig::new(Vec::new()).apply(&phrase));
    assert_eq!(roll_items(&first), roll_items(&replay));
    assert_ne!(roll_items(&first), roll_items(&original));

    let thin = transform(
        PatternMutatorConfig::new(vec![crate::MutationOp::Thin { keep_percent: 0 }]).apply(&phrase),
    );
    assert!(roll_items(&thin).is_empty());

    let thickened = PatternMutatorConfig::new(vec![crate::MutationOp::Thicken { semitones: 12 }])
        .apply(&phrase)
        .expect("thicken mutator");
    let thick_midis = roll_midis(&thickened);
    assert_eq!(thick_midis.len(), 8);
    assert!(thick_midis.contains(&72));

    let velocities =
        PatternMutatorConfig::new(vec![crate::MutationOp::VelocityRemap { low: 40, high: 40 }])
            .apply(&phrase)
            .expect("velocity mutator");
    assert!(
        roll_items(&velocities)
            .iter()
            .all(|item| item.note.velocity == 40)
    );
}

#[test]
fn pattern_mutator_preserves_locked_notes() {
    let phrase = simple_melody(&[(60, quarter()), (62, quarter()), (64, quarter())]);
    let config = PatternMutatorConfig::new(vec![
        crate::MutationOp::Reverse,
        crate::MutationOp::Transpose { semitones: 12 },
        crate::MutationOp::Thin { keep_percent: 0 },
        crate::MutationOp::VelocityRemap { low: 1, high: 1 },
    ])
    .with_seed(5)
    .with_locks(PatternLockSet::from_note_indices([1]));

    let items = roll_items(&mutate_pattern(&phrase, &config).expect("mutate pattern"));

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].onset, quarter());
    assert_eq!(items[0].note.duration, quarter());
    assert_eq!(items[0].note.pitch.to_midi(), Some(62));
    assert_eq!(items[0].note.velocity, 100);
}

#[test]
fn pattern_mutator_round_trips_wire_config_and_player() {
    let config = PatternMutatorConfig::new(vec![
        crate::MutationOp::Reverse,
        crate::MutationOp::Rotate { steps: -1 },
        crate::MutationOp::Transpose { semitones: 7 },
        crate::MutationOp::Invert {
            axis: Pitch::from_midi(60),
        },
        crate::MutationOp::ShuffleWithinBeat { beat: quarter() },
        crate::MutationOp::Thin { keep_percent: 75 },
        crate::MutationOp::Thicken { semitones: 12 },
        crate::MutationOp::VelocityRemap { low: 32, high: 96 },
        crate::MutationOp::RhythmDisplace { offset: quarter() },
        crate::MutationOp::ScaleConform {
            scale: Scale::new(PitchClass::D, Mode::Dorian),
        },
    ])
    .with_amount(64)
    .with_seed(99)
    .with_locks(PatternLockSet::from_note_indices([0, 2]));
    let wire = config.to_wire();
    let decoded = PatternMutatorConfig::from_wire(&wire).expect("decode mutator");
    let phrase = simple_melody(&[(60, quarter()), (62, quarter())]);
    let player = player_mutator(decoded.clone());

    assert_eq!(decoded, config);
    assert_eq!(decoded.to_wire(), wire);
    assert_eq!(
        roll_items(&player.play(&phrase).expect("player")),
        roll_items(&config.apply(&phrase).expect("config"))
    );
    assert_eq!(player.to_wire(), wire);
}

#[test]
fn retrograde_cutout_twice_is_equivalent_under_lowering() {
    let melody = melody_fixture();
    let once = retrograde_with_mode(&melody, RetrogradeMode::Cutout).expect("retrograde once");
    let twice = retrograde_with_mode(&once, RetrogradeMode::Cutout).expect("retrograde twice");
    assert_eq!(lowered_note_events(&melody), lowered_note_events(&twice));
}

#[test]
fn retrograde_pinned_note_on_preserves_note_durations() {
    let melody = melody_fixture();
    let Music::PianoRoll(roll) =
        retrograde_with_mode(&melody, RetrogradeMode::PinnedNoteOn).expect("retrograde")
    else {
        panic!("piano roll");
    };
    assert_eq!(
        roll.items
            .iter()
            .map(|item| item.note.duration)
            .collect::<Vec<_>>(),
        vec![quarter(), quarter(), quarter()]
    );
}

#[test]
fn augment_scales_lowered_ticks() {
    let melody = melody_fixture();
    let original = lower(&melody, &LowerOpts::default()).expect("lower");
    let augmented = lower(
        &augment(&melody, Ratio::from_integer(2)).expect("augment"),
        &LowerOpts::default(),
    )
    .expect("lower");
    let original_ticks: Vec<i64> = original.tracks[0]
        .events
        .iter()
        .map(|event| event.time.ticks)
        .collect();
    let augmented_ticks: Vec<i64> = augmented.tracks[0]
        .events
        .iter()
        .map(|event| event.time.ticks)
        .collect();
    assert_eq!(
        augmented_ticks,
        original_ticks
            .into_iter()
            .map(|tick| tick * 2)
            .collect::<Vec<_>>()
    );
}

#[derive(Clone)]
struct NegativeOnsetObject {
    note: Note,
}

impl MusicObject for NegativeOnsetObject {
    fn kind(&self) -> &'static str {
        "negative-onset-test"
    }

    fn duration(&self) -> Time {
        Time::from_integer(1)
    }

    fn voices<'a>(&'a self, _offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        out.push(TimedAtom {
            onset: Time::from_integer(-1),
            atom: AtomRef::Note(self.note.clone()),
        });
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[test]
fn public_transforms_reject_invalid_music_object_output() {
    let object = NegativeOnsetObject {
        note: note(60, quarter()),
    };

    assert!(matches!(
        transpose(&object, 1),
        Err(TransformError::InvalidMusic(_))
    ));
}

#[test]
fn transpose_changes_only_midi_keys_after_lowering() {
    let melody = melody_fixture();
    let original = lowered_note_events(&melody);
    let transposed = lowered_note_events(&transpose(&melody, 5).expect("transpose"));
    assert_eq!(original.len(), transposed.len());
    for (left, right) in original.iter().zip(&transposed) {
        assert_eq!(left.0, right.0);
        assert_eq!(left.1, right.1);
        assert_eq!(left.2, right.2);
        assert_eq!(left.4, right.4);
        assert_eq!(right.3, left.3 + 5);
    }
}

#[test]
fn diatonic_and_function_maps_follow_scale_degree_logic() {
    let melody = simple_melody(&[(60, quarter()), (62, quarter()), (64, quarter())]);
    let diatonic = transpose_diatonic(&melody, &Scale::major(PitchClass::C), 2).expect("diatonic");
    let mapped = map_to_function(
        &melody,
        &Key {
            tonic: PitchClass::C,
            mode: Mode::Major,
        },
        &FunctionMap::Dorian,
    )
    .expect("function map");
    let Music::PianoRoll(diatonic_roll) = diatonic else {
        panic!("piano roll");
    };
    let Music::PianoRoll(mapped_roll) = mapped else {
        panic!("piano roll");
    };
    assert_eq!(diatonic_roll.items[0].note.pitch.to_midi(), Some(64));
    assert_eq!(mapped_roll.items[1].note.pitch.class, PitchClass::D);
}

#[test]
fn pitch_invert_shift_loop_and_slice_return_canonical_rolls() {
    let melody = melody_fixture();
    let inverted = pitch_invert(&melody, Pitch::from_midi(64)).expect("invert");
    let shifted = shift_octave(&inverted, 1).expect("shift octave");
    let looped = loop_n(&shifted, 2).expect("loop");
    let sliced = slice(&looped, Ratio::new(1, 4), Ratio::new(3, 4)).expect("slice");
    let Music::PianoRoll(roll) = sliced else {
        panic!("piano roll");
    };
    assert!(!roll.items.is_empty());
}

#[test]
fn transpose_transform_variants_cover_pitch_deltas() {
    let melody = simple_melody(&[(60, quarter()), (62, quarter())]);
    let scale = Scale::major(PitchClass::C);

    let semitones = transform(TransposeTransform::new(PitchDelta::Semitones(2)).apply(&melody));
    let octaves = transform(TransposeTransform::new(PitchDelta::Octaves(1)).apply(&melody));
    let degrees = transform_report(
        TransposeTransform::new(PitchDelta::ScaleDegrees { scale, steps: 1 }).apply_report(&melody),
    );
    let ratio = transform(
        TransposeTransform::new(PitchDelta::FrequencyRatio(Ratio::new(2, 1))).apply(&melody),
    );
    let custom = TransposeTransform::new(PitchDelta::Custom(CallablePitchMap::new("up-five", 5)))
        .apply(&melody)
        .expect("custom transpose");

    assert_eq!(roll_midis(&semitones), vec![62, 64]);
    assert_eq!(roll_midis(&octaves), vec![72, 74]);
    assert_eq!(roll_midis(&degrees.music), vec![62, 64]);
    assert!(!degrees.has_diagnostics());
    assert_eq!(roll_midis(&ratio), vec![72, 74]);
    assert_eq!(roll_midis(&custom), vec![65, 67]);
}

#[test]
fn invert_transform_variants_cover_axes() {
    let melody = simple_melody(&[(60, quarter()), (64, quarter()), (67, quarter())]);
    let scale = Scale::major(PitchClass::C);
    let chord = PitchChord::from_root_intervals(Pitch::from_midi(60), &[4, 7]);

    let around_pitch =
        transform(InvertTransform::new(PitchAxis::Pitch(Pitch::from_midi(60))).apply(&melody));
    let around_class =
        transform(InvertTransform::new(PitchAxis::PitchClass(PitchClass::C)).apply(&melody));
    let around_degree = transform_report(
        InvertTransform::new(PitchAxis::ScaleDegree {
            scale,
            degree: 1,
            octave: 4,
        })
        .apply_report(&melody),
    );
    let around_root =
        transform_report(InvertTransform::new(PitchAxis::ChordRoot(chord)).apply_report(&melody));
    let around_frequency = transform_report(
        InvertTransform::new(PitchAxis::Frequency(Pitch::from_midi(60))).apply_report(&melody),
    );
    let around_custom = transform_report(
        InvertTransform::new(PitchAxis::Custom(CustomPitchAxis::new(
            "middle-c",
            Pitch::from_midi(60),
        )))
        .apply_report(&melody),
    );

    assert_eq!(roll_midis(&around_pitch), vec![60, 56, 53]);
    assert_eq!(roll_midis(&around_class), vec![60, 68, 65]);
    assert_eq!(roll_midis(&around_degree.music), vec![60, 56, 53]);
    assert_eq!(roll_midis(&around_root.music), vec![60, 56, 53]);
    assert_eq!(roll_midis(&around_frequency.music), vec![60, 56, 53]);
    assert_eq!(roll_midis(&around_custom.music), vec![60, 56, 53]);
    assert!(!around_degree.has_diagnostics());
    assert!(!around_root.has_diagnostics());
}

#[test]
fn retrograde_invert_orders_ties_deterministically() {
    let roll = PianoRoll::new(vec![
        TimedNote {
            onset: Time::from_integer(0),
            note: note(67, quarter()),
        },
        TimedNote {
            onset: Time::from_integer(0),
            note: note(60, quarter()),
        },
        TimedNote {
            onset: quarter(),
            note: note(64, quarter()),
        },
    ])
    .expect("roll");
    let chain = TransformChain::new(vec![
        TransformStep::Retrograde(RetrogradeTransform::new(RetrogradeMode::Cutout)),
        TransformStep::Invert(InvertTransform::new(PitchAxis::PitchClass(PitchClass::C))),
    ]);
    let report = transform_report(chain.apply_report(&Music::PianoRoll(roll)));
    let Music::PianoRoll(ref remapped) = report.music else {
        panic!("piano roll");
    };

    let order = remapped
        .items
        .iter()
        .map(|item| (item.onset, item.note.pitch.to_midi().expect("midi pitch")))
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        vec![
            (Time::from_integer(0), 68),
            (quarter(), 60),
            (quarter(), 65)
        ]
    );
    assert!(!report.has_diagnostics());
}

#[test]
fn stretch_policies_cover_ratios_fit_maps_and_warps() {
    let melody = simple_melody(&[(60, quarter()), (62, quarter())]);
    let tempo = transform(StretchPolicy::TempoRatio(Ratio::from_integer(2)).apply(&melody));
    let time = transform(StretchPolicy::TimeRatio(Ratio::from_integer(2)).apply(&melody));
    let fit = transform(StretchPolicy::FitToDuration(Ratio::from_integer(1)).apply(&melody));
    let mapped = transform_report(
        StretchPolicy::TimeMap(vec![
            TimeMapPoint::new(Time::from_integer(0), Time::from_integer(0)),
            TimeMapPoint::new(Ratio::new(1, 2), Time::from_integer(1)),
        ])
        .apply_report(&melody),
    );
    let warped = transform(
        StretchPolicy::WarpMarkers(vec![
            WarpMarker::new(Time::from_integer(0), Time::from_integer(0)),
            WarpMarker::new(Ratio::new(1, 2), Time::from_integer(1)),
        ])
        .apply(&melody),
    );

    assert_eq!(roll_span(&tempo), Ratio::new(1, 4));
    assert_eq!(roll_span(&time), Time::from_integer(1));
    assert_eq!(roll_span(&fit), Time::from_integer(1));
    assert_eq!(roll_span(&mapped.music), Time::from_integer(1));
    assert_eq!(roll_span(&warped), Time::from_integer(1));
    assert!(!mapped.has_diagnostics());
}

#[test]
fn pitch_remap_policies_cover_scale_vector_matrix_and_misc_mapping() {
    let melody = simple_melody(&[(60, quarter()), (62, quarter()), (64, quarter())]);
    let scale = Scale::major(PitchClass::C);
    let scale_remap =
        transform_report(PitchRemap::ScaleDegree { scale, steps: 1 }.apply_report(&melody));
    let vector = PitchRemap::Vector {
        scale,
        offsets: vec![0, 12],
    }
    .apply_report(&melody)
    .expect("vector remap");
    let matrix = PitchRemap::Matrix {
        scale,
        matrix: IntMatrix::new([1, 0, 1], [0, 1, 0], 1),
    }
    .apply_report(&melody)
    .expect("matrix remap");

    assert_eq!(roll_midis(&scale_remap.music), vec![62, 64, 65]);
    assert_eq!(roll_midis(&vector.music), vec![60, 74, 64]);
    assert_eq!(roll_midis(&matrix.music), vec![62, 64, 65]);
    assert!(!scale_remap.has_diagnostics());
    assert!(!vector.has_diagnostics());
    assert!(!matrix.has_diagnostics());

    let chromatic = transform(PitchRemap::Chromatic(1).apply(&melody));
    let pitch_class = PitchRemap::PitchClass {
        from: PitchClass::C,
        to: PitchClass::D,
    }
    .apply(&melody)
    .expect("pitch-class remap");
    let drum_key = transform(PitchRemap::DrumKey([(60, 36)].into_iter().collect()).apply(&melody));
    let chord_tone = transform(PitchRemap::ChordTone { scale, degree: 1 }.apply(&melody));
    let tuning = transform(PitchRemap::Tuning(TuningRemap::new(200)).apply(&melody));
    let callable =
        transform(PitchRemap::Callable(CallablePitchMap::new("down-two", -2)).apply(&melody));

    assert_eq!(roll_midis(&chromatic), vec![61, 63, 65]);
    assert_eq!(roll_midis(&pitch_class), vec![62, 62, 64]);
    assert_eq!(roll_midis(&drum_key), vec![36, 62, 64]);
    assert_eq!(roll_midis(&chord_tone), vec![60, 60, 64]);
    assert_eq!(roll_midis(&tuning), vec![62, 64, 66]);
    assert_eq!(roll_midis(&callable), vec![58, 60, 62]);
}

#[test]
fn transform_chain_reports_impossible_transforms_and_preserves_order() {
    let melody = simple_melody(&[(60, quarter())]);
    let ordered = TransformChain::new(vec![
        TransformStep::Transpose(TransposeTransform::new(PitchDelta::Semitones(2))),
        TransformStep::Remap(PitchRemap::PitchClass {
            from: PitchClass::D,
            to: PitchClass::C,
        }),
    ])
    .apply_report(&melody)
    .expect("ordered chain");
    let impossible = TransformChain::new(vec![
        TransformStep::Remap(PitchRemap::Vector {
            scale: Scale::major(PitchClass::C),
            offsets: Vec::new(),
        }),
        TransformStep::Transpose(TransposeTransform::new(PitchDelta::Semitones(1))),
    ])
    .apply_report(&melody)
    .expect("impossible chain");

    assert_eq!(roll_midis(&ordered.music), vec![60]);
    assert_eq!(roll_midis(&impossible.music), vec![61]);
    assert_eq!(impossible.diagnostics.len(), 1);
    assert_eq!(
        impossible.diagnostics[0].code,
        TransformDiagnosticCode::UnsupportedMapping
    );
}
