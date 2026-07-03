use sim_kernel::Symbol;

use crate::{
    Articulation, ChainDevice, Channel, LaneId, MemoryPerformanceSource, Music, Note,
    PerformanceInput, PerformanceInputBinding, PerformanceIntent, PerformanceSource, PlayerChain,
    PlayerMode, PlayerTargetDescriptor, ScaleLock, Tick, Time, TimeRange,
};

fn tick(ticks: i64) -> Tick {
    Tick::new(ticks, 480).expect("tick")
}

fn channel() -> Channel {
    Channel::new(0).expect("channel")
}

fn source() -> MemoryPerformanceSource {
    let mut source = MemoryPerformanceSource::new(
        Symbol::qualified("music/performance-source", "keyboard"),
        channel(),
    );
    source
        .bind_input(PerformanceInputBinding::new(
            Symbol::qualified("midi/input", "keyboard"),
            LaneId::new("performance"),
            channel(),
        ))
        .expect("bind input");
    source
}

fn player_chain() -> PlayerChain {
    let note = Note::new(
        Time::new(1, 4),
        crate::Pitch::from_midi(48),
        80,
        channel(),
        Articulation::Normal,
    )
    .expect("note");
    PlayerChain::new(
        Symbol::qualified("music/source", "performance-test"),
        Music::Note(note),
        vec![
            ChainDevice::new(
                "route",
                Symbol::qualified("music/player", "route"),
                PlayerMode::Through,
                0,
            )
            .with_route_lane(LaneId::new("routed")),
        ],
        PlayerTargetDescriptor::instrument("default"),
    )
}

#[test]
fn performance_source_tracks_held_notes_sustain_and_panic() {
    let mut source = source();
    source
        .capture_start(Symbol::qualified("music/take", "sustain"))
        .expect("start");

    let events = source
        .poll_events(vec![
            PerformanceInput::note_on(tick(0), channel(), 60, 96),
            PerformanceInput::sustain(tick(120), channel(), true),
            PerformanceInput::note_off(tick(240), channel(), 60, 0),
        ])
        .expect("events");

    assert_eq!(events.len(), 3);
    assert_eq!(source.state().held_note_count(), 1);
    assert!(source.state().sustain_pedal);

    source
        .poll_events(vec![PerformanceInput::sustain(tick(360), channel(), false)])
        .expect("sustain up");
    assert_eq!(source.state().held_note_count(), 0);

    source
        .poll_events(vec![PerformanceInput::note_on(
            tick(480),
            channel(),
            62,
            90,
        )])
        .expect("note on");
    assert_eq!(source.state().held_note_count(), 1);
    let panic_events = source.panic(tick(600)).expect("panic");
    assert!(matches!(
        panic_events.last().map(|event| &event.intent),
        Some(PerformanceIntent::Panic)
    ));
    assert_eq!(source.state().held_note_count(), 0);
}

#[test]
fn octave_transpose_and_scale_lock_are_applied_before_capture() {
    let mut source = source();
    source.set_octave_shift(1);
    source.set_transpose(1);
    source.set_scale_lock(Some(ScaleLock::major()));

    let events = source
        .poll_events(vec![PerformanceInput::note_on(tick(0), channel(), 60, 100)])
        .expect("events");

    let PerformanceIntent::NoteOn { pitch, .. } = events[0].intent else {
        panic!("note on");
    };
    assert_eq!(pitch.to_midi(), Some(72));
}

#[test]
fn captured_take_replays_from_cassette_and_converts_to_clip() {
    let mut source = source();
    source
        .capture_start(Symbol::qualified("music/take", "clip"))
        .expect("start");
    source
        .poll_events(vec![
            PerformanceInput::note_on(tick(0), channel(), 60, 100),
            PerformanceInput::note_off(tick(240), channel(), 60, 0),
        ])
        .expect("events");
    let take = source.capture_stop().expect("take");

    assert_eq!(take.replay_events().expect("replay"), take.events);
    assert_eq!(
        take.replay_content_hash().expect("replay hash"),
        take.content_hash()
    );
    assert_eq!(take.cassette().envelopes().len(), 2);

    let Music::PianoRoll(clip) = take.as_clip().expect("clip") else {
        panic!("piano roll");
    };
    assert_eq!(clip.items.len(), 1);
    assert_eq!(clip.items[0].onset, Time::new(0, 1));
    assert_eq!(clip.items[0].note.duration, Time::new(1, 8));
}

#[test]
fn performance_take_feeds_player_chain_like_stored_events() {
    let mut source = source();
    source
        .capture_start(Symbol::qualified("music/take", "chain"))
        .expect("start");
    source
        .poll_events(vec![
            PerformanceInput::note_on(tick(0), channel(), 60, 100),
            PerformanceInput::note_off(tick(120), channel(), 60, 0),
        ])
        .expect("events");
    let take = source.capture_stop().expect("take");
    let range = TimeRange::from_ticks(0, 960, 480).expect("range");
    let cx = take
        .player_chain_context(&crate::PlayContext::new(range))
        .expect("context");
    let render = player_chain().render_chain(&cx).expect("render");

    let routed_notes = render
        .events
        .iter()
        .filter_map(|event| match event {
            crate::PlayEvent::Note(note) if note.pitch.to_midi() == Some(60) => Some(note),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(routed_notes.len(), 1);
    assert_eq!(routed_notes[0].lane_id, LaneId::new("routed"));
}
