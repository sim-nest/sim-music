use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol};
use sim_lib_audio_graph_core::{Patch, PatchNode};
use sim_lib_music_core::{
    Arranger, ArrangerPlacement, Articulation, Channel, Music, Note, PlayableRef,
};
use sim_lib_plugin_core::{PluginFormat, PluginId, PluginState};

use crate::{
    COMPONENT_BUILDER_PATCH_FORMAT, ClipSource, DawClip, DawInstrumentKind, DawSession,
    DawSessionDescriptor, DawSessionRouteKind, DawTrack, DawTrackKind, PluginChain, PluginSlot,
    browse_session_graph, daw_prelude_operations, daw_session_topology_package,
    install_daw_session_lib, instrument_session_fixture, instrument_session_fixture_names,
    instrument_session_render_smoke_command, session_help_card_expr,
};

#[test]
fn session_can_be_created_saved_loaded_and_rendered_offline() {
    let session = fixture_session();
    let saved = session.save_expr();
    let loaded = DawSession::load_expr(&saved).expect("load session");
    let rendered = loaded.render_offline(4).expect("offline render");

    assert_eq!(loaded.name(), "Song");
    assert_eq!(rendered.tracks_rendered(), 1);
    assert_eq!(rendered.clips_rendered(), 1);
    assert_eq!(rendered.buffer().frames(), 4);
    assert_eq!(
        rendered.buffer().samples_f32(),
        &[0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]
    );
}

#[test]
fn plugin_chain_state_and_patch_metadata_roundtrip() {
    let mut state = PluginState::new();
    state.set_param(1, 0.75);
    let slot = PluginSlot::new(
        "gain-slot",
        PluginId::new(PluginFormat::Sim, "sim.gain").unwrap(),
        state,
    )
    .unwrap()
    .bypassed(true);
    let track = DawTrack::audio("track-with-plugin", "Track With Plugin", 2)
        .unwrap()
        .with_plugin_chain(PluginChain::default().with_slot(slot));
    let mut session = DawSession::new("plugin-song", "Plugin Song", 48_000)
        .unwrap()
        .with_patch(Patch {
            nodes: vec![PatchNode {
                id: "gain".to_owned(),
                in_channels: 2,
                out_channels: 2,
            }],
            cables: Vec::new(),
        });
    session.add_track(track).unwrap();

    let loaded = DawSession::from_expr(&session.to_expr()).unwrap();
    let loaded_slot = &loaded.tracks()[0].plugin_chain().slots()[0];

    assert_eq!(loaded.patch().nodes[0].id, "gain");
    assert_eq!(loaded_slot.plugin().stable_id, "sim.gain");
    assert_eq!(loaded_slot.state().param(1), Some(0.75));
    assert!(loaded_slot.is_bypassed());
}

#[test]
fn component_builder_patch_records_stable_ids_save_load_and_preview_hooks() {
    let mut session = DawSession::new("builder-song", "Builder Song", 48_000)
        .unwrap()
        .with_patch(Patch {
            nodes: vec![
                PatchNode {
                    id: "osc".to_owned(),
                    in_channels: 1,
                    out_channels: 2,
                },
                PatchNode {
                    id: "amp".to_owned(),
                    in_channels: 2,
                    out_channels: 2,
                },
            ],
            cables: Vec::new(),
        });
    let track = DawTrack::audio("preview-track", "Preview Track", 2)
        .unwrap()
        .with_clip(DawClip::new("preview", 0, 4, ClipSource::Constant(0.25), 1.0).unwrap());
    session.add_track(track).unwrap();

    let builder = session.component_builder_patch_expr();
    assert_eq!(
        builder_field_value(&builder, "format"),
        Some(&Expr::String(COMPONENT_BUILDER_PATCH_FORMAT.to_owned()))
    );
    assert_eq!(
        builder_field_value(&builder, "stable-component-ids"),
        Some(&Expr::Vector(vec![
            Expr::String("osc".to_owned()),
            Expr::String("amp".to_owned()),
        ]))
    );
    assert_eq!(
        builder_field_value(builder_field_value(&builder, "hooks").unwrap(), "save"),
        Some(&action_symbol("save"))
    );
    assert_eq!(
        builder_field_value(builder_field_value(&builder, "hooks").unwrap(), "load"),
        Some(&action_symbol("load"))
    );
    assert_eq!(
        builder_field_value(
            builder_field_value(&builder, "hooks").unwrap(),
            "live-preview"
        ),
        Some(&action_symbol("live-preview"))
    );

    let saved = builder_field_value(&builder, "saved-session").expect("saved session");
    let loaded = DawSession::load_expr(saved).expect("load saved builder session");
    assert_eq!(loaded.patch().nodes[0].id, "osc");
    assert_eq!(loaded.patch().nodes[1].id, "amp");

    let preview = session
        .component_builder_preview_expr(4)
        .expect("builder preview");
    assert_eq!(
        builder_field_value(&preview, "hook"),
        Some(&action_symbol("live-preview"))
    );
    assert_eq!(builder_number(&preview, "tracks-rendered"), Some("1"));
    assert_eq!(builder_number(&preview, "clips-rendered"), Some("1"));
}

#[test]
fn arranger_clip_and_track_round_trip_without_audio_preview() {
    let arranger = Arranger::new(
        vec![
            ArrangerPlacement::new(
                "note",
                PlayableRef::inline(Music::Note(
                    Note::new(
                        sim_lib_music_core::Time::new(1, 4),
                        sim_lib_music_core::Pitch::from_midi(60),
                        100,
                        Channel::new(0).unwrap(),
                        Articulation::Normal,
                    )
                    .unwrap(),
                )),
                sim_lib_music_core::Time::new(0, 1),
            )
            .unwrap(),
        ],
        vec![sim_lib_music_core::LaneId::new("notes")],
    )
    .unwrap();
    let track = DawTrack::new(
        "arranger-track",
        "Arranger Track",
        DawTrackKind::Arranger,
        2,
    )
    .unwrap()
    .with_clip(DawClip::arranger("arranger-clip", 0, 480, arranger.clone()).unwrap());
    let mut session = DawSession::new("arranger-song", "Arranger Song", 48_000).unwrap();
    session.add_track(track).unwrap();

    let loaded = DawSession::load_expr(&session.save_expr()).unwrap();
    assert_eq!(loaded, session);
    let ClipSource::Arranger(loaded_arranger) = loaded.tracks()[0].clips()[0].source() else {
        panic!("arranger source");
    };
    assert_eq!(loaded_arranger, &arranger);
    let render = loaded.render_offline(4).unwrap();
    assert_eq!(render.tracks_rendered(), 0);
    assert_eq!(render.clips_rendered(), 0);
}

#[test]
fn instrument_session_load_render_reopen_smoke() {
    let session = instrument_session_fixture();
    let saved = session.save_expr();
    let loaded = DawSession::load_expr(&saved).expect("load instrument session");
    let render = loaded.render_offline(4).expect("render preview");
    let reopened = DawSession::load_expr(&loaded.save_expr()).expect("reopen session");
    let builder = loaded.component_builder_patch_expr();
    let preview = loaded.component_builder_preview_expr(4).unwrap();
    let package = daw_session_topology_package(&loaded);

    assert_eq!(loaded, reopened);
    assert_eq!(
        instrument_session_fixture_names(),
        ["instrument-session-default", "generic-synth-graph-session",]
    );
    assert_eq!(
        instrument_session_render_smoke_command(),
        "cargo test -p sim-lib-daw-session instrument_session_load_render_reopen_smoke"
    );
    assert_eq!(loaded.instrument_instances().len(), 5);
    assert!(
        loaded
            .instrument_instances()
            .iter()
            .any(|instrument| instrument.kind() == DawInstrumentKind::Dx7
                && instrument.graph_node_id() == "dx7-voice")
    );
    for kind in [
        DawSessionRouteKind::Midi,
        DawSessionRouteKind::ParameterAutomation,
        DawSessionRouteKind::PatchEdit,
        DawSessionRouteKind::Trace,
        DawSessionRouteKind::Preview,
    ] {
        assert!(
            loaded.routes().iter().any(|route| route.kind() == kind),
            "missing route kind {}",
            kind.as_str()
        );
    }
    assert_eq!(render.tracks_rendered(), 1);
    assert_eq!(render.clips_rendered(), 1);
    assert_eq!(render.buffer().frames(), 4);
    assert_eq!(
        metadata_number(&package.metadata, "instrument-count"),
        Some("5")
    );
    assert_eq!(metadata_number(&package.metadata, "route-count"), Some("5"));
    assert!(builder_list_contains(
        &builder,
        "instrument-instances",
        "dx7:dx7-voice"
    ));
    assert!(builder_list_contains(&builder, "routes", "midi:dx7-voice"));
    assert!(builder_list_contains(&preview, "routes", "preview"));
}

#[test]
fn citizen_daw_session_descriptor_round_trips_and_fails_closed() {
    let session = fixture_session();
    let descriptor = DawSessionDescriptor::new(session.clone());

    assert_eq!(descriptor.session().unwrap(), session);

    let mut expr = descriptor.as_expr().clone();
    let Expr::Map(entries) = &mut expr else {
        panic!("DAW session descriptor should be a map");
    };
    for (key, value) in entries {
        if key == &Expr::Symbol(Symbol::qualified("daw-session", "sample-rate-hz")) {
            *value = Expr::String("0".to_owned());
        }
    }
    let err = DawSessionDescriptor::from_expr(expr).unwrap_err();
    assert!(format!("{err}").contains("sample rate"));
}

#[test]
fn browse_graph_exposes_session_tracks_buses_and_help() {
    let session = fixture_session();
    let cards = browse_session_graph(&session);
    let help = session_help_card_expr();

    assert!(cards.iter().any(|card| card_has_subject(card, "song")));
    assert!(cards.iter().any(|card| card_has_subject(card, "track-1")));
    assert!(cards.iter().any(|card| card_has_subject(card, "master")));
    assert!(format!("{help:?}").contains("render-offline"));
}

#[test]
fn daw_prelude_lists_lisp_facing_operations() {
    let operations = daw_prelude_operations();

    assert!(operations.contains(&Symbol::qualified("daw", "session")));
    assert!(operations.contains(&Symbol::qualified("daw", "render-offline")));
    assert!(operations.contains(&Symbol::qualified("daw", "topology-package")));
}

#[test]
fn topology_package_can_launch_from_session_data() {
    let session = fixture_session();
    let package = daw_session_topology_package(&session);

    assert_eq!(package.name(), &Symbol::new("daw-song"));
    assert_eq!(package.graph.nodes.len(), 4);
    assert_eq!(package.graph.edges.len(), 3);
    assert_eq!(
        package.metadata[0].0,
        Symbol::qualified("daw-session", "session-id")
    );
}

#[test]
fn install_daw_session_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_daw_session_lib(&mut cx).expect("install");
    install_daw_session_lib(&mut cx).expect("idempotent install");

    assert!(
        cx.registry()
            .lib(&Symbol::new("daw-session"))
            .expect("registered")
            .manifest
            .exports
            .iter()
            .any(|export| *export.symbol() == Symbol::qualified("daw-session", "DawSession"))
    );
}

fn fixture_session() -> DawSession {
    let mut session = DawSession::new("song", "Song", 48_000).unwrap();
    let track = DawTrack::audio("track-1", "Track 1", 2)
        .unwrap()
        .with_clip(DawClip::new("clip-1", 0, 4, ClipSource::Constant(0.5), 1.0).unwrap());
    session.add_track(track).unwrap();
    session
}

fn card_has_subject(card: &Expr, subject: &str) -> bool {
    let Expr::Map(entries) = card else {
        return false;
    };
    entries.iter().any(|(key, value)| {
        key == &Expr::Symbol(Symbol::new("subject")) && value == &Expr::Symbol(Symbol::new(subject))
    })
}

fn builder_field(name: &'static str) -> Expr {
    sim_value::build::qsym("daw-session/component-builder", name)
}

fn builder_field_value<'a>(expr: &'a Expr, name: &'static str) -> Option<&'a Expr> {
    let Expr::Map(entries) = expr else {
        return None;
    };
    entries
        .iter()
        .find_map(|(key, value)| (key == &builder_field(name)).then_some(value))
}

fn builder_number<'a>(expr: &'a Expr, name: &'static str) -> Option<&'a str> {
    let Expr::Number(number) = builder_field_value(expr, name)? else {
        return None;
    };
    Some(&number.canonical)
}

fn builder_list_contains(expr: &Expr, name: &'static str, expected: &str) -> bool {
    matches!(
        builder_field_value(expr, name),
        Some(Expr::Vector(items))
            if items
                .iter()
                .any(|item| matches!(item, Expr::String(text) if text == expected))
    )
}

fn metadata_number<'a>(metadata: &'a [(Symbol, Expr)], name: &str) -> Option<&'a str> {
    metadata.iter().find_map(|(key, value)| {
        let Expr::Number(number) = value else {
            return None;
        };
        (key.namespace.as_deref() == Some("daw-session") && key.name.as_ref() == name)
            .then_some(number.canonical.as_str())
    })
}

fn action_symbol(name: &'static str) -> Expr {
    Expr::Symbol(Symbol::qualified("component-builder/action", name))
}
