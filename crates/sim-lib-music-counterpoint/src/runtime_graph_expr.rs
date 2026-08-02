use sim_kernel::Expr;

use crate::runtime::{integer, map, span_expr, strings, symbol, time_expr, violation_expr};
use crate::{ContrapuntalForm, StrettoGraph};

pub(crate) fn stretto_graph_expr(graph: &StrettoGraph) -> Expr {
    map(vec![
        ("mode", symbol("analysis")),
        ("generation", Expr::Bool(false)),
        ("provenance", strings(&graph.provenance)),
        (
            "entries",
            Expr::Vector(
                graph
                    .compatibility
                    .nodes
                    .iter()
                    .map(|entry| {
                        map(vec![
                            ("id", integer(entry.id)),
                            ("delay", time_expr(entry.delay)),
                            ("form", symbol(&form_name(entry.transform.form))),
                            ("transposition", integer(entry.transform.transposition)),
                            (
                                "duration-factor",
                                time_expr(entry.transform.duration_factor),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "couples",
            Expr::Vector(
                graph
                    .couples
                    .iter()
                    .map(|couple| {
                        map(vec![
                            ("leader", integer(couple.leader)),
                            ("follower", integer(couple.follower)),
                            ("span", span_expr(&couple.compatibility.overlap.span)),
                            (
                                "simultaneous-windows",
                                integer(couple.compatibility.overlap.simultaneous_windows),
                            ),
                            (
                                "interval-classes",
                                Expr::Vector(
                                    couple
                                        .compatibility
                                        .overlap
                                        .interval_classes
                                        .iter()
                                        .map(|(class, count)| {
                                            Expr::Vector(vec![integer(*class), integer(*count)])
                                        })
                                        .collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "components",
            Expr::Vector(
                graph
                    .components
                    .iter()
                    .map(|component| Expr::Vector(component.iter().copied().map(integer).collect()))
                    .collect(),
            ),
        ),
        (
            "cliques",
            Expr::Vector(
                graph
                    .clusters
                    .iter()
                    .map(|cluster| {
                        map(vec![
                            (
                                "entries",
                                Expr::Vector(
                                    cluster.entries.iter().copied().map(integer).collect(),
                                ),
                            ),
                            (
                                "edge-ids",
                                Expr::Vector(
                                    cluster.edge_ids.iter().copied().map(integer).collect(),
                                ),
                            ),
                            ("fusion-mode", Expr::String(cluster.fusion.mode.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "chains",
            Expr::Vector(
                graph
                    .chains
                    .iter()
                    .map(|chain| {
                        map(vec![
                            (
                                "clusters",
                                Expr::Vector(chain.clusters.iter().copied().map(integer).collect()),
                            ),
                            (
                                "overlaps",
                                Expr::Vector(chain.overlaps.iter().copied().map(integer).collect()),
                            ),
                            (
                                "fused-entries",
                                Expr::Vector(
                                    chain.fused_entries.iter().copied().map(integer).collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "rejections",
            Expr::Vector(
                graph
                    .rejections
                    .iter()
                    .map(|rejection| {
                        map(vec![
                            ("first", integer(rejection.first)),
                            ("second", integer(rejection.second)),
                            ("span", span_expr(&rejection.overlap.span)),
                            (
                                "violations",
                                Expr::Vector(
                                    rejection.violations.iter().map(violation_expr).collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn form_name(form: ContrapuntalForm) -> String {
    match form {
        ContrapuntalForm::Original => "original".to_owned(),
        ContrapuntalForm::Retrograde => "retrograde".to_owned(),
        ContrapuntalForm::Inversion { axis } => format!("inversion-{}", axis.semitone()),
        ContrapuntalForm::RetrogradeInversion { axis } => {
            format!("retrograde-inversion-{}", axis.semitone())
        }
    }
}
