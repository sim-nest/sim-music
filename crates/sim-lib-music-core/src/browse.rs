use sim_kernel::{Error, Expr, Result, Symbol};
use sim_value::build::entry;

use crate::{MusicComponentDescriptor, MusicComponentRegistry, MusicComponentRegistryEntry};

/// Lists the registered component ids exposed to the browse surface.
///
/// Returns the id symbol of every entry in `registry`, in registration order.
pub fn music_browse_symbols(registry: &MusicComponentRegistry) -> Vec<Symbol> {
    registry.entries().map(|entry| entry.id().clone()).collect()
}

/// Builds the browse card expression for `subject` from the registry.
///
/// Looks up `subject` in `registry` and renders its component card, returning an
/// evaluation error when the subject is not registered.
pub fn music_browse_card_expr(registry: &MusicComponentRegistry, subject: &Symbol) -> Result<Expr> {
    let entry = registry
        .get(subject)
        .ok_or_else(|| Error::Eval(format!("music browse subject not registered: {subject}")))?;
    Ok(music_component_card(entry))
}

/// Renders the full browse card for a single registry entry.
///
/// Assembles a card-v2 map from the entry's descriptor, covering its kind,
/// help text, arguments, output families, registry test, and capability links.
pub fn music_component_card(entry: &MusicComponentRegistryEntry) -> Expr {
    let descriptor = entry.descriptor();
    card_v2(CardV2Spec {
        subject: descriptor.id.clone(),
        kind: descriptor.category.symbol(),
        summary: descriptor.label.clone(),
        detail: component_detail(descriptor),
        args: descriptor.to_expr(),
        result: Expr::Vector(
            descriptor
                .output_families
                .iter()
                .map(|kind| Expr::Symbol(kind.symbol()))
                .collect(),
        ),
        tests: vec![browse_test_card(descriptor)],
        requires: descriptor
            .capabilities
            .iter()
            .map(|capability| Expr::Symbol(capability.symbol()))
            .collect(),
        see_also: descriptor
            .capabilities
            .iter()
            .map(|capability| Expr::Symbol(capability.symbol()))
            .collect(),
    })
}

fn component_detail(descriptor: &MusicComponentDescriptor) -> String {
    format!(
        "{} exposes {} port(s), {} lane(s), {} parameter(s), {} determinism, and {} latency.",
        descriptor.label,
        descriptor.ports.len(),
        descriptor.lanes.len(),
        descriptor.params.len(),
        descriptor.determinism.wire_label(),
        descriptor.rate.latency_class().symbol()
    )
}

struct CardV2Spec {
    subject: Symbol,
    kind: Symbol,
    summary: String,
    detail: String,
    args: Expr,
    result: Expr,
    tests: Vec<Expr>,
    requires: Vec<Expr>,
    see_also: Vec<Expr>,
}

fn card_v2(spec: CardV2Spec) -> Expr {
    let CardV2Spec {
        subject,
        kind,
        summary,
        detail,
        args,
        result,
        tests,
        requires,
        see_also,
    } = spec;
    Expr::Map(vec![
        entry("subject", Expr::Symbol(subject.clone())),
        entry("kind", Expr::Symbol(kind)),
        entry(
            "help",
            help_expr(subject, summary, detail, see_also.clone()),
        ),
        entry("args", args),
        entry("result", result),
        entry("tests", Expr::List(tests)),
        entry("ops", Expr::List(Vec::new())),
        entry("requires", Expr::List(requires)),
        entry("see-also", Expr::List(see_also)),
        entry("shape-known", Expr::Bool(true)),
        entry("facets", Expr::List(Vec::new())),
        entry("coverage", Expr::Symbol(Symbol::new("covered"))),
        entry("provenance", Expr::List(Vec::new())),
        entry("freshness", Expr::Symbol(Symbol::new("fresh"))),
    ])
}

fn help_expr(subject: Symbol, summary: String, detail: String, see_also: Vec<Expr>) -> Expr {
    Expr::Map(vec![
        entry("subject", Expr::Symbol(subject)),
        entry("kind", Expr::Symbol(Symbol::qualified("core", "function"))),
        entry("summary", Expr::String(summary)),
        entry("detail", Expr::String(detail)),
        entry(
            "exported-by",
            Expr::Symbol(Symbol::qualified("sim", "music")),
        ),
        entry("stability", Expr::Symbol(Symbol::new("experimental"))),
        entry("capabilities", Expr::List(Vec::new())),
        entry("demand", Expr::List(Vec::new())),
        entry("see-also", Expr::List(see_also)),
    ])
}

fn browse_test_card(descriptor: &MusicComponentDescriptor) -> Expr {
    Expr::Map(vec![
        entry(
            "name",
            Expr::Symbol(Symbol::qualified(
                "music/component-test",
                descriptor.id.as_qualified_str(),
            )),
        ),
        entry(
            "subjects",
            Expr::List(vec![Expr::Symbol(descriptor.id.clone())]),
        ),
        entry("lib", Expr::Symbol(Symbol::qualified("sim", "music"))),
        entry("mode", Expr::Symbol(Symbol::new("registry"))),
        entry("expr", descriptor.to_expr()),
        entry("expect", Expr::Symbol(Symbol::new("present"))),
    ])
}
