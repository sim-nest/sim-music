use num_rational::Ratio;
use sim_lib_music_core::{
    Articulation, Channel, LaneId, MusicObject, Note, Pitch, PitchClass, PlayEvent, Tick,
};
use sim_lib_pitch_scale::Scale;
use sim_lib_sound_gm::{DrumKeyMap, DrumSound};

use crate::{
    AnchorPolicy, ArpDirection, ArpEngineConfig, ArpEngineSlot, ArpInputNote, ArpStepKind,
    ArpTraceAction, ArpTraceSource, BasslineChordSpan, BasslineConfig, BasslineOctaveRange,
    BasslineRootSource, BeatMapConfig, BeatMapLane, DualArpMode, EuclidConfig, EuclidLane,
    KeyRange, KeySplitConfig, MelodyItem, MovementPattern, MovementStep, MovementTransform,
    NoteOrderPolicy, PatternAutomation, PatternRegion, PolyStepCell, PolyStepConfig,
    PolyStepDirection, PolyStepLane, PolyStepRecordInput, QUAD_NOTE_MAX_STREAMS, QuadNoteConfig,
    QuadNoteHarmonicRelation, QuadNotePitchRange, QuadNoteRhythm, QuadNoteStreamConfig,
    QuadNoteVelocityRange, boxed, counterpoint, melody, par, player_arp_dual, player_arp_lab,
    player_bassline, player_beat_map, player_euclid, player_polystep, player_quad_note, seq,
};

fn quarter() -> Ratio<i64> {
    Ratio::new(1, 4)
}

fn note(midi: u8) -> Note {
    Note::new(
        quarter(),
        Pitch::from_midi(midi),
        100,
        Channel::new(0).expect("channel"),
        Articulation::Normal,
    )
    .expect("note")
}

fn tick(ticks: i64) -> Tick {
    Tick { ticks, tpq: 480 }
}

fn arp_note(midi: u8) -> ArpInputNote {
    ArpInputNote::from_midi(midi, 100, Channel::new(0).expect("channel"))
}

fn engine(lane: &str) -> ArpEngineConfig {
    ArpEngineConfig::new(lane, tick(120), tick(90))
}

fn lane_midis(events: &[PlayEvent], lane: &str) -> Vec<u8> {
    events
        .iter()
        .filter_map(|event| match event {
            PlayEvent::Note(note) if note.lane_id == LaneId::new(lane) => note.pitch.to_midi(),
            _ => None,
        })
        .collect()
}

fn drum_notes(events: &[PlayEvent]) -> Vec<(i64, String, u8, u8)> {
    events
        .iter()
        .filter_map(|event| match event {
            PlayEvent::Note(note) => note
                .pitch
                .to_midi()
                .map(|midi| (note.time.ticks, note.lane_id.0.clone(), midi, note.velocity)),
            _ => None,
        })
        .collect()
}

fn note_events(events: &[PlayEvent], lane: &str) -> Vec<(i64, u8, u8, i64)> {
    events
        .iter()
        .filter_map(|event| match event {
            PlayEvent::Note(note) if note.lane_id == LaneId::new(lane) => note
                .pitch
                .to_midi()
                .map(|midi| (note.time.ticks, midi, note.velocity, note.duration.ticks)),
            _ => None,
        })
        .collect()
}

#[test]
fn par_and_seq_build_expected_durations() {
    let seq = seq(vec![boxed(note(60)), boxed(note(64))]);
    let par = par(vec![boxed(note(60)), boxed(note(64))]);
    assert_eq!(seq.duration(), Ratio::new(1, 2));
    assert_eq!(par.duration(), quarter());
}

#[test]
fn counterpoint_normalizes_voice_names() {
    let line = melody(vec![MelodyItem::Note(note(60))]).expect("melody");
    let cp = counterpoint(vec![line], Vec::new()).expect("counterpoint");
    assert_eq!(cp.voice_names, vec!["Voice 1".to_owned()]);
}

#[test]
fn dual_arpeggio_sync_direction_tie_rest_and_gate_masks_are_stable() {
    let notes = vec![arp_note(60), arp_note(64)];
    let a = engine("arp-a")
        .with_pattern(
            vec![
                ArpStepKind::Play,
                ArpStepKind::Tie,
                ArpStepKind::Rest,
                ArpStepKind::Play,
            ],
            4,
        )
        .with_note_order(NoteOrderPolicy::PitchAscending);
    let b = engine("arp-b")
        .with_direction(ArpDirection::Down)
        .with_octave_range(2);
    let dual = player_arp_dual(a, b, DualArpMode::Parallel);

    let render = dual.render(&notes, 4);

    assert_eq!(dual.freeze(&notes, 4), render);
    assert_eq!(lane_midis(&render.events, "arp-a"), vec![60, 64]);
    assert_eq!(lane_midis(&render.events, "arp-b"), vec![76, 72, 64, 60]);
    assert_eq!(render.gate_masks.len(), 8);
    assert!(
        render
            .gate_masks
            .iter()
            .any(|frame| frame.time == tick(240) && !frame.gate_open && !frame.mask_open)
    );
    assert!(
        render
            .traces
            .iter()
            .any(|trace| trace.action == ArpTraceAction::Tied
                && trace.pitch == Some(Pitch::from_midi(60)))
    );
    let zero_traces = render
        .traces
        .iter()
        .filter(|trace| trace.step == 0 && trace.time == tick(0))
        .collect::<Vec<_>>();
    assert_eq!(zero_traces.len(), 2);
}

#[test]
fn key_split_routes_lanes_and_passes_notes_outside_ranges() {
    let lower = engine("lower").with_note_order(NoteOrderPolicy::Input);
    let upper =
        ArpEngineConfig::new("upper", tick(240), tick(180)).with_note_order(NoteOrderPolicy::Input);
    let mut split = KeySplitConfig::new(Pitch::from_midi(60));
    split.lower_range = KeyRange::new(Some(Pitch::from_midi(48)), Some(Pitch::from_midi(59)));
    split.upper_range = KeyRange::new(Some(Pitch::from_midi(72)), Some(Pitch::from_midi(84)));
    let dual = player_arp_dual(lower, upper, DualArpMode::KeySplit(split));

    let render = dual.render(&[arp_note(55), arp_note(64), arp_note(76)], 2);

    assert_eq!(lane_midis(&render.events, "lower"), vec![55, 55]);
    assert_eq!(lane_midis(&render.events, "upper"), vec![76, 76]);
    assert_eq!(lane_midis(&render.events, "arp-pass-through"), vec![64]);
    assert!(
        render
            .traces
            .iter()
            .any(|trace| trace.source == ArpTraceSource::PassThrough)
    );
    assert!(
        render
            .traces
            .iter()
            .any(|trace| trace.source == ArpTraceSource::Engine(ArpEngineSlot::A))
    );
    assert!(
        render
            .traces
            .iter()
            .any(|trace| trace.source == ArpTraceSource::Engine(ArpEngineSlot::B))
    );
}

#[test]
fn arpeggio_lab_splits_anchor_and_transforms_movement() {
    let movement_engine = engine("movement").with_note_order(NoteOrderPolicy::Input);
    let mut config = crate::ArpLabConfig::new(movement_engine);
    config.anchor_policy = AnchorPolicy::Lowest(1);
    config.movement_pattern =
        MovementPattern::new(vec![MovementStep::new(0, 0), MovementStep::new(1, 0)]);
    config.movement_transform = MovementTransform::transpose(12);
    let lab = player_arp_lab(config);

    let render = lab.render(&[arp_note(60), arp_note(64), arp_note(67)], 2);

    assert_eq!(
        render
            .roles
            .anchors
            .iter()
            .filter_map(|note| note.pitch.to_midi())
            .collect::<Vec<_>>(),
        vec![60]
    );
    assert_eq!(
        render
            .roles
            .movement
            .iter()
            .filter_map(|note| note.pitch.to_midi())
            .collect::<Vec<_>>(),
        vec![64, 67]
    );
    assert_eq!(
        lane_midis(&render.output.events, "arp-lab-anchor"),
        vec![60]
    );
    assert_eq!(lane_midis(&render.output.events, "movement"), vec![76, 79]);
    assert!(
        render
            .output
            .traces
            .iter()
            .any(|trace| trace.action == ArpTraceAction::HeldAnchor)
    );
    assert_eq!(
        lab.freeze(&[arp_note(60), arp_note(64), arp_note(67)], 2),
        render
    );
}

#[test]
fn beat_map_coordinates_seeded_fills_and_freeze_are_stable() {
    let mut config = BeatMapConfig::new(7);
    config.x = 12;
    config.y = 88;
    config.density = 42;
    config.complexity = 70;
    config.fill = 100;
    config.steps = 16;
    let player = player_beat_map(config.clone());

    let render = player.render();

    assert_eq!(player.freeze(), render);
    assert_eq!(player_beat_map(config).render(), render);
    assert!(render.traces.iter().any(|trace| trace.step >= 12));
}

#[test]
fn beat_map_density_is_monotonic_for_same_coordinates() {
    let mut sparse = BeatMapConfig::new(13);
    sparse.density = 10;
    sparse.steps = 16;
    sparse.fill = 0;
    let mut dense = sparse.clone();
    dense.density = 80;

    let sparse_count = drum_notes(&player_beat_map(sparse).render().events).len();
    let dense_count = drum_notes(&player_beat_map(dense).render().events).len();

    assert!(sparse_count <= dense_count);
    assert!(dense_count > sparse_count);
}

#[test]
fn beat_map_uses_custom_drum_key_remap_and_region_hooks() {
    let mut config = BeatMapConfig::new(21);
    config.density = 100;
    config.steps = 8;
    config.lanes = vec![BeatMapLane::new("backbeat", "remapped")];
    config.kit = DrumKeyMap::custom(
        "test",
        [DrumSound::new(40, "Deep Snare", ["backbeat", "snare"])],
    );
    config.automation = PatternAutomation::active_in(vec![PatternRegion::new(2, 4)]);
    let render = player_beat_map(config).render();

    assert_eq!(
        drum_notes(&render.events)
            .into_iter()
            .map(|(time, lane, key, _)| (time, lane, key))
            .collect::<Vec<_>>(),
        vec![
            (240, "remapped".to_owned(), 40),
            (360, "remapped".to_owned(), 40),
        ]
    );
}

#[test]
fn euclid_renders_rotated_accented_lane_output_and_freezes() {
    let config = EuclidConfig::new(16)
        .with_lane(
            EuclidLane::new("kick", 4, 16)
                .with_accent_every(2)
                .with_lane("kick-lane"),
        )
        .with_lane(
            EuclidLane::new("snare", 2, 8)
                .with_rotation(1)
                .with_lane("snare-lane"),
        );
    let player = player_euclid(config);

    let render = player.render();

    assert_eq!(player.freeze(), render);
    assert_eq!(
        drum_notes(&render.events)
            .into_iter()
            .filter(|(_, lane, _, _)| lane == "kick-lane")
            .collect::<Vec<_>>(),
        vec![
            (0, "kick-lane".to_owned(), 36, 86),
            (480, "kick-lane".to_owned(), 36, 112),
            (960, "kick-lane".to_owned(), 36, 86),
            (1440, "kick-lane".to_owned(), 36, 112),
        ]
    );
    assert_eq!(
        drum_notes(&render.events)
            .into_iter()
            .filter(|(_, lane, _, _)| lane == "snare-lane")
            .map(|(time, _, key, _)| (time, key))
            .collect::<Vec<_>>(),
        vec![(360, 38), (840, 38), (1320, 38), (1800, 38)]
    );
}

#[test]
fn bassline_conforms_to_key_and_self_clocks_from_held_root() {
    let config = bassline_config(31).with_steps(8).with_density(100);
    let scale = config.scale;
    let player = player_bassline(config);

    let render = player.render(&[]);

    assert_eq!(player.freeze(&[]), render);
    assert_eq!(note_events(&render.events, "bassline")[0].1, 36);
    assert!(
        render
            .traces
            .iter()
            .all(|trace| scale.degree_of(trace.pitch.class).is_some())
    );
    assert!(
        render
            .traces
            .iter()
            .all(|trace| trace.source == BasslineRootSource::HeldRoot)
    );
}

#[test]
fn bassline_follows_upstream_chords_when_present() {
    let config = bassline_config(41).with_steps(8).with_density(100);
    let player = player_bassline(config);

    let render = player.render(&[
        BasslineChordSpan::new(PitchClass::F, 0, 4),
        BasslineChordSpan::new(PitchClass::G, 4, 8),
    ]);

    let downbeats = render
        .traces
        .iter()
        .filter(|trace| trace.step == 0 || trace.step == 4)
        .map(|trace| (trace.step, trace.root, trace.pitch.class, trace.source))
        .collect::<Vec<_>>();
    assert_eq!(
        downbeats,
        vec![
            (
                0,
                PitchClass::F,
                PitchClass::F,
                BasslineRootSource::ChordFollow,
            ),
            (
                4,
                PitchClass::G,
                PitchClass::G,
                BasslineRootSource::ChordFollow,
            ),
        ]
    );
}

#[test]
fn bassline_seeded_variation_ghost_slide_and_freeze_are_stable() {
    let mut config = bassline_config(51).with_steps(8).with_density(100);
    config.octave_range = BasslineOctaveRange::new(2, 3);
    config.accent_every = 0;
    config.ghost_notes = 100;
    config.slide = 100;
    config.note_length = tick(60);
    let player = player_bassline(config.clone());

    let render = player.render(&[]);
    let varied = player_bassline(BasslineConfig { seed: 52, ..config }).render(&[]);

    assert_eq!(player.freeze(&[]), render);
    assert_ne!(render, varied);
    assert!(render.traces.iter().all(|trace| trace.ghost));
    assert!(render.traces.iter().all(|trace| trace.slide));
    assert!(
        note_events(&render.events, "bassline")
            .iter()
            .all(|(_, _, _, duration)| *duration == 120)
    );
}

#[test]
fn polystep_renders_polyrhythmic_lane_lengths_and_routing() {
    let lead = PolyStepLane::new("poly-lead", 3)
        .with_step(0, PolyStepCell::note(Pitch::from_midi(60)))
        .with_step(
            1,
            PolyStepCell::chord(vec![Pitch::from_midi(64), Pitch::from_midi(67)]),
        )
        .with_step(2, PolyStepCell::note(Pitch::from_midi(72)));
    let bass = PolyStepLane::new("poly-bass", 2)
        .with_direction(PolyStepDirection::Reverse)
        .with_channel(Channel(1))
        .with_step(0, PolyStepCell::note(Pitch::from_midi(36)))
        .with_step(1, PolyStepCell::note(Pitch::from_midi(43)));
    let player = player_polystep(
        PolyStepConfig::new(5)
            .with_steps(6)
            .with_rate(tick(120))
            .with_lane(lead)
            .with_lane(bass),
    );

    let render = player.render();

    assert_eq!(player.freeze(), render);
    assert_eq!(
        lane_midis(&render.events, "poly-lead"),
        vec![60, 64, 67, 72, 60, 64, 67, 72]
    );
    assert_eq!(
        lane_midis(&render.events, "poly-bass"),
        vec![43, 36, 43, 36, 43, 36]
    );
    assert!(
        render
            .traces
            .iter()
            .any(|trace| trace.lane_id == LaneId::new("poly-bass")
                && trace.direction == PolyStepDirection::Reverse)
    );
}

#[test]
fn polystep_probability_seed_ratchet_tie_slide_and_freeze_are_stable() {
    let lane = PolyStepLane::new("poly-prob", 1).with_step(
        0,
        PolyStepCell::note(Pitch::from_midi(60))
            .with_probability(50)
            .with_ratchet(3)
            .with_gate(tick(60))
            .with_tie(true)
            .with_slide(true),
    );
    let config = PolyStepConfig::new(19)
        .with_steps(16)
        .with_rate(tick(120))
        .with_lane(lane);
    let player = player_polystep(config.clone());

    let render = player.render();
    let varied = player_polystep(PolyStepConfig { seed: 23, ..config }).render();

    assert_eq!(player.freeze(), render);
    assert_ne!(render, varied);
    assert!(render.traces.iter().any(|trace| trace.emitted
        && trace.probability == 50
        && trace.ratchet == 3
        && trace.tie
        && trace.slide));
    assert!(
        note_events(&render.events, "poly-prob")
            .iter()
            .all(|(_, _, _, duration)| *duration == 120)
    );
    let emitted_time = render
        .traces
        .iter()
        .find(|trace| trace.emitted)
        .expect("emitted probabilistic step")
        .time
        .ticks;
    assert!(
        note_events(&render.events, "poly-prob")
            .windows(3)
            .any(
                |window| window.iter().map(|event| event.0).collect::<Vec<_>>()
                    == vec![emitted_time, emitted_time + 40, emitted_time + 80]
            )
    );
}

#[test]
fn polystep_step_record_captures_performance_input() {
    let mut lane = PolyStepLane::new("poly-record", 4);
    lane.record_input(PolyStepRecordInput::new(2, Pitch::from_midi(72), 111).with_gate(tick(240)));
    let player = player_polystep(PolyStepConfig::new(0).with_steps(4).with_lane(lane));

    let render = player.render();

    assert_eq!(
        note_events(&render.events, "poly-record"),
        vec![(240, 72, 111, 240)]
    );
}

#[test]
fn quad_note_streams_are_seeded_and_freeze_stably() {
    let stream_a =
        quad_stream("quad-a", 1)
            .with_density(100)
            .with_pitch_range(QuadNotePitchRange::new(
                Pitch::from_midi(48),
                Pitch::from_midi(72),
            ));
    let stream_b = quad_stream("quad-b", 2)
        .with_density(100)
        .with_rhythm(QuadNoteRhythm::Syncopated)
        .with_velocity_range(QuadNoteVelocityRange::new(40, 120));
    let config = QuadNoteConfig::new(Scale::major(PitchClass::C), 101)
        .with_steps(8)
        .with_rate(tick(120))
        .with_note_length(tick(80))
        .with_stream(stream_a.clone())
        .with_stream(stream_b.clone());
    let player = player_quad_note(config.clone());

    let render = player.render();
    let changed_stream_seed = player_quad_note(QuadNoteConfig {
        streams: vec![
            stream_a,
            QuadNoteStreamConfig {
                seed: 99,
                ..stream_b
            },
        ],
        ..config.clone()
    })
    .render();

    assert_eq!(player.freeze(), render);
    assert_eq!(player_quad_note(config).render(), render);
    assert_eq!(
        note_events(&render.events, "quad-a"),
        note_events(&changed_stream_seed.events, "quad-a")
    );
    assert_ne!(
        note_events(&render.events, "quad-b"),
        note_events(&changed_stream_seed.events, "quad-b")
    );
}

#[test]
fn quad_note_locks_streams_to_scale_and_harmonic_relation() {
    let scale = Scale::major(PitchClass::C);
    let player =
        player_quad_note(
            QuadNoteConfig::new(scale, 202)
                .with_harmonic_relation(QuadNoteHarmonicRelation::Fifths)
                .with_steps(1)
                .with_stream(
                    quad_stream("quad-root", 1).with_pitch_range(QuadNotePitchRange::new(
                        Pitch::from_midi(60),
                        Pitch::from_midi(60),
                    )),
                )
                .with_stream(quad_stream("quad-fifth", 2).with_pitch_range(
                    QuadNotePitchRange::new(Pitch::from_midi(67), Pitch::from_midi(67)),
                )),
        );

    let render = player.render();

    assert_eq!(lane_midis(&render.events, "quad-root"), vec![60]);
    assert_eq!(lane_midis(&render.events, "quad-fifth"), vec![67]);
    assert!(render.traces.iter().all(|trace| {
        trace
            .pitch
            .map(|pitch| scale.degree_of(pitch.class).is_some())
            .unwrap_or(true)
    }));
    assert!(render.traces.iter().any(
        |trace| trace.lane_id == LaneId::new("quad-fifth") && trace.relation_degree_offset == 4
    ));
}

#[test]
fn quad_note_limits_rendering_to_four_streams() {
    let mut config = QuadNoteConfig::new(Scale::major(PitchClass::C), 303).with_steps(1);
    for index in 0..5 {
        config = config.with_stream(quad_stream(format!("quad-{index}"), index as u64));
    }
    let render = player_quad_note(config).render();

    for index in 0..QUAD_NOTE_MAX_STREAMS {
        assert!(!lane_midis(&render.events, &format!("quad-{index}")).is_empty());
    }
    assert!(lane_midis(&render.events, "quad-4").is_empty());
    assert_eq!(render.traces.len(), QUAD_NOTE_MAX_STREAMS);
}

fn bassline_config(seed: u64) -> BasslineConfig {
    BasslineConfig::new(Scale::major(PitchClass::C), Pitch::from_midi(36), seed)
        .with_octave_range(BasslineOctaveRange::new(2, 2))
}

fn quad_stream(lane: impl Into<String>, seed: u64) -> QuadNoteStreamConfig {
    QuadNoteStreamConfig::new(lane, seed)
        .with_density(100)
        .with_pitch_range(QuadNotePitchRange::new(
            Pitch::from_midi(60),
            Pitch::from_midi(72),
        ))
}
