use sim_kernel::{CapabilityName, Expr, Symbol};
use sim_lib_topology::{Edge, Graph, GraphTest, Node, PortRef, TopologyPackage};
use sim_value::build::uint;

use crate::DawSession;

/// Builds a topology package that can launch a DAW session render path.
pub fn daw_session_topology_package(session: &DawSession) -> TopologyPackage {
    let mut graph = Graph::minimal(format!("daw-{}", session.id().name.as_ref()));
    let mut load = Node::named("session", "call");
    load.target = Some(session.to_expr());
    load.role = Some(Symbol::qualified("daw", "load"));

    let mut render = Node::named("render", "call");
    render.target = Some(Expr::Symbol(Symbol::qualified("daw", "render-offline")));
    render.role = Some(Symbol::qualified("daw", "render-offline"));

    graph.nodes = vec![
        Node::named("in", "in"),
        load,
        render,
        Node::named("out", "out"),
    ];
    graph.edges = vec![
        Edge::new(0, PortRef::output("in"), PortRef::input("session")),
        Edge::new(1, PortRef::output("session"), PortRef::input("render")),
        Edge::new(2, PortRef::output("render"), PortRef::input("out")),
    ];
    graph.metadata = vec![
        (
            Symbol::qualified("daw-session", "session-id"),
            Expr::Symbol(session.id().clone()),
        ),
        (
            Symbol::qualified("daw-session", "session-name"),
            Expr::String(session.name().to_owned()),
        ),
        (
            Symbol::qualified("daw-session", "instrument-count"),
            uint(session.instrument_instances().len() as u64),
        ),
        (
            Symbol::qualified("daw-session", "route-count"),
            uint(session.routes().len() as u64),
        ),
    ];
    graph.tests = vec![GraphTest::new(
        Symbol::new("session-load-smoke"),
        Expr::Nil,
        Expr::Symbol(session.id().clone()),
    )];

    TopologyPackage {
        tests: graph.tests.clone(),
        metadata: graph.metadata.clone(),
        capabilities: graph_package_capabilities(&graph),
        graph,
    }
}

trait PackageCapability: Sized {
    fn from_graph_symbol(capability: &Symbol) -> Self;
}

impl PackageCapability for Symbol {
    fn from_graph_symbol(capability: &Symbol) -> Self {
        capability.clone()
    }
}

impl PackageCapability for CapabilityName {
    fn from_graph_symbol(capability: &Symbol) -> Self {
        CapabilityName::new(capability.as_qualified_str())
    }
}

fn graph_package_capabilities<C>(graph: &Graph) -> Vec<C>
where
    C: PackageCapability,
{
    graph
        .capabilities
        .iter()
        .map(PackageCapability::from_graph_symbol)
        .collect()
}
