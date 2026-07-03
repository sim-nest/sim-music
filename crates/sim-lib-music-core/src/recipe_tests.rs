use num_rational::Ratio;
use sim_kernel::Symbol;

use crate::{
    Channel, LaneId, MemoryPerformanceSource, MusicCapability, PerformanceInput,
    PerformanceInputBinding, PerformanceSource, PianoRoll, Tick, default_music_component_registry,
    lfo_modulator_id, note_echo_player_id, pattern_mutator_player_id,
};

fn tick(ticks: i64) -> Tick {
    Tick { ticks, tpq: 480 }
}

fn channel() -> Channel {
    Channel::new(0).expect("channel")
}

fn param_names(entry: &crate::MusicComponentRegistryEntry) -> Vec<String> {
    entry
        .descriptor()
        .params
        .iter()
        .map(|param| param.id.name.to_string())
        .collect()
}

#[test]
fn captured_take_recipe_replays_and_direct_records_to_piano_roll() {
    let mut source = MemoryPerformanceSource::new(
        Symbol::qualified("music/performance-source", "keyboard"),
        channel(),
    );
    source
        .bind_input(PerformanceInputBinding::new(
            Symbol::qualified("midi/input", "keyboard"),
            LaneId::new("keyboard"),
            channel(),
        ))
        .expect("binding");
    source
        .capture_start(Symbol::qualified("music/take", "keyboard-direct-record"))
        .expect("capture start");
    source
        .poll_events(vec![
            PerformanceInput::note_on(tick(0), channel(), 60, 108),
            PerformanceInput::note_off(tick(240), channel(), 60, 0),
        ])
        .expect("events");
    let take = source.capture_stop().expect("capture stop");
    let roll = PianoRoll::from_performance_take(&take).expect("piano roll");

    assert!(take.content_hash().starts_with("fnv1a64:"));
    assert_eq!(
        take.replay_content_hash().expect("replay hash"),
        take.content_hash()
    );
    assert_eq!(take.cassette().items().expect("cassette items").len(), 2);
    assert_eq!(roll.items.len(), 1);
    assert_eq!(roll.items[0].onset, Ratio::from_integer(0));
    assert_eq!(roll.items[0].note.duration, Ratio::new(1, 8));
    assert_eq!(roll.items[0].note.pitch.to_midi(), Some(60));
}

#[test]
fn pattern_mutator_and_lfo_recipe_descriptors_are_live() {
    let registry = default_music_component_registry();
    let mutator = registry
        .require_capability(&pattern_mutator_player_id(), MusicCapability::Renderable)
        .expect("pattern mutator renderable");
    let lfo = registry
        .require_capability(&lfo_modulator_id(), MusicCapability::Modulator)
        .expect("lfo modulator");

    let mutator_params = param_names(mutator);
    let lfo_params = param_names(lfo);

    assert!(mutator_params.iter().any(|name| name == "lock-set"));
    assert!(mutator_params.iter().any(|name| name == "scale-conform"));
    assert!(mutator_params.iter().any(|name| name == "seed"));
    assert!(lfo.has_capability(MusicCapability::Oscillator));
    assert!(lfo_params.iter().any(|name| name == "target"));
    assert!(lfo_params.iter().any(|name| name == "frequency"));
}

#[test]
fn missing_capability_failure_recipe_has_structured_error() {
    let registry = default_music_component_registry();
    let err = registry
        .require_capability(&note_echo_player_id(), MusicCapability::Oscillator)
        .expect_err("note echo is not an oscillator");

    assert!(format!("{err}").contains("missing capability oscillator"));
}

#[test]
fn golden_recipe_sources_are_registered_for_generated_docs() {
    for source in [
        include_str!("../recipes/02-golden-fixtures/captured-take-direct-record/recipe.toml"),
        include_str!("../recipes/02-golden-fixtures/pattern-mutator-locked-take/recipe.toml"),
        include_str!("../recipes/02-golden-fixtures/lfo-player-instrument-params/recipe.toml"),
        include_str!("../recipes/02-golden-fixtures/missing-capability-failure/recipe.toml"),
    ] {
        assert!(source.contains("golden") || source.contains("failure"));
        assert!(source.contains("codec = \"lisp\""));
    }
}
