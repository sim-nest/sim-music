use super::*;
use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_pitch_scale::{Key, Mode, Scale};

#[test]
fn chord_pitch_class_mask_matches_bit_chord() {
    let chord = Chord::from_root_intervals(Pitch::from_midi(60), &[4, 7]);
    assert_eq!(chord.bit_chord().mask, chord.pitch_classes());
}

#[test]
fn inversion_and_transpose_preserve_shape() {
    let chord = Chord::from_root_intervals(Pitch::from_midi(60), &[4, 7, 11]);
    assert_eq!(chord.invert(2).pitches().len(), chord.pitches().len());
    assert_eq!(chord.transpose(2).notes[0].class, PitchClass::D);
}

#[test]
fn subset_chord_symbol_parser_handles_slash_bass() {
    let symbol = ChordSymbol::parse("Cmaj7/E").unwrap();
    assert_eq!(symbol.root, PitchClass::C);
    assert_eq!(symbol.slash_bass, Some(PitchClass::E));
    let chord = symbol.to_chord(4);
    assert_eq!(chord.pitches().len(), 5);
}

#[test]
fn chord_tones_follow_scale() {
    let chord = Chord::chord_tones_in(Scale::major(PitchClass::C), 2, 4);
    assert_eq!(
        chord
            .pitches()
            .into_iter()
            .map(|pitch| pitch.class)
            .collect::<Vec<_>>(),
        vec![PitchClass::D, PitchClass::F, PitchClass::A]
    );
}

#[test]
fn scale_stack_player_generates_single_note_chords() {
    let player = AutoChordPlayer::new(AutoChordConfig::new(Scale::major(PitchClass::C))).unwrap();

    let notes = player.render_note(ScalesChordInput::new(Pitch::from_midi(62), 96));

    assert_eq!(
        notes.iter().map(|note| note.pitch).collect::<Vec<_>>(),
        vec![
            Pitch::from_midi(62),
            Pitch::from_midi(65),
            Pitch::from_midi(69)
        ]
    );
    assert!(notes.iter().all(|note| note.velocity == 96));
}

#[test]
fn per_degree_chord_type_and_note_count_are_applied() {
    let config = AutoChordConfig::new(Scale::major(PitchClass::C))
        .with_degree_chords(vec![DegreeChordType::new(5, ChordType::DominantSeventh)])
        .unwrap()
        .with_note_count(4)
        .unwrap();
    let player = AutoChordPlayer::new(config).unwrap();

    let notes = player.render_note(ScalesChordInput::new(Pitch::from_midi(67), 90));

    assert_eq!(
        notes.iter().map(|note| note.pitch).collect::<Vec<_>>(),
        vec![
            Pitch::from_midi(67),
            Pitch::from_midi(71),
            Pitch::from_midi(74),
            Pitch::from_midi(77),
        ]
    );
}

#[test]
fn inversion_open_drop_and_velocity_policies_are_deterministic() {
    let config = AutoChordConfig::new(Scale::major(PitchClass::C))
        .with_note_count(4)
        .unwrap()
        .with_inversion(1)
        .with_voicing(VoicingPolicy::Open { spread: 12 })
        .with_octave_shift(1)
        .with_velocity(VelocityPolicy::Offset(-20));
    let player = AutoChordPlayer::new(config).unwrap();
    let input = ScalesChordInput::new(Pitch::from_midi(60), 100);

    let first = player.render_note(input);
    let second = player.render_note(input);

    assert_eq!(first, second);
    assert_eq!(
        first.iter().map(|note| note.pitch).collect::<Vec<_>>(),
        vec![
            Pitch::from_midi(76),
            Pitch::from_midi(91),
            Pitch::from_midi(107),
            Pitch::from_midi(120),
        ]
    );
    assert!(first.iter().all(|note| note.velocity == 80));

    let drop = VoicingPolicy::Drop {
        voice_index_from_top: 1,
        octaves: 1,
    }
    .apply(vec![
        Pitch::from_midi(60),
        Pitch::from_midi(64),
        Pitch::from_midi(67),
    ]);
    assert_eq!(
        drop,
        vec![
            Pitch::from_midi(52),
            Pitch::from_midi(60),
            Pitch::from_midi(67)
        ]
    );
}

#[test]
fn combined_scales_chords_player_filters_before_chording() {
    let chord = AutoChordPlayer::new(AutoChordConfig::new(Scale::major(PitchClass::C))).unwrap();
    let player = ScalesChordsPlayer::new(
        sim_lib_pitch_scale::ScaleLockPlayer::from_scale(
            Scale::major(PitchClass::C),
            sim_lib_pitch_scale::ScaleLockPolicy::Filter,
        ),
        chord,
    );

    assert!(
        player
            .process_note(ScalesChordInput::new(Pitch::from_midi(61), 100))
            .is_empty()
    );
    assert_eq!(
        player
            .process_note(ScalesChordInput::new(Pitch::from_midi(60), 100))
            .len(),
        3
    );
}

#[test]
fn chord_sequencer_renders_progression_slots_with_voicing_and_duration() {
    let player = chord_sequence_config().player().unwrap();

    let render = player.render_progression();

    assert_eq!(render.total_ticks, 1920);
    assert_eq!(
        render
            .events
            .iter()
            .map(|event| (event.slot_index, event.start_tick, event.duration_ticks))
            .collect::<Vec<_>>(),
        vec![(0, 0, 480), (1, 480, 960), (2, 1440, 480)]
    );
    assert_eq!(
        render.events[0]
            .notes
            .iter()
            .map(|note| note.pitch)
            .collect::<Vec<_>>(),
        vec![
            Pitch::from_midi(60),
            Pitch::from_midi(64),
            Pitch::from_midi(67),
            Pitch::from_midi(71),
        ]
    );
    assert_eq!(
        render.events[1]
            .notes
            .iter()
            .map(|note| note.pitch)
            .collect::<Vec<_>>(),
        vec![
            Pitch::from_midi(65),
            Pitch::from_midi(81),
            Pitch::from_midi(96),
        ]
    );
    assert_eq!(render.events[0].roman, "Imaj7");
    assert!(render.events[2].suggestions[0].roman == "I");
}

#[test]
fn chord_sequencer_single_key_triggering_selects_slots() {
    let player = chord_sequence_config().player().unwrap();

    assert_eq!(player.slot_index_for_trigger(Pitch::from_midi(65)), Some(1));
    assert_eq!(player.slot_index_for_trigger(Pitch::from_midi(64)), Some(2));

    let event = player
        .trigger(ChordSequenceInput::new(Pitch::from_midi(65), 100))
        .expect("triggered slot");

    assert_eq!(event.slot_index, 1);
    assert_eq!(event.trigger, Some(Pitch::from_midi(65)));
    assert!(event.notes.iter().all(|note| note.velocity == 88));
}

#[test]
fn harmonic_suggestion_ranking_is_stable() {
    let context = HarmonicSuggestionContext::new(
        Key {
            tonic: PitchClass::C,
            mode: Mode::Major,
        },
        ChordSymbol::parse("G7").unwrap(),
    )
    .with_max_candidates(5);

    let first = suggest_harmony(context.clone());
    let second = suggest_harmony(context);

    assert_eq!(first, second);
    assert_eq!(
        first
            .iter()
            .map(|suggestion| suggestion.roman.as_str())
            .collect::<Vec<_>>(),
        vec!["I", "vi", "IV", "ii", "subV7"]
    );
    assert_eq!(first[0].function, HarmonicFunction::Tonic);
    assert!(first.iter().any(|suggestion| suggestion.substitution));
    assert!(first.windows(2).all(|pair| pair[0].score >= pair[1].score));
}

#[test]
fn chord_sequencer_config_wire_round_trips() {
    let config = chord_sequence_config();

    let wire = config.to_wire();
    let parsed = ChordSequencerConfig::from_wire(&wire).unwrap();

    assert_eq!(parsed, config);
    assert_eq!(
        parsed.player().unwrap().render_progression(),
        config.player().unwrap().render_progression()
    );
}

fn chord_sequence_config() -> ChordSequencerConfig {
    ChordSequencerConfig::new(
        Key {
            tonic: PitchClass::C,
            mode: Mode::Major,
        },
        vec![
            ChordSequencerSlot::new(
                ChordSymbol::parse("Cmaj7").unwrap(),
                VoicingPolicy::Closed,
                480,
            )
            .unwrap()
            .with_trigger(Pitch::from_midi(60))
            .with_velocity(VelocityPolicy::Preserve),
            ChordSequencerSlot::new(
                ChordSymbol::parse("F").unwrap(),
                VoicingPolicy::Open { spread: 12 },
                960,
            )
            .unwrap()
            .with_trigger(Pitch::from_midi(65))
            .with_velocity(VelocityPolicy::Fixed(88)),
            ChordSequencerSlot::new(
                ChordSymbol::parse("G7").unwrap(),
                VoicingPolicy::Drop {
                    voice_index_from_top: 1,
                    octaves: 1,
                },
                480,
            )
            .unwrap()
            .with_velocity(VelocityPolicy::Offset(-8)),
        ],
    )
    .unwrap()
    .with_root_octave(4)
    .with_default_velocity(96)
    .with_suggestion_limit(5)
}
