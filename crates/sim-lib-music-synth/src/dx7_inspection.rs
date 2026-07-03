use sim_kernel::{Expr, NumberLiteral, Symbol};

use crate::{
    ComponentBackend, DX7_GAIN_UNITY, Dx7AlgorithmEdge, Dx7AlgorithmTopology, Dx7CarrierOutput,
};

/// A snapshot of a DX7 algorithm's operator graph: nodes, modulation and
/// carrier edges, and summary counts, suitable for inspection or export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dx7GraphInspection {
    /// Identifier (1..=32) of the DX7 algorithm this graph describes.
    pub algorithm_id: u8,
    /// Backend the inspected voice runs on (algorithmic or modeled).
    pub backend: ComponentBackend,
    /// Total number of graph nodes, including the synthetic mix node.
    pub node_count: usize,
    /// Number of operator-to-operator modulation edges.
    pub modulation_edge_count: usize,
    /// Number of carrier operators feeding the output mix.
    pub carrier_count: usize,
    /// The feedback edge, if the algorithm has one.
    pub feedback_edge: Option<Dx7GraphEdgeInspection>,
    /// The graph nodes (one per operator plus the mix node).
    pub nodes: Vec<Dx7GraphNodeInspection>,
    /// All graph edges: modulation, carrier, and feedback.
    pub edges: Vec<Dx7GraphEdgeInspection>,
}

impl Dx7GraphInspection {
    /// Builds a graph inspection by walking the operators, modulation edges,
    /// carriers, and feedback edge of `topology` for the given `backend`.
    pub fn new(topology: &Dx7AlgorithmTopology, backend: ComponentBackend) -> Self {
        let mut nodes = topology
            .operator_order
            .iter()
            .map(|operator| {
                let gain = topology
                    .gain_for_operator(*operator)
                    .map(|gain| gain.gain_raw)
                    .unwrap_or(DX7_GAIN_UNITY);
                Dx7GraphNodeInspection {
                    id: operator_node(*operator),
                    operator: Some(*operator),
                    carrier: topology
                        .carrier_outputs()
                        .any(|carrier| carrier.operator == *operator),
                    gain_raw: gain,
                }
            })
            .collect::<Vec<_>>();
        nodes.push(Dx7GraphNodeInspection {
            id: "mix".to_owned(),
            operator: None,
            carrier: false,
            gain_raw: 0,
        });

        let mut edges = topology
            .modulation_edges()
            .map(|edge| graph_edge(topology, edge, "modulation", 0))
            .collect::<Vec<_>>();
        for carrier in topology.carrier_outputs() {
            edges.push(carrier_edge(carrier));
        }
        let feedback_edge = topology.feedback_edge.map(|edge| {
            let edge = graph_edge(topology, edge, "feedback", 1);
            edges.push(edge.clone());
            edge
        });

        Self {
            algorithm_id: topology.id,
            backend,
            node_count: nodes.len(),
            modulation_edge_count: topology.modulation_edges().count(),
            carrier_count: topology.carrier_count(),
            feedback_edge,
            nodes,
            edges,
        }
    }

    /// Serializes the inspection into a SIM `Expr` map, with nested vectors of
    /// node and edge maps, for codec output.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                field("tag"),
                Expr::Symbol(Symbol::qualified("audio-synth", "dx7-graph-inspection")),
            ),
            (field("algorithm"), number_u8(self.algorithm_id)),
            (field("backend"), Expr::Symbol(self.backend.symbol())),
            (field("node-count"), number_usize(self.node_count)),
            (
                field("modulation-edge-count"),
                number_usize(self.modulation_edge_count),
            ),
            (field("carrier-count"), number_usize(self.carrier_count)),
            (
                field("feedback"),
                self.feedback_edge
                    .as_ref()
                    .map(Dx7GraphEdgeInspection::to_expr)
                    .unwrap_or(Expr::Bool(false)),
            ),
            (
                field("nodes"),
                Expr::Vector(
                    self.nodes
                        .iter()
                        .map(Dx7GraphNodeInspection::to_expr)
                        .collect(),
                ),
            ),
            (
                field("edges"),
                Expr::Vector(
                    self.edges
                        .iter()
                        .map(Dx7GraphEdgeInspection::to_expr)
                        .collect(),
                ),
            ),
        ])
    }
}

/// A single node in the inspected DX7 graph: an operator or the output mix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dx7GraphNodeInspection {
    /// Node label (e.g. `"op1"` for operators, `"mix"` for the output node).
    pub id: String,
    /// Operator number (1..=6) for operator nodes, or `None` for the mix node.
    pub operator: Option<u8>,
    /// Whether this operator is a carrier feeding the output mix.
    pub carrier: bool,
    /// The operator's raw fixed-point gain value.
    pub gain_raw: u16,
}

impl Dx7GraphNodeInspection {
    fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            (field("id"), Expr::String(self.id.clone())),
            (
                field("operator"),
                self.operator
                    .map(number_u8)
                    .unwrap_or_else(|| Expr::Bool(false)),
            ),
            (field("carrier"), Expr::Bool(self.carrier)),
            (field("gain-raw"), number_u16(self.gain_raw)),
        ])
    }
}

/// A single edge in the inspected DX7 graph connecting two nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dx7GraphEdgeInspection {
    /// Label of the source node.
    pub from: String,
    /// Label of the destination node.
    pub to: String,
    /// Edge kind symbol: modulation, carrier, or feedback.
    pub kind: Symbol,
    /// Raw fixed-point gain applied along this edge.
    pub gain_raw: u16,
    /// Edge delay in frames (nonzero for feedback edges).
    pub delay_frames: u32,
}

impl Dx7GraphEdgeInspection {
    fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            (field("from"), Expr::String(self.from.clone())),
            (field("to"), Expr::String(self.to.clone())),
            (field("kind"), Expr::Symbol(self.kind.clone())),
            (field("gain-raw"), number_u16(self.gain_raw)),
            (field("delay-frames"), number_u32(self.delay_frames)),
        ])
    }
}

fn graph_edge(
    topology: &Dx7AlgorithmTopology,
    edge: Dx7AlgorithmEdge,
    kind: &'static str,
    delay_frames: u32,
) -> Dx7GraphEdgeInspection {
    Dx7GraphEdgeInspection {
        from: operator_node(edge.from_operator),
        to: operator_node(edge.to_operator),
        kind: Symbol::qualified("audio-synth/dx7-edge", kind),
        gain_raw: topology
            .gain_for_operator(edge.gain_point)
            .map(|gain| gain.gain_raw)
            .unwrap_or(DX7_GAIN_UNITY),
        delay_frames,
    }
}

fn carrier_edge(carrier: Dx7CarrierOutput) -> Dx7GraphEdgeInspection {
    Dx7GraphEdgeInspection {
        from: operator_node(carrier.operator),
        to: "mix".to_owned(),
        kind: Symbol::qualified("audio-synth/dx7-edge", "carrier"),
        gain_raw: carrier.gain_raw,
        delay_frames: 0,
    }
}

fn operator_node(operator: u8) -> String {
    format!("op{operator}")
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym("audio-synth/dx7-graph", name)
}

fn number_u8(value: u8) -> Expr {
    number_i64(i64::from(value))
}

fn number_u16(value: u16) -> Expr {
    number_i64(i64::from(value))
}

fn number_u32(value: u32) -> Expr {
    number_i64(i64::from(value))
}

fn number_usize(value: usize) -> Expr {
    number_i64(value.min(i64::MAX as usize) as i64)
}

fn number_i64(value: i64) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}
