use sim_kernel::{Expr, Symbol};

use crate::{DawSession, daw_prelude_operations};

/// Builds the browse card describing a session: its name, tracks, buses, and
/// patch nodes, keyed by stable symbols for agent inspection.
pub fn session_card_expr(session: &DawSession) -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("subject")),
            Expr::Symbol(session.id().clone()),
        ),
        (
            Expr::Symbol(Symbol::new("kind")),
            Expr::Symbol(Symbol::qualified("daw-session", "session-card")),
        ),
        (
            Expr::Symbol(Symbol::new("name")),
            Expr::String(session.name().to_owned()),
        ),
        (
            Expr::Symbol(Symbol::new("tracks")),
            Expr::Vector(
                session
                    .tracks()
                    .iter()
                    .map(|track| Expr::Symbol(track.id().clone()))
                    .collect(),
            ),
        ),
        (
            Expr::Symbol(Symbol::new("buses")),
            Expr::Vector(
                session
                    .buses()
                    .iter()
                    .map(|bus| Expr::Symbol(bus.id().clone()))
                    .collect(),
            ),
        ),
        (
            Expr::Symbol(Symbol::new("patch-nodes")),
            Expr::Vector(
                session
                    .patch()
                    .nodes
                    .iter()
                    .map(|node| Expr::String(node.id.clone()))
                    .collect(),
            ),
        ),
    ])
}

/// Builds the help card listing the role of the DAW session surface and the
/// prelude operations it exposes.
pub fn session_help_card_expr() -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("subject")),
            Expr::Symbol(Symbol::qualified("daw-session", "help")),
        ),
        (
            Expr::Symbol(Symbol::new("kind")),
            Expr::Symbol(Symbol::qualified("browse", "help-card")),
        ),
        (
            Expr::Symbol(Symbol::new("role")),
            Expr::String("headless DAW session save/load/render surface".to_owned()),
        ),
        (
            Expr::Symbol(Symbol::new("operations")),
            Expr::Vector(
                daw_prelude_operations()
                    .into_iter()
                    .map(Expr::Symbol)
                    .collect(),
            ),
        ),
    ])
}

/// Returns the full set of browse cards for a session: the session card, the
/// help card, then one card per track and per bus.
pub fn browse_session_graph(session: &DawSession) -> Vec<Expr> {
    let mut cards = vec![session_card_expr(session), session_help_card_expr()];
    cards.extend(session.tracks().iter().map(|track| {
        Expr::Map(vec![
            (
                Expr::Symbol(Symbol::new("subject")),
                Expr::Symbol(track.id().clone()),
            ),
            (
                Expr::Symbol(Symbol::new("kind")),
                Expr::Symbol(Symbol::qualified("daw-session", "track-card")),
            ),
            (
                Expr::Symbol(Symbol::new("session")),
                Expr::Symbol(session.id().clone()),
            ),
            (
                Expr::Symbol(Symbol::new("clips")),
                Expr::Vector(
                    track
                        .clips()
                        .iter()
                        .map(|clip| Expr::Symbol(clip.id().clone()))
                        .collect(),
                ),
            ),
        ])
    }));
    cards.extend(session.buses().iter().map(|bus| {
        Expr::Map(vec![
            (
                Expr::Symbol(Symbol::new("subject")),
                Expr::Symbol(bus.id().clone()),
            ),
            (
                Expr::Symbol(Symbol::new("kind")),
                Expr::Symbol(Symbol::qualified("daw-session", "bus-card")),
            ),
            (
                Expr::Symbol(Symbol::new("session")),
                Expr::Symbol(session.id().clone()),
            ),
        ])
    }));
    cards
}
