use sim_kernel::{Expr, Symbol};

use crate::{
    ComponentCapability, ComponentRegistry, ComponentRegistryCategory, ComponentRegistryEntry,
};

const BUILDER_NS: &str = "audio-synth/component-builder";

/// The patch format identifier emitted by the component builder palette.
pub const COMPONENT_BUILDER_PATCH_FORMAT: &str = "component-builder-patch-v1";

impl ComponentRegistry {
    /// The patch format identifier emitted by the component builder palette.
    pub const COMPONENT_BUILDER_PATCH_FORMAT: &'static str = COMPONENT_BUILDER_PATCH_FORMAT;

    /// Renders the component builder palette as an expression, optionally
    /// filtered by fidelity category and capability.
    pub fn component_palette_expr(
        &self,
        category_filter: Option<ComponentRegistryCategory>,
        capability_filter: Option<ComponentCapability>,
    ) -> Expr {
        Expr::Map(vec![
            (
                field("tag"),
                Expr::Symbol(Symbol::qualified("audio-synth", "component-palette")),
            ),
            (
                field("patch-format"),
                Expr::String(COMPONENT_BUILDER_PATCH_FORMAT.to_owned()),
            ),
            (
                field("category-filter"),
                category_filter
                    .map(|category| Expr::Symbol(category.symbol()))
                    .unwrap_or(Expr::Nil),
            ),
            (
                field("capability-filter"),
                capability_filter
                    .map(|capability| Expr::Symbol(capability.symbol()))
                    .unwrap_or(Expr::Nil),
            ),
            (
                field("items"),
                Expr::Vector(
                    self.entries()
                        .filter(|entry| category_matches(entry, category_filter))
                        .filter(|entry| capability_matches(entry, capability_filter))
                        .map(palette_item_expr)
                        .collect(),
                ),
            ),
        ])
    }
}

fn category_matches(
    entry: &ComponentRegistryEntry,
    category_filter: Option<ComponentRegistryCategory>,
) -> bool {
    category_filter.is_none_or(|category| entry.category() == category)
}

fn capability_matches(
    entry: &ComponentRegistryEntry,
    capability_filter: Option<ComponentCapability>,
) -> bool {
    capability_filter.is_none_or(|capability| entry.has_capability(capability))
}

fn palette_item_expr(entry: &ComponentRegistryEntry) -> Expr {
    Expr::Map(vec![
        (field("id"), Expr::Symbol(entry.id().clone())),
        (field("label"), Expr::String(entry.label().to_owned())),
        (field("category"), Expr::Symbol(entry.category().symbol())),
        (field("wrapper"), Expr::Symbol(entry.wrapper().symbol())),
        (
            field("capabilities"),
            Expr::Vector(
                entry
                    .capabilities()
                    .iter()
                    .map(|capability| Expr::Symbol(capability.symbol()))
                    .collect(),
            ),
        ),
        (field("implemented"), Expr::Bool(entry.is_implemented())),
        (field("descriptor"), entry.to_editor_descriptor_expr()),
    ])
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym(BUILDER_NS, name)
}
