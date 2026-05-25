//! Multi-View DAG with Kahn's topological sort, diamond detection, and frontier coordination.
//!
//! Foundation for views that read from other materialized views. Guarantees upstream
//! views are fully refreshed before any downstream consumer reads their delta.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

/// Unique identifier for a materialized view in the DAG.
pub type ViewId = u64;

/// Unique identifier for a source (base table or upstream matview).
pub type SourceId = u64;

/// A directed edge from an upstream view to a downstream view.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DagEdge {
    pub upstream: ViewId,
    pub downstream: ViewId,
}

/// The view dependency DAG.
#[derive(Debug, Clone, Default)]
pub struct ViewDag {
    /// Adjacency list: view → set of downstream dependents.
    pub edges: HashMap<ViewId, HashSet<ViewId>>,
    /// Reverse adjacency: view → set of upstream dependencies.
    pub reverse_edges: HashMap<ViewId, HashSet<ViewId>>,
    /// All known views.
    pub views: HashSet<ViewId>,
}

/// A node identified as a diamond apex — reachable from the same root via 2+ paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiamondApex {
    pub view_id: ViewId,
    pub root: ViewId,
    pub paths: usize,
}

/// Frontier vector clock: per-source sequence tracking.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontierClock {
    /// source_id → last processed sequence number.
    pub clocks: BTreeMap<SourceId, u64>,
}

/// Consistency policy for diamond apex nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiamondPolicy {
    /// Wait until ALL upstream views have frontier ≥ F.
    Slowest,
}

impl ViewDag {
    /// Create an empty DAG.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a view to the DAG (may have no edges yet).
    pub fn add_view(&mut self, view_id: ViewId) {
        self.views.insert(view_id);
        self.edges.entry(view_id).or_default();
        self.reverse_edges.entry(view_id).or_default();
    }

    /// Add a dependency edge: `downstream` depends on `upstream`.
    pub fn add_edge(&mut self, upstream: ViewId, downstream: ViewId) {
        self.views.insert(upstream);
        self.views.insert(downstream);
        self.edges.entry(upstream).or_default().insert(downstream);
        self.reverse_edges
            .entry(downstream)
            .or_default()
            .insert(upstream);
        self.edges.entry(downstream).or_default();
        self.reverse_edges.entry(upstream).or_default();
    }

    /// Remove a view from the DAG. Returns an error naming dependent views if any exist.
    pub fn remove_view(&mut self, view_id: ViewId) -> Result<(), Vec<ViewId>> {
        // Check for downstream dependents.
        if let Some(dependents) = self.edges.get(&view_id) {
            if !dependents.is_empty() {
                return Err(dependents.iter().copied().collect());
            }
        }
        // Remove outgoing edges.
        self.edges.remove(&view_id);
        // Remove incoming edges (from upstream views' adjacency lists).
        if let Some(upstreams) = self.reverse_edges.remove(&view_id) {
            for u in upstreams {
                if let Some(downs) = self.edges.get_mut(&u) {
                    downs.remove(&view_id);
                }
            }
        }
        self.views.remove(&view_id);
        Ok(())
    }

    /// Kahn's topological sort. Returns an ordered vec of view IDs such that
    /// all dependencies precede their dependents. O(V+E).
    pub fn topological_sort(&self) -> Vec<ViewId> {
        let mut in_degree: HashMap<ViewId, usize> = HashMap::new();
        for &v in &self.views {
            in_degree.insert(v, 0);
        }
        for downs in self.edges.values() {
            for &d in downs {
                *in_degree.entry(d).or_default() += 1;
            }
        }

        let mut queue: VecDeque<ViewId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&v, _)| v)
            .collect();

        // Sort the initial queue for determinism.
        let mut sorted_start: Vec<ViewId> = queue.drain(..).collect();
        sorted_start.sort();
        queue.extend(sorted_start);

        let mut result = Vec::with_capacity(self.views.len());
        while let Some(node) = queue.pop_front() {
            result.push(node);
            if let Some(downs) = self.edges.get(&node) {
                let mut sorted_downs: Vec<ViewId> = downs.iter().copied().collect();
                sorted_downs.sort();
                for d in sorted_downs {
                    if let Some(deg) = in_degree.get_mut(&d) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(d);
                        }
                    }
                }
            }
        }
        result
    }

    /// Detect diamond apexes: nodes reachable from the same root via 2+ paths.
    /// O(V+E) per root, O(V*(V+E)) worst case.
    pub fn detect_diamonds(&self) -> Vec<DiamondApex> {
        let roots: Vec<ViewId> = self
            .views
            .iter()
            .copied()
            .filter(|v| {
                self.reverse_edges
                    .get(v)
                    .map(|s| s.is_empty())
                    .unwrap_or(true)
            })
            .collect();

        let mut diamonds = Vec::new();

        for root in &roots {
            // BFS from this root; count how many paths reach each node.
            let mut path_count: HashMap<ViewId, usize> = HashMap::new();
            let mut visited_order: Vec<ViewId> = Vec::new();

            // Use topo-order BFS to accumulate path counts.
            let topo = self.topological_sort();
            path_count.insert(*root, 1);

            for &node in &topo {
                let count = path_count.get(&node).copied().unwrap_or(0);
                if count == 0 {
                    continue;
                }
                if let Some(downs) = self.edges.get(&node) {
                    for &d in downs {
                        *path_count.entry(d).or_default() += count;
                    }
                }
                visited_order.push(node);
            }

            for (&view_id, &paths) in &path_count {
                if paths > 1 && view_id != *root {
                    diamonds.push(DiamondApex {
                        view_id,
                        root: *root,
                        paths,
                    });
                }
            }
        }

        diamonds.sort_by_key(|d| (d.root, d.view_id));
        diamonds
    }

    /// Check if a view is a diamond apex (convergence point).
    pub fn is_diamond_apex(&self, view_id: ViewId) -> bool {
        self.detect_diamonds().iter().any(|d| d.view_id == view_id)
    }

    /// Get all upstream dependencies for a view (transitive closure).
    pub fn all_upstreams(&self, view_id: ViewId) -> HashSet<ViewId> {
        let mut result = HashSet::new();
        let mut queue = VecDeque::new();
        if let Some(upstreams) = self.reverse_edges.get(&view_id) {
            for &u in upstreams {
                queue.push_back(u);
            }
        }
        while let Some(node) = queue.pop_front() {
            if result.insert(node) {
                if let Some(upstreams) = self.reverse_edges.get(&node) {
                    for &u in upstreams {
                        queue.push_back(u);
                    }
                }
            }
        }
        result
    }
}

impl FrontierClock {
    /// Create a new empty frontier.
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the frontier for a source.
    pub fn advance(&mut self, source_id: SourceId, seq: u64) {
        let entry = self.clocks.entry(source_id).or_default();
        if seq > *entry {
            *entry = seq;
        }
    }

    /// Get the frontier for a source. Returns 0 if not yet tracked.
    pub fn get(&self, source_id: SourceId) -> u64 {
        self.clocks.get(&source_id).copied().unwrap_or(0)
    }

    /// Check if this frontier is at or beyond the target for all sources in `target`.
    pub fn is_at_or_beyond(&self, target: &FrontierClock) -> bool {
        for (&source_id, &target_seq) in &target.clocks {
            if self.get(source_id) < target_seq {
                return false;
            }
        }
        true
    }

    /// Minimum frontier across all tracked sources.
    pub fn min_frontier(&self) -> u64 {
        self.clocks.values().copied().min().unwrap_or(0)
    }

    /// Serialize to JSON bytes for durable persistence.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserialize from JSON bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

/// Check if a diamond apex view can proceed with refresh under Slowest policy.
/// All upstream views must have frontier ≥ the required frontier.
pub fn can_refresh_diamond(
    apex_view: ViewId,
    dag: &ViewDag,
    frontiers: &HashMap<ViewId, FrontierClock>,
    required_frontier: &FrontierClock,
) -> bool {
    let upstreams = dag.all_upstreams(apex_view);
    for upstream_id in &upstreams {
        if let Some(frontier) = frontiers.get(upstream_id) {
            if !frontier.is_at_or_beyond(required_frontier) {
                return false;
            }
        } else {
            // Upstream has no frontier at all — not ready.
            return false;
        }
    }
    true
}

/// Explain a view's dependency graph as a human-readable string.
pub fn explain_dag(dag: &ViewDag, view_id: ViewId) -> String {
    let mut lines = Vec::new();
    lines.push(format!("View {view_id} dependency graph:"));

    let upstreams = dag.all_upstreams(view_id);
    if upstreams.is_empty() {
        lines.push("  No upstream dependencies (root view)".to_string());
    } else {
        let mut sorted: Vec<_> = upstreams.into_iter().collect();
        sorted.sort();
        lines.push(format!("  Upstream: {:?}", sorted));
    }

    let diamonds = dag.detect_diamonds();
    let relevant: Vec<_> = diamonds.iter().filter(|d| d.view_id == view_id).collect();
    if relevant.is_empty() {
        lines.push("  Diamond: none".to_string());
    } else {
        for d in relevant {
            lines.push(format!(
                "  Diamond apex: root={}, paths={}",
                d.root, d.paths
            ));
        }
    }

    let topo = dag.topological_sort();
    lines.push(format!("  Topological order: {:?}", topo));

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_chain_topological_order() {
        let mut dag = ViewDag::new();
        dag.add_edge(1, 2);
        dag.add_edge(2, 3);
        let order = dag.topological_sort();
        assert_eq!(order, vec![1, 2, 3]);
    }

    #[test]
    fn diamond_detection() {
        // A→B, A→C, B→D, C→D
        let mut dag = ViewDag::new();
        dag.add_edge(1, 2); // A→B
        dag.add_edge(1, 3); // A→C
        dag.add_edge(2, 4); // B→D
        dag.add_edge(3, 4); // C→D
        let diamonds = dag.detect_diamonds();
        assert!(diamonds.iter().any(|d| d.view_id == 4 && d.root == 1));
    }

    #[test]
    fn remove_view_with_dependents_fails() {
        let mut dag = ViewDag::new();
        dag.add_edge(1, 2);
        dag.add_edge(2, 3);
        let result = dag.remove_view(2);
        assert!(result.is_err());
        let deps = result.unwrap_err();
        assert_eq!(deps, vec![3]);
    }

    #[test]
    fn frontier_clock_advance_and_check() {
        let mut clock = FrontierClock::new();
        clock.advance(1, 10);
        clock.advance(2, 20);

        let mut target = FrontierClock::new();
        target.advance(1, 10);
        target.advance(2, 15);
        assert!(clock.is_at_or_beyond(&target));

        target.advance(2, 25);
        assert!(!clock.is_at_or_beyond(&target));
    }

    #[test]
    fn diamond_slowest_policy() {
        let mut dag = ViewDag::new();
        dag.add_edge(1, 2); // A→B
        dag.add_edge(1, 3); // A→C
        dag.add_edge(2, 4); // B→D
        dag.add_edge(3, 4); // C→D

        let mut frontiers = HashMap::new();
        let mut b_frontier = FrontierClock::new();
        b_frontier.advance(1, 10);
        frontiers.insert(2, b_frontier);

        let mut c_frontier = FrontierClock::new();
        c_frontier.advance(1, 5); // C is behind
        frontiers.insert(3, c_frontier);

        // A (root) frontier
        let mut a_frontier = FrontierClock::new();
        a_frontier.advance(1, 10);
        frontiers.insert(1, a_frontier);

        let mut required = FrontierClock::new();
        required.advance(1, 10);

        // D should NOT refresh because C's frontier (5) < required (10).
        assert!(!can_refresh_diamond(4, &dag, &frontiers, &required));

        // Advance C to 10.
        frontiers.get_mut(&3).unwrap().advance(1, 10);
        assert!(can_refresh_diamond(4, &dag, &frontiers, &required));
    }
}
