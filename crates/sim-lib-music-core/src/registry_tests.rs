use sim_kernel::{Expr, Symbol};
use sim_lib_stream_core::ClockDomain;
use sim_value::access::field as card_field;

use crate::{
    MusicCapability, MusicComponentRegistry, MusicComponentRegistryEntry, MusicUnit,
    arpeggio_lab_player_descriptor, arpeggio_lab_player_id, automation_curve_modulator_id,
    bassline_player_descriptor, bassline_player_id, beat_map_player_id,
    chord_sequencer_player_descriptor, chord_sequencer_player_id, default_music_component_registry,
    drum_key_map_player_id, dual_arpeggio_player_descriptor, dual_arpeggio_player_id,
    envelope_modulator_id, euclid_player_id, lfo_modulator_id, music_browse_card_expr,
    music_browse_symbols, note_echo_player_id, oscillator_modulator_id,
    pattern_mutator_player_descriptor, pattern_mutator_player_id, polystep_player_descriptor,
    polystep_player_id, quad_note_player_descriptor, quad_note_player_id, random_walk_modulator_id,
    scales_chords_player_descriptor, scales_chords_player_id,
};

#[test]
fn registry_resolves_player_families_and_shared_instrument() {
    let registry = default_music_component_registry();
    let players = registry.by_capability(MusicCapability::Player);

    assert_eq!(players.len(), 12);
    assert!(registry.get(&scales_chords_player_id()).is_some());
    assert!(registry.get(&dual_arpeggio_player_id()).is_some());
    assert!(registry.get(&arpeggio_lab_player_id()).is_some());
    assert!(registry.get(&note_echo_player_id()).is_some());
    assert!(registry.get(&beat_map_player_id()).is_some());
    assert!(registry.get(&euclid_player_id()).is_some());
    assert!(registry.get(&drum_key_map_player_id()).is_some());
    assert!(registry.get(&chord_sequencer_player_id()).is_some());
    assert!(registry.get(&bassline_player_id()).is_some());
    assert!(registry.get(&polystep_player_id()).is_some());
    assert!(registry.get(&quad_note_player_id()).is_some());
    assert!(registry.get(&pattern_mutator_player_id()).is_some());
    assert!(
        registry
            .get(&Symbol::qualified("music/instrument", "default"))
            .is_some()
    );
    assert!(registry.get(&lfo_modulator_id()).is_some());
    assert!(registry.get(&envelope_modulator_id()).is_some());
    assert!(registry.get(&oscillator_modulator_id()).is_some());
    assert!(registry.get(&random_walk_modulator_id()).is_some());
    assert!(registry.get(&automation_curve_modulator_id()).is_some());
}

#[test]
fn arpeggio_descriptors_are_registered_as_implemented() {
    let dual = dual_arpeggio_player_descriptor().expect("dual descriptor");
    let lab = arpeggio_lab_player_descriptor().expect("lab descriptor");

    assert!(dual.implemented);
    assert!(lab.implemented);
    assert!(dual.output_families.contains(&crate::LaneKind::Control));
    assert!(lab.output_families.contains(&crate::LaneKind::Control));
    assert!(
        dual.params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "tie-rest"))
    );
    assert!(
        lab.params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "anchor-role"))
    );
}

#[test]
fn scales_chords_descriptor_is_registered_as_implemented() {
    let descriptor = scales_chords_player_descriptor().expect("descriptor");

    assert!(descriptor.implemented);
    assert!(descriptor.has_capability(MusicCapability::Player));
    assert!(descriptor.has_capability(MusicCapability::Playable));
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "scale"))
    );
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "chord-type"))
    );
}

#[test]
fn note_echo_descriptor_is_registered_as_implemented() {
    let registry = default_music_component_registry();
    let descriptor = registry
        .get(&note_echo_player_id())
        .expect("note echo descriptor")
        .descriptor();

    assert!(descriptor.implemented);
    assert!(descriptor.has_capability(MusicCapability::Player));
    assert!(descriptor.output_families.contains(&crate::LaneKind::Midi));
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "feedback-count"))
    );
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "scale-snap"))
    );
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "channel-policy"))
    );
}

#[test]
fn drum_player_descriptors_are_registered_as_implemented() {
    let registry = default_music_component_registry();
    for id in [
        beat_map_player_id(),
        euclid_player_id(),
        drum_key_map_player_id(),
    ] {
        let descriptor = registry
            .get(&id)
            .expect("drum player descriptor")
            .descriptor();
        assert!(descriptor.implemented);
        assert!(descriptor.has_capability(MusicCapability::Player));
        assert!(descriptor.output_families.contains(&crate::LaneKind::Drum));
    }

    let beat_map = registry
        .get(&beat_map_player_id())
        .expect("beat map descriptor")
        .descriptor();
    assert_eq!(beat_map.determinism, crate::DeterminismPolicy::Seeded);
    assert!(
        beat_map
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "mirror-lanes"))
    );

    let euclid = registry
        .get(&euclid_player_id())
        .expect("euclid descriptor")
        .descriptor();
    assert!(
        euclid
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "rotation"))
    );
}

#[test]
fn chord_sequencer_descriptor_is_registered_as_implemented() {
    let descriptor = chord_sequencer_player_descriptor().expect("descriptor");

    assert!(descriptor.implemented);
    assert_eq!(
        descriptor.determinism,
        crate::DeterminismPolicy::Deterministic
    );
    assert!(descriptor.has_capability(MusicCapability::Player));
    assert!(descriptor.output_families.contains(&crate::LaneKind::Trace));
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "progression-slots"))
    );
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "trigger-mode"))
    );
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "suggestion-count"))
    );
}

#[test]
fn bassline_descriptor_is_registered_as_implemented() {
    let descriptor = bassline_player_descriptor().expect("descriptor");

    assert!(descriptor.implemented);
    assert_eq!(descriptor.determinism, crate::DeterminismPolicy::Seeded);
    assert!(descriptor.has_capability(MusicCapability::Player));
    assert!(descriptor.output_families.contains(&crate::LaneKind::Trace));
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "chord-follow"))
    );
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "ghost-notes"))
    );
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "seed"))
    );
}

#[test]
fn polystep_descriptor_is_registered_as_implemented() {
    let descriptor = polystep_player_descriptor().expect("descriptor");

    assert!(descriptor.implemented);
    assert_eq!(descriptor.determinism, crate::DeterminismPolicy::Seeded);
    assert!(descriptor.has_capability(MusicCapability::Player));
    assert!(descriptor.output_families.contains(&crate::LaneKind::Trace));
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "probability"))
    );
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "step-record"))
    );
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "target-routing"))
    );
}

#[test]
fn quad_note_descriptor_is_registered_as_implemented() {
    let descriptor = quad_note_player_descriptor().expect("descriptor");

    assert!(descriptor.implemented);
    assert_eq!(descriptor.determinism, crate::DeterminismPolicy::Seeded);
    assert!(descriptor.has_capability(MusicCapability::Player));
    assert!(descriptor.output_families.contains(&crate::LaneKind::Trace));
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "stream-count"))
    );
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "harmonic-relation"))
    );
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "master-seed"))
    );
}

#[test]
fn pattern_mutator_descriptor_is_registered_as_implemented() {
    let descriptor = pattern_mutator_player_descriptor().expect("descriptor");

    assert!(descriptor.implemented);
    assert_eq!(descriptor.determinism, crate::DeterminismPolicy::Seeded);
    assert!(descriptor.has_capability(MusicCapability::Player));
    assert!(descriptor.output_families.contains(&crate::LaneKind::Trace));
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "mutation-ops"))
    );
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "amount"))
    );
    assert!(
        descriptor
            .params
            .iter()
            .any(|param| param.id == Symbol::qualified("music/player-param", "lock-set"))
    );
}

#[test]
fn modulator_descriptors_are_registered_as_playable_controls() {
    let registry = default_music_component_registry();
    for id in [
        lfo_modulator_id(),
        envelope_modulator_id(),
        oscillator_modulator_id(),
        random_walk_modulator_id(),
        automation_curve_modulator_id(),
    ] {
        let descriptor = registry
            .get(&id)
            .expect("modulator descriptor")
            .descriptor();
        assert!(descriptor.implemented);
        assert!(descriptor.has_capability(MusicCapability::Modulator));
        assert!(descriptor.has_capability(MusicCapability::Playable));
        assert!(descriptor.has_capability(MusicCapability::Renderable));
        assert_eq!(descriptor.ports.len(), 1);
        assert!(
            descriptor
                .output_families
                .contains(&crate::LaneKind::Control)
        );
        assert!(
            descriptor
                .params
                .iter()
                .any(|param| param.id == Symbol::qualified("music/player-param", "target"))
        );
    }

    let lfo = registry
        .get(&lfo_modulator_id())
        .expect("lfo descriptor")
        .descriptor();
    assert!(lfo.has_capability(MusicCapability::Oscillator));
    assert_eq!(lfo.rate.clock_domain(), ClockDomain::Control);

    let oscillator = registry
        .get(&oscillator_modulator_id())
        .expect("oscillator descriptor")
        .descriptor();
    assert!(oscillator.has_capability(MusicCapability::Oscillator));
    assert_eq!(oscillator.rate.clock_domain(), ClockDomain::Sample);

    let random_walk = registry
        .get(&random_walk_modulator_id())
        .expect("random walk descriptor")
        .descriptor();
    assert_eq!(random_walk.determinism, crate::DeterminismPolicy::Seeded);
}

#[test]
fn descriptors_cover_required_capability_records() {
    let registry = default_music_component_registry();
    for capability in [
        MusicCapability::Playable,
        MusicCapability::Player,
        MusicCapability::Modulator,
        MusicCapability::Oscillator,
        MusicCapability::PerformanceSource,
        MusicCapability::Renderable,
    ] {
        assert!(
            !registry.by_capability(capability).is_empty(),
            "missing {capability:?}"
        );
    }
}

#[test]
fn descriptor_expression_is_stable_and_complete() {
    let descriptor = dual_arpeggio_player_descriptor().expect("descriptor");
    let expr = descriptor.to_expr();

    assert!(descriptor.has_capability(MusicCapability::Player));
    let rate = descriptor
        .params
        .iter()
        .find(|param| param.id == Symbol::qualified("music/player-param", "rate"))
        .expect("rate param");
    let octaves = descriptor
        .params
        .iter()
        .find(|param| param.id == Symbol::qualified("music/player-param", "octaves"))
        .expect("octaves param");
    assert_eq!(rate.unit, MusicUnit::Beats);
    assert_eq!(octaves.unit, MusicUnit::None);
    assert_eq!(descriptor.ports.len(), 2);
    assert!(!descriptor.accepted_event_families.is_empty());
    assert!(!descriptor.output_families.is_empty());

    let Expr::Map(entries) = expr else {
        panic!("descriptor serializes as a map");
    };
    assert_eq!(entries[0].0, field("id"));
    assert_eq!(entries[1].0, field("label"));
    assert_eq!(entries[2].0, field("category"));
    assert_eq!(entries[3].0, field("capabilities"));
}

#[test]
fn browse_cards_are_generated_for_registered_components() {
    let registry = default_music_component_registry();
    let symbols = music_browse_symbols(&registry);

    assert!(symbols.contains(&scales_chords_player_id()));
    assert!(symbols.contains(&dual_arpeggio_player_id()));
    assert!(symbols.contains(&arpeggio_lab_player_id()));
    assert!(symbols.contains(&beat_map_player_id()));
    assert!(symbols.contains(&euclid_player_id()));
    assert!(symbols.contains(&drum_key_map_player_id()));
    assert!(symbols.contains(&bassline_player_id()));
    assert!(symbols.contains(&polystep_player_id()));
    assert!(symbols.contains(&quad_note_player_id()));
    assert!(symbols.contains(&pattern_mutator_player_id()));
    assert!(symbols.contains(&lfo_modulator_id()));
    assert!(symbols.contains(&envelope_modulator_id()));
    assert!(symbols.contains(&oscillator_modulator_id()));
    assert!(symbols.contains(&random_walk_modulator_id()));
    assert!(symbols.contains(&automation_curve_modulator_id()));
    for symbol in symbols {
        let card = music_browse_card_expr(&registry, &symbol).expect("browse card");
        assert_eq!(card_field(&card, "subject"), Some(&Expr::Symbol(symbol)));
        assert_eq!(card_field(&card, "shape-known"), Some(&Expr::Bool(true)));
        assert!(matches!(card_field(&card, "help"), Some(Expr::Map(_))));
        assert!(matches!(card_field(&card, "args"), Some(Expr::Map(_))));
        assert!(matches!(card_field(&card, "tests"), Some(Expr::List(items)) if !items.is_empty()));
    }
}

#[test]
fn registry_reports_missing_capability_errors() {
    let registry = default_music_component_registry();
    let err = registry
        .require_capability(&note_echo_player_id(), MusicCapability::Oscillator)
        .expect_err("note echo is not an oscillator");

    assert!(format!("{err}").contains("missing capability oscillator"));
}

#[test]
fn registry_rejects_duplicate_ids() {
    let mut registry = MusicComponentRegistry::new();
    let entry =
        MusicComponentRegistryEntry::new(dual_arpeggio_player_descriptor().expect("descriptor"));
    registry
        .register(entry.clone())
        .expect("first registration");
    let err = registry
        .register(entry)
        .expect_err("duplicate should be rejected");

    assert!(format!("{err}").contains("duplicate music component registry id"));
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym("music/component-descriptor", name)
}
