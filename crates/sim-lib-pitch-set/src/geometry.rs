//! Pitch-set geometry and graph neighborhoods.

use sim_lib_discrete_graph::{Directedness, Graph, GraphError};
use sim_lib_discrete_search::SearchControl;
use sim_lib_pitch_core::PitchClass;

use crate::PitchClassMask;

/// How zero-sized gaps are represented.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ZeroGapPolicy {
    /// Collapse duplicate pitch classes; zero gaps never appear.
    CollapseDuplicates,
    /// Preserve adjacent duplicate pitch classes as explicit zero gaps.
    PreserveMultiplicity,
}

/// Directed interval steps between adjacent pitch classes in a cyclic ordering.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GapForm {
    /// Adjacent cyclic semitone gaps. The sum is `12` for duplicate-free
    /// pitch-class forms and may include zero entries when multiplicity is
    /// preserved.
    pub gaps: Vec<u8>,
    /// The policy used to decide whether adjacent equal pitch classes produce a
    /// zero gap.
    pub zero_gap_policy: ZeroGapPolicy,
}

/// Pairwise directed pitch-class intervals.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntervalForm {
    /// Ordered pitch-class pairs and directed semitone distances from the first
    /// pitch class to the second.
    pub intervals: Vec<(PitchClass, PitchClass, u8)>,
}

/// Pitch-class universe used by a neighborhood graph.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PitchSetSpace {
    /// Required set cardinality for every graph node.
    pub cardinality: u8,
}

impl PitchSetSpace {
    /// The complete twelve-class universe at a fixed cardinality.
    pub fn chromatic(cardinality: u8) -> Self {
        Self { cardinality }
    }
}

/// Allowed move family between pitch-class sets.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PitchSetMovePolicy {
    /// Any one pitch class may be replaced by any absent pitch class.
    Jumping,
    /// A pitch class may move by one semitone into an unoccupied neighbor.
    NonJumping,
}

/// Graph builder for pitch-set neighborhoods.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PitchSetNeighborhood {
    /// Node universe.
    pub space: PitchSetSpace,
    /// Edge policy.
    pub move_policy: PitchSetMovePolicy,
}

/// Error raised while building a pitch-set graph.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PitchSetGraphError {
    /// Cardinality must fit the twelve pitch classes.
    #[error("pitch-set cardinality {0} is outside 0..=12")]
    InvalidCardinality(u8),
    /// SearchControl stopped enumeration before the graph was complete.
    #[error("pitch-set graph enumeration exceeded {limit}: used {used}")]
    SearchLimit {
        /// Limit field that stopped the run.
        limit: &'static str,
        /// Charged amount at the stop.
        used: u64,
    },
    /// A discrete graph construction error occurred.
    #[error(transparent)]
    Graph(#[from] GraphError),
}

impl PitchClassMask {
    /// Return adjacent cyclic gaps for this set's pitch classes.
    ///
    /// `PitchClassMask` has set identity, so duplicate pitch classes are already
    /// collapsed and zero gaps are absent. Use [`gap_form_from_pitch_classes`]
    /// when caller-owned multiplicity must be preserved.
    pub fn gap_form(self) -> GapForm {
        gap_form_from_pitch_classes(&self.pitch_classes(), ZeroGapPolicy::CollapseDuplicates)
    }

    /// Return directed pairwise intervals between all pitch classes in the set.
    pub fn interval_form(self) -> IntervalForm {
        interval_form_from_pitch_classes(&self.pitch_classes())
    }
}

/// Return adjacent cyclic gaps for caller-owned pitch-class material.
pub fn gap_form_from_pitch_classes(
    pitch_classes: &[PitchClass],
    zero_gap_policy: ZeroGapPolicy,
) -> GapForm {
    let mut values: Vec<u8> = pitch_classes
        .iter()
        .map(|pitch_class| pitch_class.value())
        .collect();
    values.sort_unstable();
    if matches!(zero_gap_policy, ZeroGapPolicy::CollapseDuplicates) {
        values.dedup();
    }
    let gaps = match values.len() {
        0 => Vec::new(),
        1 => vec![0],
        len => values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let next = values[(index + 1) % len];
                (i16::from(next) - i16::from(*value)).rem_euclid(12) as u8
            })
            .collect(),
    };
    GapForm {
        gaps,
        zero_gap_policy,
    }
}

/// Return directed pairwise intervals for caller-owned pitch-class material.
pub fn interval_form_from_pitch_classes(pitch_classes: &[PitchClass]) -> IntervalForm {
    let mut values: Vec<PitchClass> = pitch_classes.to_vec();
    values.sort_by_key(|pitch_class| pitch_class.value());
    values.dedup();
    let mut intervals = Vec::new();
    for (index, from) in values.iter().enumerate() {
        for to in values.iter().skip(index + 1) {
            intervals.push((*from, *to, to.value().wrapping_sub(from.value()) % 12));
        }
    }
    IntervalForm { intervals }
}

impl PitchSetNeighborhood {
    /// Create a pitch-set neighborhood graph builder.
    pub fn new(space: PitchSetSpace, move_policy: PitchSetMovePolicy) -> Self {
        Self { space, move_policy }
    }

    /// Materialize the neighborhood as an undirected graph.
    ///
    /// Open enumeration is charged against `SearchControl::max_work` and
    /// `SearchControl::max_results`; every accepted node and edge consumes one
    /// unit of work.
    pub fn materialize(
        self,
        control: SearchControl,
    ) -> Result<Graph<PitchClassMask, i64>, PitchSetGraphError> {
        if self.space.cardinality > 12 {
            return Err(PitchSetGraphError::InvalidCardinality(
                self.space.cardinality,
            ));
        }
        let nodes = enumerate_masks(self.space.cardinality, &control)?;
        let mut graph = Graph::with_nodes(nodes, Directedness::Undirected);
        let mut work = graph.node_count() as u64;
        for source in 0..graph.nodes.len() {
            for target in (source + 1)..graph.nodes.len() {
                if are_neighbors(graph.nodes[source], graph.nodes[target], self.move_policy) {
                    work = work.checked_add(1).ok_or(PitchSetGraphError::SearchLimit {
                        limit: "max_work",
                        used: u64::MAX,
                    })?;
                    if let Some(max_work) = control.max_work
                        && work > max_work
                    {
                        return Err(PitchSetGraphError::SearchLimit {
                            limit: "max_work",
                            used: work,
                        });
                    }
                    graph.add_edge(source, target, 1)?;
                }
            }
        }
        Ok(graph)
    }
}

fn enumerate_masks(
    cardinality: u8,
    control: &SearchControl,
) -> Result<Vec<PitchClassMask>, PitchSetGraphError> {
    let mut masks = Vec::new();
    let mut work = 0u64;
    for bits in 0u16..=0x0fff {
        if bits.count_ones() == u32::from(cardinality) {
            work = work.checked_add(1).ok_or(PitchSetGraphError::SearchLimit {
                limit: "max_work",
                used: u64::MAX,
            })?;
            if let Some(max_work) = control.max_work
                && work > max_work
            {
                return Err(PitchSetGraphError::SearchLimit {
                    limit: "max_work",
                    used: work,
                });
            }
            if let Some(max_results) = control.max_results
                && masks.len() >= max_results
            {
                return Err(PitchSetGraphError::SearchLimit {
                    limit: "max_results",
                    used: masks.len() as u64,
                });
            }
            masks.push(PitchClassMask::new(bits).expect("enumeration yields valid mask bits"));
        }
    }
    Ok(masks)
}

fn are_neighbors(a: PitchClassMask, b: PitchClassMask, policy: PitchSetMovePolicy) -> bool {
    let removed = a.bits() & !b.bits();
    let added = b.bits() & !a.bits();
    if removed.count_ones() != 1 || added.count_ones() != 1 {
        return false;
    }
    match policy {
        PitchSetMovePolicy::Jumping => true,
        PitchSetMovePolicy::NonJumping => {
            let from = removed.trailing_zeros() as i32;
            let to = added.trailing_zeros() as i32;
            matches!((to - from).rem_euclid(12), 1 | 11)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_lib_discrete_graph::{shortest_path, verify_shortest_paths};
    use sim_lib_discrete_search::SearchControl;

    fn mask(values: &[PitchClass]) -> PitchClassMask {
        PitchClassMask::from_pitch_classes(values)
    }

    #[test]
    fn gap_forms_make_zero_gap_multiplicity_explicit() {
        let source = [PitchClass::C, PitchClass::C, PitchClass::E, PitchClass::G];

        assert_eq!(
            gap_form_from_pitch_classes(&source, ZeroGapPolicy::CollapseDuplicates).gaps,
            vec![4, 3, 5]
        );
        assert_eq!(
            gap_form_from_pitch_classes(&source, ZeroGapPolicy::PreserveMultiplicity).gaps,
            vec![0, 4, 3, 5]
        );
        assert_eq!(mask(&source).interval_vector().0, [0, 0, 1, 1, 1, 0]);
    }

    #[test]
    fn interval_and_gap_forms_are_transposition_invariant() {
        let source = mask(&[PitchClass::C, PitchClass::DS, PitchClass::FS, PitchClass::A]);
        let transposed = source.rotate(5);

        assert_eq!(source.gap_form(), transposed.gap_form());
        assert_eq!(source.interval_vector(), transposed.interval_vector());
    }

    #[test]
    fn interval_and_gap_forms_are_inversion_invariant_after_reordering() {
        let source = mask(&[PitchClass::C, PitchClass::D, PitchClass::F, PitchClass::A]);
        let inverted = source.invert(PitchClass::C);

        let mut source_gaps = source.gap_form().gaps;
        let mut inverted_gaps = inverted.gap_form().gaps;
        source_gaps.sort_unstable();
        inverted_gaps.sort_unstable();
        assert_eq!(source_gaps, inverted_gaps);
        assert_eq!(source.interval_vector(), inverted.interval_vector());
    }

    #[test]
    fn jumping_neighborhood_materializes_reversible_graph() {
        let graph =
            PitchSetNeighborhood::new(PitchSetSpace::chromatic(2), PitchSetMovePolicy::Jumping)
                .materialize(SearchControl::default())
                .unwrap();
        let start = graph
            .nodes
            .iter()
            .position(|node| *node == mask(&[PitchClass::C, PitchClass::E]))
            .unwrap();
        let goal = graph
            .nodes
            .iter()
            .position(|node| *node == mask(&[PitchClass::D, PitchClass::F]))
            .unwrap();

        let round = shortest_path(&graph, start, goal).unwrap();

        assert_eq!(round.distance, Some(2));
        verify_shortest_paths(&graph, &round.certificate).unwrap();
        for edge in &graph.edges {
            assert!(
                graph
                    .neighbors(edge.target)
                    .unwrap()
                    .iter()
                    .any(|neighbor| {
                        neighbor.node == edge.source && *neighbor.weight == edge.weight
                    })
            );
        }
    }

    #[test]
    fn non_jumping_neighborhood_uses_single_step_edges() {
        let graph =
            PitchSetNeighborhood::new(PitchSetSpace::chromatic(1), PitchSetMovePolicy::NonJumping)
                .materialize(SearchControl::default())
                .unwrap();
        let start = graph
            .nodes
            .iter()
            .position(|node| node.bits() == 0b1)
            .unwrap();
        let goal = graph
            .nodes
            .iter()
            .position(|node| node.bits() == 0b100)
            .unwrap();

        let trail = shortest_path(&graph, start, goal).unwrap();

        assert_eq!(trail.distance, Some(2));
        verify_shortest_paths(&graph, &trail.certificate).unwrap();
    }

    #[test]
    fn search_control_charges_open_enumeration() {
        let err =
            PitchSetNeighborhood::new(PitchSetSpace::chromatic(3), PitchSetMovePolicy::Jumping)
                .materialize(SearchControl::default().with_max_results(4))
                .unwrap_err();

        assert_eq!(
            err,
            PitchSetGraphError::SearchLimit {
                limit: "max_results",
                used: 4,
            }
        );
    }
}
