//! Validated precedence graph for immutable serial events.

use std::collections::{BTreeMap, BTreeSet};

use crate::{SerialEventId, SerialPlanError};

/// A validated finite precedence DAG over stable event ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrecedenceGraph<I> {
    edges: BTreeMap<I, BTreeSet<I>>,
}

impl<I: Ord> Default for PrecedenceGraph<I> {
    fn default() -> Self {
        Self {
            edges: BTreeMap::new(),
        }
    }
}

impl PrecedenceGraph<SerialEventId> {
    /// Builds a graph from `before -> after` edges.
    pub fn try_new(
        edges: impl IntoIterator<Item = (SerialEventId, SerialEventId)>,
        known_nodes: &BTreeSet<SerialEventId>,
    ) -> Result<Self, SerialPlanError> {
        let mut graph = Self::default();
        for (before, after) in edges {
            if !known_nodes.contains(&before) {
                return Err(SerialPlanError::UnknownPrecedenceNode(before));
            }
            if !known_nodes.contains(&after) {
                return Err(SerialPlanError::UnknownPrecedenceNode(after));
            }
            if before == after {
                return Err(SerialPlanError::SelfPrecedence(before));
            }
            graph.edges.entry(before).or_default().insert(after);
        }
        graph.validate_acyclic(known_nodes)?;
        Ok(graph)
    }

    /// Returns whether one direct precedence edge exists.
    pub fn contains_edge(&self, before: &SerialEventId, after: &SerialEventId) -> bool {
        self.edges
            .get(before)
            .is_some_and(|targets| targets.contains(after))
    }

    /// Returns the outgoing precedence targets for one event.
    pub fn successors(&self, event_id: &SerialEventId) -> Option<&BTreeSet<SerialEventId>> {
        self.edges.get(event_id)
    }

    /// Returns every direct precedence edge in canonical order.
    pub fn edges(&self) -> impl Iterator<Item = (&SerialEventId, &SerialEventId)> {
        self.edges
            .iter()
            .flat_map(|(before, afters)| afters.iter().map(move |after| (before, after)))
    }

    fn validate_acyclic(
        &self,
        known_nodes: &BTreeSet<SerialEventId>,
    ) -> Result<(), SerialPlanError> {
        #[derive(Copy, Clone, PartialEq, Eq)]
        enum Mark {
            Visiting,
            Done,
        }

        fn visit(
            node: &SerialEventId,
            graph: &PrecedenceGraph<SerialEventId>,
            marks: &mut BTreeMap<SerialEventId, Mark>,
        ) -> Result<(), SerialPlanError> {
            match marks.get(node) {
                Some(Mark::Done) => return Ok(()),
                Some(Mark::Visiting) => return Err(SerialPlanError::PrecedenceCycle(node.clone())),
                None => {}
            }
            marks.insert(node.clone(), Mark::Visiting);
            if let Some(targets) = graph.edges.get(node) {
                for target in targets {
                    visit(target, graph, marks)?;
                }
            }
            marks.insert(node.clone(), Mark::Done);
            Ok(())
        }

        let mut marks = BTreeMap::new();
        for node in known_nodes {
            visit(node, self, &mut marks)?;
        }
        Ok(())
    }
}
