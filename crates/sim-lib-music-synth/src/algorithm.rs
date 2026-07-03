use sim_kernel::{Expr, NumberLiteral, Symbol};
use sim_lib_topology::{
    Graph, InstrumentTopologyAdapter, InstrumentTopologyCord, InstrumentTopologyJack,
    InstrumentTopologyModule, InstrumentTopologySpec, PortRef,
};

use crate::{ComponentBackend, dx7_modeled_operator_component_id, dx7_operator_component_id};

/// Number of DX7 FM algorithms.
pub const DX7_ALGORITHM_COUNT: usize = 32;
/// Number of operators in a DX7 voice.
pub const DX7_OPERATOR_COUNT: usize = 6;
/// Raw gain value representing unity (1.0).
pub const DX7_GAIN_UNITY: u16 = 1024;

/// A directed modulation or feedback edge between two operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx7AlgorithmEdge {
    /// Source (modulator) operator number.
    pub from_operator: u8,
    /// Destination (modulated) operator number.
    pub to_operator: u8,
    /// Gain point applied along the edge.
    pub gain_point: u8,
}

/// A carrier operator routed to the voice output mixer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx7CarrierOutput {
    /// Carrier operator number.
    pub operator: u8,
    /// Gain point applied at the output.
    pub gain_point: u8,
    /// Raw output gain, where [`DX7_GAIN_UNITY`] is 1.0.
    pub gain_raw: u16,
}

impl Dx7CarrierOutput {
    /// Returns the carrier gain as a unit-normalized float.
    pub fn gain(self) -> f32 {
        f32::from(self.gain_raw) / f32::from(DX7_GAIN_UNITY)
    }
}

/// A per-operator gain point in an algorithm topology.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx7AlgorithmGainPoint {
    /// Operator the gain point applies to.
    pub operator: u8,
    /// Raw gain, where [`DX7_GAIN_UNITY`] is 1.0.
    pub gain_raw: u16,
}

impl Dx7AlgorithmGainPoint {
    /// Returns the gain as a unit-normalized float.
    pub fn gain(self) -> f32 {
        f32::from(self.gain_raw) / f32::from(DX7_GAIN_UNITY)
    }
}

/// Full routing topology of one DX7 algorithm: operator order, modulation and
/// feedback edges, carrier outputs, and gain points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx7AlgorithmTopology {
    /// Algorithm number (1..=32).
    pub id: u8,
    /// Operator processing order.
    pub operator_order: [u8; DX7_OPERATOR_COUNT],
    /// Up to five modulation edges (unused slots are `None`).
    pub modulation_edges: [Option<Dx7AlgorithmEdge>; 5],
    /// Optional self-feedback edge.
    pub feedback_edge: Option<Dx7AlgorithmEdge>,
    /// Carrier outputs by operator slot (non-carriers are `None`).
    pub carrier_outputs: [Option<Dx7CarrierOutput>; DX7_OPERATOR_COUNT],
    /// Per-operator gain points.
    pub gain_points: [Dx7AlgorithmGainPoint; DX7_OPERATOR_COUNT],
}

impl Dx7AlgorithmTopology {
    /// Iterates the present modulation edges, skipping empty slots.
    pub fn modulation_edges(&self) -> impl Iterator<Item = Dx7AlgorithmEdge> + '_ {
        self.modulation_edges.iter().filter_map(|edge| *edge)
    }

    /// Iterates the present carrier outputs, skipping non-carrier slots.
    pub fn carrier_outputs(&self) -> impl Iterator<Item = Dx7CarrierOutput> + '_ {
        self.carrier_outputs.iter().filter_map(|carrier| *carrier)
    }

    /// Returns the number of carrier operators.
    pub fn carrier_count(&self) -> usize {
        self.carrier_outputs().count()
    }

    /// Returns the gain point for the given operator, if any.
    pub fn gain_for_operator(&self, operator: u8) -> Option<Dx7AlgorithmGainPoint> {
        self.gain_points
            .iter()
            .copied()
            .find(|gain| gain.operator == operator)
    }

    /// Lowers the topology into a generic instrument topology spec for the given
    /// backend, wiring operators, the carrier mixer, and modulation cords.
    pub fn to_topology_spec(&self, backend: ComponentBackend) -> InstrumentTopologySpec {
        let mut spec = InstrumentTopologySpec::new(Symbol::qualified(
            "audio-synth/dx7-algorithm",
            format!("{:02}", self.id),
        ))
        .with_metadata(Symbol::new("algorithm"), number_u8(self.id))
        .with_metadata(Symbol::new("backend"), Expr::Symbol(backend.symbol()));

        for operator in self.operator_order {
            let gain = self
                .gain_for_operator(operator)
                .map(|gain| gain.gain_raw)
                .unwrap_or(DX7_GAIN_UNITY);
            spec = spec.with_module(
                InstrumentTopologyModule::new(operator_node(operator), operator_kind(backend))
                    .with_input(InstrumentTopologyJack::stream("modulation", false))
                    .with_output(InstrumentTopologyJack::stream("audio", true))
                    .with_setting(Symbol::new("operator"), number_u8(operator))
                    .with_setting(Symbol::new("gain-raw"), number_u16(gain))
                    .with_setting(
                        Symbol::new("carrier"),
                        Expr::Bool(
                            self.carrier_outputs()
                                .any(|carrier| carrier.operator == operator),
                        ),
                    ),
            );
        }

        let mut mix =
            InstrumentTopologyModule::new("mix", Symbol::qualified("audio-synth/dx7", "mix"))
                .with_output(InstrumentTopologyJack::stream("audio", true));
        for carrier in self.carrier_outputs() {
            mix = mix
                .with_input(InstrumentTopologyJack::stream(
                    carrier_input(carrier.operator),
                    true,
                ))
                .with_setting(
                    Symbol::new(format!("gain-op{}", carrier.operator)),
                    number_u16(carrier.gain_raw),
                );
        }
        spec = spec.with_module(mix);

        for edge in self.modulation_edges() {
            spec = spec.with_cord(InstrumentTopologyCord::new(
                PortRef::named(operator_node(edge.from_operator), "audio"),
                PortRef::named(operator_node(edge.to_operator), "modulation"),
            ));
        }
        for carrier in self.carrier_outputs() {
            spec = spec.with_cord(InstrumentTopologyCord::new(
                PortRef::named(operator_node(carrier.operator), "audio"),
                PortRef::named("mix", carrier_input(carrier.operator)),
            ));
        }
        if let Some(feedback) = self.feedback_edge {
            spec = spec.with_metadata(
                Symbol::new("feedback-from"),
                number_u8(feedback.from_operator),
            );
            spec = spec.with_metadata(Symbol::new("feedback-to"), number_u8(feedback.to_operator));
        }
        spec
    }

    /// Builds the runnable instrument graph for this algorithm on the given
    /// backend.
    pub fn to_topology_graph(&self, backend: ComponentBackend) -> Graph {
        InstrumentTopologyAdapter.graph_from_spec(&self.to_topology_spec(backend))
    }
}

/// The 32 DX7 algorithm topologies, indexed by algorithm number minus one.
pub const DX7_ALGORITHM_TOPOLOGIES: [Dx7AlgorithmTopology; DX7_ALGORITHM_COUNT] = [
    alg(
        1,
        [e(1, 2), e(2, 3), e(3, 4), e(4, 5), e(5, 6)],
        carriers(0, 0, 0, 0, 0, 6),
    ),
    alg(
        2,
        [e(1, 2), e(2, 3), e(3, 6), e(4, 5), e(5, 6)],
        carriers(0, 0, 0, 0, 0, 6),
    ),
    alg(
        3,
        [e(1, 3), e(2, 3), e(3, 4), e(4, 5), e(5, 6)],
        carriers(0, 0, 0, 0, 0, 6),
    ),
    alg(
        4,
        [e(1, 2), e(2, 6), e(3, 4), e(4, 5), e(5, 6)],
        carriers(0, 0, 0, 0, 0, 6),
    ),
    alg(
        5,
        [e(1, 2), e(2, 3), e(3, 6), e(4, 6), e(5, 6)],
        carriers(0, 0, 0, 0, 0, 6),
    ),
    alg(
        6,
        [e(1, 2), e(2, 6), e(3, 4), e(4, 6), e(5, 6)],
        carriers(0, 0, 0, 0, 0, 6),
    ),
    alg(
        7,
        [e(1, 6), e(2, 3), e(3, 6), e(4, 5), e(5, 6)],
        carriers(0, 0, 0, 0, 0, 6),
    ),
    alg(
        8,
        [e(1, 6), e(2, 6), e(3, 6), e(4, 5), e(5, 6)],
        carriers(0, 0, 0, 0, 0, 6),
    ),
    alg(
        9,
        [e(1, 2), e(2, 3), e(3, 4), e(4, 5), n()],
        carriers(0, 0, 0, 0, 5, 6),
    ),
    alg(
        10,
        [e(1, 2), e(2, 3), e(3, 5), e(4, 5), n()],
        carriers(0, 0, 0, 0, 5, 6),
    ),
    alg(
        11,
        [e(1, 2), e(2, 5), e(3, 4), e(4, 5), n()],
        carriers(0, 0, 0, 0, 5, 6),
    ),
    alg(
        12,
        [e(1, 5), e(2, 3), e(3, 4), e(4, 5), n()],
        carriers(0, 0, 0, 0, 5, 6),
    ),
    alg(
        13,
        [e(1, 2), e(2, 3), e(3, 6), e(4, 5), n()],
        carriers(0, 0, 0, 0, 5, 6),
    ),
    alg(
        14,
        [e(1, 2), e(2, 6), e(3, 4), e(4, 5), n()],
        carriers(0, 0, 0, 0, 5, 6),
    ),
    alg(
        15,
        [e(1, 5), e(2, 6), e(3, 4), e(4, 5), n()],
        carriers(0, 0, 0, 0, 5, 6),
    ),
    alg(
        16,
        [e(1, 5), e(2, 6), e(3, 6), e(4, 5), n()],
        carriers(0, 0, 0, 0, 5, 6),
    ),
    alg(
        17,
        [e(1, 2), e(2, 3), e(3, 4), n(), n()],
        carriers(0, 0, 0, 4, 5, 6),
    ),
    alg(
        18,
        [e(1, 2), e(2, 4), e(3, 4), n(), n()],
        carriers(0, 0, 0, 4, 5, 6),
    ),
    alg(
        19,
        [e(1, 4), e(2, 3), e(3, 4), n(), n()],
        carriers(0, 0, 0, 4, 5, 6),
    ),
    alg(
        20,
        [e(1, 4), e(2, 5), e(3, 6), n(), n()],
        carriers(0, 0, 0, 4, 5, 6),
    ),
    alg(
        21,
        [e(1, 2), e(2, 5), e(3, 4), n(), n()],
        carriers(0, 0, 0, 4, 5, 6),
    ),
    alg(
        22,
        [e(1, 2), e(2, 6), e(3, 4), n(), n()],
        carriers(0, 0, 0, 4, 5, 6),
    ),
    alg(
        23,
        [e(1, 4), e(2, 5), e(3, 5), n(), n()],
        carriers(0, 0, 0, 4, 5, 6),
    ),
    alg(
        24,
        [e(1, 4), e(2, 5), e(3, 6), n(), n()],
        carriers(0, 0, 0, 4, 5, 6),
    ),
    alg(
        25,
        [e(1, 2), e(2, 3), n(), n(), n()],
        carriers(0, 0, 3, 4, 5, 6),
    ),
    alg(
        26,
        [e(1, 3), e(2, 3), n(), n(), n()],
        carriers(0, 0, 3, 4, 5, 6),
    ),
    alg(
        27,
        [e(1, 4), e(2, 3), n(), n(), n()],
        carriers(0, 0, 3, 4, 5, 6),
    ),
    alg(
        28,
        [e(1, 5), e(2, 6), n(), n(), n()],
        carriers(0, 0, 3, 4, 5, 6),
    ),
    alg(
        29,
        [e(1, 2), n(), n(), n(), n()],
        carriers(0, 2, 3, 4, 5, 6),
    ),
    alg(
        30,
        [e(1, 3), n(), n(), n(), n()],
        carriers(0, 2, 3, 4, 5, 6),
    ),
    alg(
        31,
        [e(1, 4), n(), n(), n(), n()],
        carriers(0, 2, 3, 4, 5, 6),
    ),
    alg(32, [n(), n(), n(), n(), n()], carriers(1, 2, 3, 4, 5, 6)),
];

/// Returns the topology for the given algorithm number, if it exists.
pub fn dx7_algorithm_topology(id: u8) -> Option<&'static Dx7AlgorithmTopology> {
    DX7_ALGORITHM_TOPOLOGIES
        .iter()
        .find(|topology| topology.id == id)
}

/// Clamps a raw patch algorithm byte into the valid 1..=32 range.
pub fn dx7_patch_algorithm_id(raw_algorithm: u8) -> u8 {
    raw_algorithm.clamp(1, DX7_ALGORITHM_COUNT as u8)
}

/// Returns the topology for a raw patch algorithm byte, clamping it into range.
pub fn dx7_algorithm_topology_for_patch(raw_algorithm: u8) -> &'static Dx7AlgorithmTopology {
    dx7_algorithm_topology(dx7_patch_algorithm_id(raw_algorithm))
        .expect("clamped DX7 algorithm id is present")
}

const fn alg(
    id: u8,
    modulation_edges: [Option<Dx7AlgorithmEdge>; 5],
    carrier_outputs: [Option<Dx7CarrierOutput>; DX7_OPERATOR_COUNT],
) -> Dx7AlgorithmTopology {
    Dx7AlgorithmTopology {
        id,
        operator_order: [1, 2, 3, 4, 5, 6],
        modulation_edges,
        feedback_edge: Some(Dx7AlgorithmEdge {
            from_operator: 1,
            to_operator: 1,
            gain_point: 1,
        }),
        carrier_outputs,
        gain_points: gains(),
    }
}

const fn e(from_operator: u8, to_operator: u8) -> Option<Dx7AlgorithmEdge> {
    Some(Dx7AlgorithmEdge {
        from_operator,
        to_operator,
        gain_point: from_operator,
    })
}

const fn n() -> Option<Dx7AlgorithmEdge> {
    None
}

const fn carriers(
    op1: u8,
    op2: u8,
    op3: u8,
    op4: u8,
    op5: u8,
    op6: u8,
) -> [Option<Dx7CarrierOutput>; DX7_OPERATOR_COUNT] {
    [
        carrier(op1),
        carrier(op2),
        carrier(op3),
        carrier(op4),
        carrier(op5),
        carrier(op6),
    ]
}

const fn carrier(operator: u8) -> Option<Dx7CarrierOutput> {
    if operator == 0 {
        None
    } else {
        Some(Dx7CarrierOutput {
            operator,
            gain_point: operator,
            gain_raw: DX7_GAIN_UNITY,
        })
    }
}

const fn gains() -> [Dx7AlgorithmGainPoint; DX7_OPERATOR_COUNT] {
    [gain(1), gain(2), gain(3), gain(4), gain(5), gain(6)]
}

const fn gain(operator: u8) -> Dx7AlgorithmGainPoint {
    Dx7AlgorithmGainPoint {
        operator,
        gain_raw: DX7_GAIN_UNITY,
    }
}

fn operator_node(operator: u8) -> String {
    format!("op{operator}")
}

fn carrier_input(operator: u8) -> String {
    format!("in{operator}")
}

fn operator_kind(backend: ComponentBackend) -> Symbol {
    match backend {
        ComponentBackend::Algorithmic => dx7_operator_component_id(),
        ComponentBackend::Modeled => dx7_modeled_operator_component_id(),
    }
}

fn number_u8(value: u8) -> Expr {
    number_i64(i64::from(value))
}

fn number_u16(value: u16) -> Expr {
    number_i64(i64::from(value))
}

fn number_i64(value: i64) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}
