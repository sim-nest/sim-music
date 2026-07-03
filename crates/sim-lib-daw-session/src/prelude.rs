use sim_kernel::{Expr, Symbol};

/// Lisp-facing DAW operation names exposed by the DAW prelude.
pub fn daw_prelude_operations() -> Vec<Symbol> {
    [
        "session",
        "track",
        "bus",
        "clip",
        "plugin-chain",
        "save",
        "load",
        "render-offline",
        "browse",
        "topology-package",
    ]
    .into_iter()
    .map(|name| Symbol::qualified("daw", name))
    .collect()
}

/// Builds the DAW prelude help card listing the exposed operations.
pub fn daw_prelude_card_expr() -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("subject")),
            Expr::Symbol(Symbol::qualified("daw", "prelude")),
        ),
        (
            Expr::Symbol(Symbol::new("kind")),
            Expr::Symbol(Symbol::qualified("browse", "help-card")),
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
