use sim_kernel::{Expr, Result, Symbol};
use sim_value::build::uint;

use crate::DawSession;

const BUILDER_NS: &str = "daw-session/component-builder";
const ACTION_NS: &str = "component-builder/action";

/// Format tag stamped into every component-builder patch and preview export.
pub const COMPONENT_BUILDER_PATCH_FORMAT: &str = "component-builder-patch-v1";
/// Ordered list of action names a component-builder UI may invoke against a
/// session (connect, disconnect, save, load, live-preview, and so on).
pub const COMPONENT_BUILDER_ACTIONS: [&str; 19] = [
    "connect",
    "disconnect",
    "add-module",
    "duplicate",
    "delete",
    "bypass",
    "reset",
    "inspect",
    "route-matrix",
    "enable-section",
    "disable-section",
    "midi-route",
    "automation-route",
    "patch-edit",
    "trace-view",
    "preview-render",
    "save",
    "load",
    "live-preview",
];

impl DawSession {
    /// Exports the session as a component-builder patch: stable component ids,
    /// instrument instances, routes, the audio graph patch, the saved session,
    /// and the available builder actions and hooks.
    pub fn component_builder_patch_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                field("tag"),
                Expr::Symbol(Symbol::qualified("daw-session", "component-builder-patch")),
            ),
            (
                field("format"),
                Expr::String(COMPONENT_BUILDER_PATCH_FORMAT.to_owned()),
            ),
            (field("session-id"), Expr::Symbol(self.id().clone())),
            (
                field("stable-component-ids"),
                Expr::Vector(stable_component_ids(self)),
            ),
            (
                field("instrument-instances"),
                Expr::Vector(
                    self.instrument_instances()
                        .iter()
                        .map(|instrument| {
                            Expr::String(format!(
                                "{}:{}",
                                instrument.kind().as_str(),
                                instrument.graph_node_id()
                            ))
                        })
                        .collect(),
                ),
            ),
            (
                field("routes"),
                Expr::Vector(
                    self.routes()
                        .iter()
                        .map(|route| {
                            Expr::String(format!(
                                "{}:{}",
                                route.kind().as_str(),
                                route.target_node_id()
                            ))
                        })
                        .collect(),
                ),
            ),
            (field("patch"), self.patch().to_expr()),
            (field("saved-session"), self.save_expr()),
            (
                field("actions"),
                Expr::Vector(component_builder_action_exprs()),
            ),
            (field("hooks"), hooks_expr()),
        ])
    }

    /// Renders `frames` of offline audio and exports a component-builder
    /// preview card summarizing the render counters and live-preview hook.
    pub fn component_builder_preview_expr(&self, frames: usize) -> Result<Expr> {
        let render = self.render_offline(frames)?;
        Ok(Expr::Map(vec![
            (
                field("tag"),
                Expr::Symbol(Symbol::qualified(
                    "daw-session",
                    "component-builder-preview",
                )),
            ),
            (
                field("format"),
                Expr::String(COMPONENT_BUILDER_PATCH_FORMAT.to_owned()),
            ),
            (field("session-id"), Expr::Symbol(self.id().clone())),
            (field("frames"), uint(frames as u64)),
            (field("sample-rate-hz"), uint(self.sample_rate_hz() as u64)),
            (
                field("tracks-rendered"),
                uint(render.tracks_rendered() as u64),
            ),
            (
                field("clips-rendered"),
                uint(render.clips_rendered() as u64),
            ),
            (
                field("stable-component-ids"),
                Expr::Vector(stable_component_ids(self)),
            ),
            (
                field("instrument-instances"),
                Expr::Vector(
                    self.instrument_instances()
                        .iter()
                        .map(|instrument| Expr::String(instrument.id().to_string()))
                        .collect(),
                ),
            ),
            (
                field("routes"),
                Expr::Vector(
                    self.routes()
                        .iter()
                        .map(|route| Expr::String(route.kind().as_str().to_owned()))
                        .collect(),
                ),
            ),
            (field("hook"), action("live-preview")),
        ]))
    }
}

/// Returns the [`COMPONENT_BUILDER_ACTIONS`] names as namespaced symbols.
pub fn component_builder_action_symbols() -> Vec<Symbol> {
    COMPONENT_BUILDER_ACTIONS
        .iter()
        .map(|name| Symbol::qualified(ACTION_NS, *name))
        .collect()
}

fn component_builder_action_exprs() -> Vec<Expr> {
    COMPONENT_BUILDER_ACTIONS
        .iter()
        .map(|name| action(name))
        .collect()
}

fn stable_component_ids(session: &DawSession) -> Vec<Expr> {
    session
        .patch()
        .nodes
        .iter()
        .map(|node| Expr::String(node.id.clone()))
        .collect()
}

fn hooks_expr() -> Expr {
    Expr::Map(vec![
        (field("save"), action("save")),
        (field("load"), action("load")),
        (field("live-preview"), action("live-preview")),
    ])
}

fn action(name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified(ACTION_NS, name))
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym(BUILDER_NS, name)
}
