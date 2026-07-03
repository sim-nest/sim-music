use sim_lib_music_core::{Channel, PianoRoll, Pitch, PitchClass, PlayEvent, Tick};
use sim_lib_pitch_chord::{
    AutoChordConfig, ChordSequencerConfig, ChordSequencerSlot, ChordSymbol, ScalesChordInput,
    ScalesChordsPlayer, VoicingPolicy,
};
use sim_lib_pitch_scale::{Key, Mode, Scale, ScaleLockPolicy};
use sim_lib_sound_gm::{DrumKeyMap, DrumSound};

use crate::{
    ArpEngineConfig, ArpInputNote, BasslineChordSpan, BasslineConfig, BasslineRootSource,
    BeatMapConfig, BeatMapLane, DualArpMode, PolyStepCell, PolyStepConfig, PolyStepDirection,
    PolyStepLane, QuadNoteConfig, QuadNotePitchRange, QuadNoteRhythm, QuadNoteStreamConfig,
    QuadNoteVelocityRange, player_arp_dual, player_bassline, player_beat_map, player_polystep,
    player_quad_note,
};

fn tick(ticks: i64) -> Tick {
    Tick { ticks, tpq: 480 }
}

fn channel() -> Channel {
    Channel::new(0).expect("channel")
}

fn midi_notes(events: &[PlayEvent]) -> Vec<(i64, String, u8)> {
    events
        .iter()
        .filter_map(|event| match event {
            PlayEvent::Note(note) => note
                .pitch
                .to_midi()
                .map(|midi| (note.time.ticks, note.lane_id.0.clone(), midi)),
            _ => None,
        })
        .collect()
}

#[test]
fn keyboard_scales_dual_arp_recipe_direct_records_to_piano_roll() {
    let scale = Scale::major(PitchClass::C);
    let chord = AutoChordConfig::new(scale).with_note_count(3).unwrap();
    let keyboard_chain =
        ScalesChordsPlayer::from_config(scale, ScaleLockPolicy::Quantize, chord).unwrap();
    let chord_notes = keyboard_chain.process_note(ScalesChordInput::new(Pitch::from_midi(60), 112));
    let arp_notes = chord_notes
        .iter()
        .map(|note| ArpInputNote::new(note.pitch, note.velocity, channel()))
        .collect::<Vec<_>>();
    let dual = player_arp_dual(
        ArpEngineConfig::new("scale-arp-a", tick(120), tick(90)),
        ArpEngineConfig::new("scale-arp-b", tick(240), tick(120)),
        DualArpMode::Parallel,
    );

    let frozen = dual.freeze(&arp_notes, 4);
    let roll = PianoRoll::from_note_events(
        frozen
            .events
            .iter()
            .filter_map(|event| match event {
                PlayEvent::Note(note) => Some(note.clone()),
                _ => None,
            })
            .collect(),
    )
    .expect("direct-recorded piano roll");

    assert_eq!(chord_notes.len(), 3);
    assert_eq!(midi_notes(&frozen.events).len(), roll.items.len());
    assert!(
        roll.items
            .iter()
            .any(|item| item.note.pitch == Pitch::from_midi(60))
    );
    assert!(
        roll.items
            .iter()
            .any(|item| item.note.pitch == Pitch::from_midi(64))
    );
    assert!(
        roll.items
            .iter()
            .any(|item| item.note.pitch == Pitch::from_midi(67))
    );
}

#[test]
fn beat_map_gm_recipe_freezes_stable_drum_notes() {
    let mut config = BeatMapConfig::new(3800);
    config.steps = 8;
    config.density = 100;
    config.complexity = 64;
    config.fill = 0;
    config.lanes = vec![
        BeatMapLane::new("kick", "gm-kick"),
        BeatMapLane::new("snare", "gm-snare"),
        BeatMapLane::new("closed-hat", "gm-hat"),
    ];
    config.kit = DrumKeyMap::custom(
        "recipe-gm",
        [
            DrumSound::new(36, "Kick", ["kick"]),
            DrumSound::new(38, "Snare", ["snare"]),
            DrumSound::new(42, "Closed Hat", ["closed-hat"]),
        ],
    );
    let player = player_beat_map(config);
    let frozen = player.freeze();
    let notes = midi_notes(&frozen.events);

    assert_eq!(player.render(), frozen);
    assert!(
        notes
            .iter()
            .any(|(_, lane, midi)| lane == "gm-kick" && *midi == 36)
    );
    assert!(
        notes
            .iter()
            .any(|(_, lane, midi)| lane == "gm-snare" && *midi == 38)
    );
    assert!(
        notes
            .iter()
            .any(|(_, lane, midi)| lane == "gm-hat" && *midi == 42)
    );
}

#[test]
fn bassline_chord_sequencer_recipe_targets_followed_roots() {
    let key = Key {
        tonic: PitchClass::C,
        mode: Mode::Major,
    };
    let chord_player = ChordSequencerConfig::new(
        key,
        vec![
            ChordSequencerSlot::new(
                ChordSymbol::parse("Cmaj7").unwrap(),
                VoicingPolicy::Closed,
                480,
            )
            .unwrap(),
            ChordSequencerSlot::new(
                ChordSymbol::parse("Fmaj7").unwrap(),
                VoicingPolicy::Closed,
                480,
            )
            .unwrap(),
            ChordSequencerSlot::new(
                ChordSymbol::parse("G7").unwrap(),
                VoicingPolicy::Closed,
                480,
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .player()
    .unwrap();
    let progression = chord_player.render_progression();
    let spans = progression
        .events
        .iter()
        .enumerate()
        .map(|(index, event)| {
            BasslineChordSpan::new(event.chord.root, index as u64 * 4, index as u64 * 4 + 4)
        })
        .collect::<Vec<_>>();
    let config = BasslineConfig::new(Scale::major(PitchClass::C), Pitch::from_midi(36), 3824)
        .with_steps(12)
        .with_density(100);
    let render = player_bassline(config).freeze(&spans);

    assert_eq!(progression.total_ticks, 1440);
    assert!(
        render
            .traces
            .iter()
            .all(|trace| trace.source == BasslineRootSource::ChordFollow)
    );
    assert!(
        render
            .traces
            .iter()
            .any(|trace| trace.root == PitchClass::F)
    );
    assert!(
        render
            .traces
            .iter()
            .any(|trace| trace.root == PitchClass::G)
    );
}

#[test]
fn polystep_quad_note_recipe_uses_explicit_seeds() {
    let poly = player_polystep(
        PolyStepConfig::new(3828).with_steps(6).with_lane(
            PolyStepLane::new("poly-a", 4)
                .with_direction(PolyStepDirection::PingPong)
                .with_step(0, PolyStepCell::note(Pitch::from_midi(60)))
                .with_step(
                    1,
                    PolyStepCell::chord(vec![Pitch::from_midi(64), Pitch::from_midi(67)]),
                )
                .with_step(
                    2,
                    PolyStepCell::note(Pitch::from_midi(72)).with_probability(100),
                ),
        ),
    );
    let quad = player_quad_note(
        QuadNoteConfig::new(Scale::major(PitchClass::C), 3829)
            .with_steps(6)
            .with_stream(
                QuadNoteStreamConfig::new("quad-a", 3830)
                    .with_rhythm(QuadNoteRhythm::Syncopated)
                    .with_density(100)
                    .with_pitch_range(QuadNotePitchRange::new(
                        Pitch::from_midi(48),
                        Pitch::from_midi(72),
                    ))
                    .with_velocity_range(QuadNoteVelocityRange::new(80, 90)),
            ),
    );

    let poly_frozen = poly.freeze();
    let quad_frozen = quad.freeze();

    assert_eq!(poly.render(), poly_frozen);
    assert_eq!(quad.render(), quad_frozen);
    assert!(midi_notes(&poly_frozen.events).len() >= 4);
    assert!(midi_notes(&quad_frozen.events).len() >= 3);
    assert!(quad_frozen.traces.iter().all(|trace| trace.density == 100));
}

#[test]
fn recipe_sources_are_registered_for_generated_docs() {
    for source in [
        include_str!("../recipes/02-player-recipes/keyboard-scales-dual-arp/recipe.toml"),
        include_str!("../recipes/02-player-recipes/beat-map-gm-smf/recipe.toml"),
        include_str!("../recipes/02-player-recipes/bassline-chord-sup/recipe.toml"),
        include_str!("../recipes/02-player-recipes/polystep-quad-note-seeded/recipe.toml"),
    ] {
        assert!(source.contains("golden"));
        assert!(source.contains("codec = \"lisp\""));
    }
}
