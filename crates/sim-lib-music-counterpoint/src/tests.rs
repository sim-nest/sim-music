use std::sync::Arc;

// conformance: counterpoint reports and stretto graphs retain exact rule and relation evidence.

use num_rational::Ratio;
use sim_codec::{Input, decode_with_codec};
use sim_codec_lisp::LispCodecLib;
use sim_kernel::{DefaultFactory, EagerPolicy, Expr, QuoteMode, RawArgs, ReadPolicy, Symbol};
use sim_lib_music_core::{
    Articulation, Channel, Counterpoint, Melody, MelodyItem, Note, Pitch, Time,
};
use sim_lib_music_shapes::{encode_counterpoint, encode_melody};

use crate::{
    ContrapuntalForm, RuleSet, StrettoCluster, StrettoEntry, StrettoPolicy, StrettoTransform,
    analyze_counterpoint, cluster_overlap, fuse_stretto_entries, install_music_counterpoint_lib,
    materialize_transform, music_counterpoint_analyze_symbol, music_stretto_graph_symbol,
    stretto_graph,
};

fn time(numerator: i64, denominator: i64) -> Time {
    Ratio::new(numerator, denominator)
}

fn note(midi: u8, duration: Time) -> MelodyItem {
    MelodyItem::Note(
        Note::new(
            duration,
            Pitch::from_midi(midi),
            100,
            Channel::new(0).expect("channel"),
            Articulation::Normal,
        )
        .expect("note"),
    )
}

fn melody(midis: &[u8], duration: Time) -> Melody {
    Melody::new(
        midis
            .iter()
            .copied()
            .map(|midi| note(midi, duration))
            .collect(),
    )
    .expect("melody")
}

#[test]
fn species_rules_are_inspectable_and_validate() {
    let first = RuleSet::species_one(time(1, 1));
    let fourth = RuleSet::species_four(time(1, 1));
    assert_eq!(first.durations.allowed_pulse_ratios, vec![time(1, 1)]);
    assert!(first.dissonance.allowed_contexts.is_empty());
    assert!(
        fourth
            .dissonance
            .allowed_contexts
            .contains(&crate::DissonanceContext::Suspension)
    );
    first.validate().expect("first species");
    fourth.validate().expect("fourth species");
}

#[test]
fn every_violation_names_voices_notes_time_rule_and_metric() {
    let cp = Counterpoint::new(
        vec![melody(&[73], time(1, 1)), melody(&[60], time(1, 1))],
        vec!["Upper".to_owned(), "Lower".to_owned()],
    )
    .expect("counterpoint");
    let report = analyze_counterpoint(&cp, &RuleSet::species_one(time(1, 1)));
    let dissonance = report
        .violations
        .iter()
        .find(|violation| violation.rule == "dissonance-preparation-resolution")
        .expect("dissonance violation");
    assert_eq!(
        (dissonance.span.start, dissonance.span.end),
        (time(0, 1), time(1, 1))
    );
    assert_eq!(
        dissonance
            .voices
            .iter()
            .map(|voice| voice.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Upper", "Lower"]
    );
    assert_eq!(dissonance.notes.len(), 2);
    assert!(
        dissonance
            .notes
            .iter()
            .all(|note| !note.note_id.as_str().is_empty() && !note.event_id.as_str().is_empty())
    );
    assert_eq!(dissonance.metric.metric, "harmonic-interval-class");
    assert_eq!(dissonance.metric.observed, "1");
    assert_eq!(report.provenance.mode, "existing-counterpoint");
}

#[test]
fn exact_motion_reports_parallel_fifths() {
    let cp = Counterpoint::new(
        vec![melody(&[72, 74], time(1, 1)), melody(&[65, 67], time(1, 1))],
        vec!["Upper".to_owned(), "Lower".to_owned()],
    )
    .expect("counterpoint");
    let report = analyze_counterpoint(&cp, &RuleSet::species_one(time(1, 1)));
    let parallel = report
        .violations
        .iter()
        .find(|violation| violation.rule == "parallel-perfect")
        .expect("parallel perfect");
    assert_eq!(parallel.metric.observed, "5->5");
    assert_eq!(parallel.notes.len(), 4);
    assert_eq!(report.motions.len(), 1);
}

#[test]
fn weak_stepwise_passing_dissonance_is_admitted_by_species_two() {
    let upper = melody(&[72, 74, 76], time(1, 2));
    let lower = Melody::new(vec![
        note(60, time(1, 1)),
        MelodyItem::Rest(sim_lib_music_core::Rest::new(time(1, 2)).expect("rest")),
    ])
    .expect("lower");
    let cp = Counterpoint::new(
        vec![upper, lower],
        vec!["Upper".to_owned(), "Lower".to_owned()],
    )
    .expect("counterpoint");
    let report = analyze_counterpoint(&cp, &RuleSet::species_two(time(1, 1)));
    assert!(
        report
            .violations
            .iter()
            .all(|violation| violation.rule != "dissonance-preparation-resolution"),
        "{:?}",
        report.violations
    );
}

#[test]
fn stretto_fixture_preserves_couples_and_clique_relations() {
    let subject = melody(&[60, 60, 60, 60], time(1, 4));
    let policy = StrettoPolicy {
        delays: vec![time(1, 4)],
        transforms: vec![StrettoTransform::original(4), StrettoTransform::original(7)],
        max_entries: 3,
        minimum_cluster_voices: 3,
        ..StrettoPolicy::default()
    };
    let graph = stretto_graph(&subject, policy).expect("stretto graph");
    assert_eq!(graph.compatibility.node_count(), 3);
    assert_eq!(graph.couples.len(), 3);
    assert_eq!(graph.clusters.len(), 1);
    assert_eq!(graph.clusters[0].entries, vec![0, 1, 2]);
    assert_eq!(graph.clusters[0].edge_ids.len(), 3);
    assert_eq!(
        graph.clusters[0].fusion.mode,
        "derived-analysis-not-generation"
    );
    assert!(
        graph
            .provenance
            .iter()
            .any(|fact| fact == "generation=false")
    );
}

#[test]
fn catalog_chain_fixture_uses_largest_normalized_overlap() {
    let source = melody(&[60], time(1, 4));
    let specifications = [
        (0, 0, 0),
        (1, 1, 4),
        (2, 2, 7),
        (3, 0, 0),
        (4, 1, 3),
        (5, 2, 8),
    ];
    let entries = specifications
        .into_iter()
        .map(|(id, quarter_delay, transposition)| StrettoEntry {
            id,
            delay: time(quarter_delay, 4),
            transform: StrettoTransform::original(transposition),
            melody: source.clone(),
        })
        .collect::<Vec<_>>();
    let left_entries = entries[..3].to_vec();
    let right_entries = entries[3..].to_vec();
    let left = StrettoCluster {
        entries: vec![0, 1, 2],
        edge_ids: Vec::new(),
        fusion: fuse_stretto_entries(&left_entries).expect("left fusion"),
    };
    let right = StrettoCluster {
        entries: vec![3, 4, 5],
        edge_ids: Vec::new(),
        fusion: fuse_stretto_entries(&right_entries).expect("right fusion"),
    };
    assert_eq!(cluster_overlap(&left, &right, &entries), 2);
}

#[test]
fn transform_candidates_delegate_retrograde_pitch_and_time_changes() {
    let subject = melody(&[60, 62, 64], time(1, 4));
    let transformed = materialize_transform(
        &subject,
        &StrettoTransform {
            form: ContrapuntalForm::Retrograde,
            transposition: 12,
            duration_factor: time(2, 1),
        },
    )
    .expect("transform");
    let notes = transformed
        .items
        .iter()
        .filter_map(|item| match item {
            MelodyItem::Note(note) => Some((note.pitch.semitone(), note.duration)),
            MelodyItem::Rest(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        notes,
        vec![(76, time(1, 2)), (74, time(1, 2)), (72, time(1, 2))]
    );
}

#[test]
fn lisp_surfaces_expose_violation_evidence_and_stretto_relations() {
    let mut cx = sim_kernel::Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_music_counterpoint_lib(&mut cx).expect("install");

    let cp = Counterpoint::new(
        vec![melody(&[73], time(1, 1)), melody(&[60], time(1, 1))],
        vec!["Upper".to_owned(), "Lower".to_owned()],
    )
    .expect("counterpoint");
    let function = cx
        .resolve_function(&music_counterpoint_analyze_symbol())
        .expect("analysis function");
    let callable = function.object().as_callable().expect("callable");
    let value = callable
        .call_exprs(
            &mut cx,
            RawArgs::new(vec![
                Expr::Symbol(Symbol::new(":counterpoint")),
                Expr::String(encode_counterpoint(&cp)),
                Expr::Symbol(Symbol::new(":rules")),
                quoted_symbol("species-one"),
            ]),
        )
        .expect("analysis call");
    let Expr::Map(report) = value.object().as_expr(&mut cx).expect("report expr") else {
        panic!("expected analysis map");
    };
    assert_eq!(
        field(&report, "mode"),
        Some(&Expr::String("existing-counterpoint".to_owned()))
    );
    let Some(Expr::Vector(violations)) = field(&report, "violations") else {
        panic!("expected violation vector");
    };
    let Expr::Map(violation) = &violations[0] else {
        panic!("expected violation map");
    };
    for key in ["voices", "notes", "span", "rule", "metric"] {
        assert!(field(violation, key).is_some(), "missing {key}");
    }

    let subject = melody(&[60, 60, 60, 60], time(1, 4));
    let function = cx
        .resolve_function(&music_stretto_graph_symbol())
        .expect("stretto function");
    let callable = function.object().as_callable().expect("callable");
    let value = callable
        .call_exprs(
            &mut cx,
            RawArgs::new(vec![
                Expr::Symbol(Symbol::new(":subject")),
                Expr::String(encode_melody(&subject)),
                Expr::Symbol(Symbol::new(":policy")),
                Expr::Map(vec![
                    (
                        Expr::Symbol(Symbol::new(":delays")),
                        Expr::Vector(vec![Expr::String("1/4".to_owned())]),
                    ),
                    (
                        Expr::Symbol(Symbol::new(":transpositions")),
                        Expr::Vector(vec![
                            Expr::String("4".to_owned()),
                            Expr::String("7".to_owned()),
                        ]),
                    ),
                    (
                        Expr::Symbol(Symbol::new(":minimum-cluster-voices")),
                        Expr::String("3".to_owned()),
                    ),
                    (
                        Expr::Symbol(Symbol::new(":max-entries")),
                        Expr::String("3".to_owned()),
                    ),
                ]),
            ]),
        )
        .expect("stretto call");
    let Expr::Map(graph) = value.object().as_expr(&mut cx).expect("graph expr") else {
        panic!("expected graph map");
    };
    assert_eq!(field(&graph, "generation"), Some(&Expr::Bool(false)));
    for key in ["entries", "couples", "components", "cliques", "chains"] {
        assert!(field(&graph, key).is_some(), "missing {key}");
    }
}

#[test]
fn checked_lisp_specimens_execute_and_reproduce_evidence() {
    let mut cx = sim_kernel::Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::register_core_classes(&mut cx);
    sim_test_support::register_f64_number_domain(&mut cx);
    let lisp = LispCodecLib::new(cx.registry_mut().fresh_codec_id()).expect("lisp codec");
    cx.load_lib(&lisp).expect("install lisp codec");
    install_music_counterpoint_lib(&mut cx).expect("install counterpoint lib");

    let report = evaluate_lisp_specimen(
        &mut cx,
        include_str!("../recipes/01-basics/counterpoint-report/setup.siml"),
    );
    let Expr::Map(report) = report else {
        panic!("counterpoint specimen must return a report map: {report:?}");
    };
    let Some(Expr::Vector(violations)) = field(&report, "violations") else {
        panic!("counterpoint specimen must return violation evidence");
    };
    assert!(!violations.is_empty());
    for violation in violations {
        let Expr::Map(violation) = violation else {
            panic!("violation evidence must be a map");
        };
        for key in ["voices", "notes", "span", "rule", "metric"] {
            assert!(field(violation, key).is_some(), "missing {key}");
        }
    }

    let graph = evaluate_lisp_specimen(
        &mut cx,
        include_str!("../recipes/01-basics/stretto-graph/setup.siml"),
    );
    let Expr::Map(graph) = graph else {
        panic!("stretto specimen must return a graph map: {graph:?}");
    };
    assert_eq!(field(&graph, "generation"), Some(&Expr::Bool(false)));
    let Some(Expr::Vector(couples)) = field(&graph, "couples") else {
        panic!("stretto specimen must return couple relations");
    };
    assert!(!couples.is_empty());
    for key in ["components", "cliques"] {
        let Some(Expr::Vector(relations)) = field(&graph, key) else {
            panic!("stretto specimen must return {key} relations");
        };
        assert!(!relations.is_empty(), "stretto specimen has no {key}");
    }
    assert!(matches!(field(&graph, "chains"), Some(Expr::Vector(_))));
}

fn evaluate_lisp_specimen(cx: &mut sim_kernel::Cx, source: &str) -> Expr {
    let form = decode_with_codec(
        cx,
        &Symbol::qualified("codec", "lisp"),
        Input::Text(source.trim().to_owned()),
        ReadPolicy::default(),
    )
    .expect("decode Lisp specimen");
    let value = cx
        .eval_expr(lower_lisp_eval_surface(form))
        .expect("evaluate Lisp specimen");
    value
        .object()
        .as_expr(cx)
        .expect("Lisp specimen result expression")
}

fn lower_lisp_eval_surface(expr: Expr) -> Expr {
    match expr {
        Expr::List(items) if items.len() > 1 => {
            let mut items = items
                .into_iter()
                .map(lower_lisp_eval_surface)
                .collect::<Vec<_>>();
            Expr::Call {
                operator: Box::new(items.remove(0)),
                args: items,
            }
        }
        Expr::List(items) => Expr::List(items.into_iter().map(lower_lisp_eval_surface).collect()),
        Expr::Vector(items) => {
            Expr::Vector(items.into_iter().map(lower_lisp_eval_surface).collect())
        }
        Expr::Map(entries) => Expr::Map(
            entries
                .into_iter()
                .map(|(key, value)| (lower_lisp_eval_surface(key), lower_lisp_eval_surface(value)))
                .collect(),
        ),
        Expr::Set(items) => Expr::Set(items.into_iter().map(lower_lisp_eval_surface).collect()),
        Expr::Block(items) => Expr::Block(items.into_iter().map(lower_lisp_eval_surface).collect()),
        Expr::Annotated { expr, annotations } => Expr::Annotated {
            expr: Box::new(lower_lisp_eval_surface(*expr)),
            annotations: annotations
                .into_iter()
                .map(|(name, value)| (name, lower_lisp_eval_surface(value)))
                .collect(),
        },
        Expr::Extension { tag, payload } => Expr::Extension {
            tag,
            payload: Box::new(lower_lisp_eval_surface(*payload)),
        },
        other => other,
    }
}

fn quoted_symbol(value: &str) -> Expr {
    Expr::Quote {
        mode: QuoteMode::Quote,
        expr: Box::new(Expr::Symbol(Symbol::new(value))),
    }
}

fn field<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    entries.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol) if symbol.name.as_ref() == name => Some(value),
        _ => None,
    })
}
