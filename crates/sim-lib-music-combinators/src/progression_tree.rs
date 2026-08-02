use std::{collections::BTreeMap, fmt::Debug};

use sim_lib_discrete_rank::BoundedIntVectorSpace;
use sim_lib_discrete_search::{
    NeverInterrupt, SearchControl, SearchInterrupt, SearchOrder, SearchProblem, SearchRun,
    SearchStep, solve,
};
use sim_lib_rank::Nat;
use thiserror::Error;

/// A node in a finite ordered progression tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressionTree<T> {
    /// Catalog value at this node.
    pub value: T,
    /// Ordered child progressions.
    pub children: Vec<ProgressionTree<T>>,
}

impl<T> ProgressionTree<T> {
    /// Builds a tree node from a value and ordered children.
    pub fn new(value: T, children: Vec<Self>) -> Self {
        Self { value, children }
    }

    /// Returns the number of nodes in this tree.
    pub fn node_count(&self) -> usize {
        1 + self.children.iter().map(Self::node_count).sum::<usize>()
    }
}

/// Fixed finite tree topology encoded as preorder child counts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressionTreeTopology {
    child_counts: Vec<usize>,
}

impl ProgressionTreeTopology {
    /// Builds a topology and rejects empty, incomplete, or trailing preorder data.
    pub fn new(child_counts: Vec<usize>) -> Result<Self, ProgressionTreeError> {
        if child_counts.is_empty() {
            return Err(ProgressionTreeError::EmptyTopology);
        }
        if child_counts.len() > 127 {
            return Err(ProgressionTreeError::TopologyTooLarge {
                nodes: child_counts.len(),
            });
        }
        let mut open_slots = 1usize;
        for (index, children) in child_counts.iter().copied().enumerate() {
            if open_slots == 0 {
                return Err(ProgressionTreeError::TrailingTopologyNode { index });
            }
            open_slots = open_slots - 1 + children;
        }
        if open_slots != 0 {
            return Err(ProgressionTreeError::IncompleteTopology {
                missing: open_slots,
            });
        }
        Ok(Self { child_counts })
    }

    /// Returns the preorder child counts.
    pub fn child_counts(&self) -> &[usize] {
        &self.child_counts
    }

    /// Returns the fixed number of tree nodes.
    pub fn node_count(&self) -> usize {
        self.child_counts.len()
    }
}

/// A progression tree paired with its stable shared-rank ordinal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankedProgressionTree<T> {
    /// Mixed-radix ordinal for the tree's preorder catalog labels.
    pub rank: Nat,
    /// Materialized tree.
    pub tree: ProgressionTree<T>,
}

/// Prefix and completed-tree predicate used by bounded tree exploration.
pub trait ProgressionTreeFilter<T> {
    /// Returns whether a preorder value prefix may still produce a result.
    fn allows_prefix(&self, prefix: &[T], topology: &ProgressionTreeTopology) -> bool;

    /// Returns whether a complete tree should be emitted.
    fn allows_tree(&self, _tree: &ProgressionTree<T>) -> bool {
        true
    }
}

/// Filter admitting every tree in a finite catalog.
#[derive(Copy, Clone, Debug, Default)]
pub struct AllProgressionTrees;

impl<T> ProgressionTreeFilter<T> for AllProgressionTrees {
    fn allows_prefix(&self, _prefix: &[T], _topology: &ProgressionTreeTopology) -> bool {
        true
    }
}

/// A finite label catalog and fixed topology for rankable progression trees.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressionTreeCatalog<T> {
    values: Vec<T>,
    value_indices: BTreeMap<T, u64>,
    topology: ProgressionTreeTopology,
    rank_space: BoundedIntVectorSpace,
}

impl<T: Ord + Clone> ProgressionTreeCatalog<T> {
    /// Builds a catalog, rejecting empty or duplicate value sets.
    pub fn new(
        values: Vec<T>,
        topology: ProgressionTreeTopology,
    ) -> Result<Self, ProgressionTreeError> {
        if values.is_empty() {
            return Err(ProgressionTreeError::EmptyCatalog);
        }
        let radix =
            u64::try_from(values.len()).map_err(|_| ProgressionTreeError::CatalogTooLarge)?;
        let mut value_indices = BTreeMap::new();
        for (index, value) in values.iter().cloned().enumerate() {
            let ordinal =
                u64::try_from(index).map_err(|_| ProgressionTreeError::CatalogTooLarge)?;
            if value_indices.insert(value, ordinal).is_some() {
                return Err(ProgressionTreeError::DuplicateCatalogValue);
            }
        }
        let rank_space = BoundedIntVectorSpace::try_new(vec![radix; topology.node_count()])
            .map_err(|error| ProgressionTreeError::Rank(error.to_string()))?;
        Ok(Self {
            values,
            value_indices,
            topology,
            rank_space,
        })
    }

    /// Returns the fixed tree topology.
    pub fn topology(&self) -> &ProgressionTreeTopology {
        &self.topology
    }

    /// Returns catalog values in stable digit order.
    pub fn values(&self) -> &[T] {
        &self.values
    }

    /// Ranks one exactly shaped tree through the shared mixed-radix rank space.
    pub fn rank(&self, tree: &ProgressionTree<T>) -> Result<Nat, ProgressionTreeError> {
        let mut values = Vec::with_capacity(self.topology.node_count());
        let mut child_counts = Vec::with_capacity(self.topology.node_count());
        flatten_tree(tree, &mut values, &mut child_counts);
        if child_counts != self.topology.child_counts {
            return Err(ProgressionTreeError::TopologyMismatch);
        }
        let digits = values
            .into_iter()
            .map(|value| {
                self.value_indices
                    .get(value)
                    .copied()
                    .ok_or(ProgressionTreeError::ValueOutsideCatalog)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.rank_space
            .rank(&digits)
            .map_err(|error| ProgressionTreeError::Rank(error.to_string()))
    }

    /// Unranks one ordinal through the shared mixed-radix rank space.
    pub fn unrank(&self, rank: &Nat) -> Result<ProgressionTree<T>, ProgressionTreeError> {
        let digits = self
            .rank_space
            .unrank(rank)
            .map_err(|error| ProgressionTreeError::Rank(error.to_string()))?;
        self.tree_from_digits(&digits)
    }

    /// Explores filtered trees with the shared deterministic search engine.
    pub fn explore<F>(
        &self,
        control: SearchControl,
        filter: &F,
    ) -> Result<SearchRun<RankedProgressionTree<T>>, ProgressionTreeError>
    where
        T: Debug,
        F: ProgressionTreeFilter<T>,
    {
        self.explore_with_interrupt(control, filter, &NeverInterrupt)
    }

    /// Explores filtered trees while polling a caller-owned interrupt.
    pub fn explore_with_interrupt<F>(
        &self,
        control: SearchControl,
        filter: &F,
        interrupt: &dyn SearchInterrupt,
    ) -> Result<SearchRun<RankedProgressionTree<T>>, ProgressionTreeError>
    where
        T: Debug,
        F: ProgressionTreeFilter<T>,
    {
        validate_search_control(&control)?;
        let problem = TreeSearchProblem {
            catalog: self,
            filter,
        };
        Ok(solve(&problem, control, interrupt))
    }

    fn tree_from_digits(&self, digits: &[u64]) -> Result<ProgressionTree<T>, ProgressionTreeError> {
        if digits.len() != self.topology.node_count() {
            return Err(ProgressionTreeError::TopologyMismatch);
        }
        let mut cursor = 0;
        let tree = build_tree(self, digits, &mut cursor)?;
        if cursor != digits.len() {
            return Err(ProgressionTreeError::TopologyMismatch);
        }
        Ok(tree)
    }
}

/// Invalid progression-tree catalog, topology, value, or ordinal.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ProgressionTreeError {
    /// A tree topology must contain one root.
    #[error("progression-tree topology is empty")]
    EmptyTopology,
    /// Topologies share the discrete rank adapter's finite-vector limit.
    #[error("progression-tree topology has {nodes} nodes, maximum is 127")]
    TopologyTooLarge {
        /// Supplied node count.
        nodes: usize,
    },
    /// Preorder data continued after the root tree was complete.
    #[error("progression-tree topology has a trailing node at index {index}")]
    TrailingTopologyNode {
        /// First trailing index.
        index: usize,
    },
    /// Preorder data ended before all child slots were filled.
    #[error("progression-tree topology is missing {missing} nodes")]
    IncompleteTopology {
        /// Unfilled child slots.
        missing: usize,
    },
    /// A catalog needs at least one possible node value.
    #[error("progression-tree value catalog is empty")]
    EmptyCatalog,
    /// A catalog cannot be represented by the shared u64-radix adapter.
    #[error("progression-tree value catalog is too large")]
    CatalogTooLarge,
    /// Every catalog value must have one stable digit.
    #[error("progression-tree value catalog contains a duplicate")]
    DuplicateCatalogValue,
    /// A tree's child counts differ from the catalog topology.
    #[error("progression tree does not match its catalog topology")]
    TopologyMismatch,
    /// A tree contains a value absent from the catalog.
    #[error("progression tree contains a value outside its catalog")]
    ValueOutsideCatalog,
    /// The shared discrete rank adapter rejected an ordinal or digit vector.
    #[error("progression-tree rank error: {0}")]
    Rank(String),
    /// Search controls must bound work, results, frontier, and memory.
    #[error("progression-tree search control is unbounded: {field}")]
    UnboundedSearch {
        /// Missing bound.
        field: &'static str,
    },
    /// A search bound was explicitly zero.
    #[error("progression-tree search control bound {field} must be positive")]
    ZeroSearchBound {
        /// Invalid bound.
        field: &'static str,
    },
    /// Wall-clock stopping is not reproducible.
    #[error("progression-tree search rejects wall-clock limits; use charged work bounds")]
    WallClockLimit,
    /// A beam search must retain at least one node.
    #[error("progression-tree beam width must be positive")]
    ZeroBeamWidth,
    /// Every work charge must be positive.
    #[error("progression-tree search work costs must be positive")]
    ZeroWorkCost,
}

struct TreeSearchProblem<'a, T, F> {
    catalog: &'a ProgressionTreeCatalog<T>,
    filter: &'a F,
}

impl<T, F> SearchProblem for TreeSearchProblem<'_, T, F>
where
    T: Ord + Clone + Debug,
    F: ProgressionTreeFilter<T>,
{
    type State = Vec<u64>;
    type Choice = u64;
    type Output = RankedProgressionTree<T>;

    fn initial_state(&self) -> Self::State {
        Vec::new()
    }

    fn expand(&self, state: &Self::State, out: &mut Vec<Self::Choice>) {
        if state.len() < self.catalog.topology.node_count() {
            out.extend(0..self.catalog.values.len() as u64);
        }
    }

    fn apply(&self, state: &Self::State, choice: &Self::Choice) -> SearchStep<Self::State> {
        let Some(_) = self.catalog.values.get(*choice as usize) else {
            return SearchStep::infeasible("tree choice is outside value catalog");
        };
        let mut next = state.clone();
        next.push(*choice);
        let prefix = next
            .iter()
            .map(|digit| self.catalog.values[*digit as usize].clone())
            .collect::<Vec<_>>();
        if self.filter.allows_prefix(&prefix, &self.catalog.topology) {
            SearchStep::Continue(next)
        } else {
            SearchStep::pruned("progression-tree prefix filter rejected candidate")
        }
    }

    fn finish(&self, state: &Self::State) -> Option<Self::Output> {
        if state.len() != self.catalog.topology.node_count() {
            return None;
        }
        let tree = self.catalog.tree_from_digits(state).ok()?;
        if !self.filter.allows_tree(&tree) {
            return None;
        }
        let rank = self.catalog.rank_space.rank(state).ok()?;
        Some(RankedProgressionTree { rank, tree })
    }
}

fn flatten_tree<'a, T>(
    tree: &'a ProgressionTree<T>,
    values: &mut Vec<&'a T>,
    child_counts: &mut Vec<usize>,
) {
    values.push(&tree.value);
    child_counts.push(tree.children.len());
    for child in &tree.children {
        flatten_tree(child, values, child_counts);
    }
}

fn build_tree<T: Ord + Clone>(
    catalog: &ProgressionTreeCatalog<T>,
    digits: &[u64],
    cursor: &mut usize,
) -> Result<ProgressionTree<T>, ProgressionTreeError> {
    let index = *cursor;
    let digit = *digits
        .get(index)
        .ok_or(ProgressionTreeError::TopologyMismatch)?;
    let value = catalog
        .values
        .get(digit as usize)
        .cloned()
        .ok_or(ProgressionTreeError::ValueOutsideCatalog)?;
    let child_count = catalog.topology.child_counts[index];
    *cursor += 1;
    let mut children = Vec::with_capacity(child_count);
    for _ in 0..child_count {
        children.push(build_tree(catalog, digits, cursor)?);
    }
    Ok(ProgressionTree { value, children })
}

fn validate_search_control(control: &SearchControl) -> Result<(), ProgressionTreeError> {
    for (field, value) in [
        ("max_work", control.max_work),
        (
            "max_results",
            control
                .max_results
                .and_then(|value| u64::try_from(value).ok()),
        ),
        (
            "max_frontier",
            control
                .max_frontier
                .and_then(|value| u64::try_from(value).ok()),
        ),
        (
            "max_memory_nodes",
            control
                .max_memory_nodes
                .and_then(|value| u64::try_from(value).ok()),
        ),
    ] {
        match value {
            None => return Err(ProgressionTreeError::UnboundedSearch { field }),
            Some(0) => return Err(ProgressionTreeError::ZeroSearchBound { field }),
            Some(_) => {}
        }
    }
    if control.max_time.is_some() {
        return Err(ProgressionTreeError::WallClockLimit);
    }
    if matches!(control.order, SearchOrder::Beam { width: 0 }) {
        return Err(ProgressionTreeError::ZeroBeamWidth);
    }
    if control.costs.expand == 0
        || control.costs.score == 0
        || control.costs.propagate == 0
        || control.costs.emit == 0
    {
        return Err(ProgressionTreeError::ZeroWorkCost);
    }
    Ok(())
}
