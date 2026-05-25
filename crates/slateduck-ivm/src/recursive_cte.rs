//! Recursive CTE support for IVM.
//!
//! `WITH RECURSIVE` enables transitive closure, hierarchical rollups, and
//! graph reachability. Maps to a fixed-point iteration loop: the base case
//! is the seed; the recursive term is the iteration body; termination is
//! detected when output = input (no new rows produced).
//!
//! ## Cross-shard convergence
//! Unbounded recursive CTEs run on a single coordinator shard that receives a
//! global shuffle of all participating rows. Bounded-depth recursions
//! (`max_depth ≤ D`) may use sharded execution.
//!
//! ## Spike findings
//! DBSP's `iterate` operator computes a local fixed point within its own time
//! domain. SlateDuck's snapshot-frontier model maps cleanly: each snapshot
//! advance triggers one iteration epoch. The fixed point is reached when the
//! delta from the recursive body is empty. The `iterate` operator is callable
//! externally by wrapping the circuit in an iteration scope.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Recursive CTE configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecursiveCteConfig {
    /// Name of the recursive CTE.
    pub cte_name: String,
    /// Maximum iterations before marking view as Stale.
    pub max_iterations: u32,
    /// Whether this is a bounded-depth recursion (e.g., CONNECT BY with max_depth).
    pub bounded_depth: Option<u32>,
    /// Required shard count. Unbounded recursions enforce shard_count = 1.
    pub shard_count: u32,
}

impl Default for RecursiveCteConfig {
    fn default() -> Self {
        Self {
            cte_name: String::new(),
            max_iterations: 100,
            bounded_depth: None,
            shard_count: 1,
        }
    }
}

/// State of the recursive iteration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IterationState {
    /// Not yet started.
    NotStarted,
    /// Currently iterating.
    InProgress { iteration: u32 },
    /// Fixed point reached (output = input).
    Converged { iterations: u32 },
    /// Maximum iterations exceeded — view marked Stale.
    Diverged { iterations: u32 },
}

/// The recursive CTE evaluator.
#[derive(Debug, Clone)]
pub struct RecursiveCteEvaluator {
    pub config: RecursiveCteConfig,
    /// Current iteration state.
    pub state: IterationState,
    /// Current working set of rows (the "frontier" of new rows per iteration).
    pub frontier: HashSet<Vec<u8>>,
    /// All rows accumulated across iterations (the full result set).
    pub accumulated: HashSet<Vec<u8>>,
    /// Iteration count.
    pub iteration_count: u32,
}

impl RecursiveCteEvaluator {
    /// Create a new recursive CTE evaluator.
    pub fn new(config: RecursiveCteConfig) -> Self {
        Self {
            config,
            state: IterationState::NotStarted,
            frontier: HashSet::new(),
            accumulated: HashSet::new(),
            iteration_count: 0,
        }
    }

    /// Initialize with the base case (seed) rows.
    pub fn seed(&mut self, rows: Vec<Vec<u8>>) {
        self.frontier.clear();
        self.accumulated.clear();
        for row in rows {
            self.accumulated.insert(row.clone());
            self.frontier.insert(row);
        }
        self.state = IterationState::InProgress { iteration: 0 };
        self.iteration_count = 0;
    }

    /// Execute one iteration step. The `apply_fn` produces new rows from the
    /// current frontier. Returns true if new rows were produced (not yet converged).
    pub fn step<F>(&mut self, apply_fn: F) -> bool
    where
        F: FnOnce(&HashSet<Vec<u8>>) -> Vec<Vec<u8>>,
    {
        if matches!(
            self.state,
            IterationState::Converged { .. } | IterationState::Diverged { .. }
        ) {
            return false;
        }

        self.iteration_count += 1;

        // Check max iterations
        if self.iteration_count > self.config.max_iterations {
            self.state = IterationState::Diverged {
                iterations: self.iteration_count,
            };
            return false;
        }

        // Apply the recursive body to the current frontier
        let new_rows = apply_fn(&self.frontier);

        // Filter out rows already in the accumulated set (fixed-point detection)
        let mut new_frontier = HashSet::new();
        for row in new_rows {
            if !self.accumulated.contains(&row) {
                self.accumulated.insert(row.clone());
                new_frontier.insert(row);
            }
        }

        if new_frontier.is_empty() {
            // Fixed point reached
            self.state = IterationState::Converged {
                iterations: self.iteration_count,
            };
            self.frontier.clear();
            false
        } else {
            self.frontier = new_frontier;
            self.state = IterationState::InProgress {
                iteration: self.iteration_count,
            };
            true
        }
    }

    /// Run to completion (up to max_iterations).
    pub fn run_to_completion<F>(&mut self, mut apply_fn: F)
    where
        F: FnMut(&HashSet<Vec<u8>>) -> Vec<Vec<u8>>,
    {
        while self.step(&mut apply_fn) {}
    }

    /// Get the accumulated result set.
    pub fn result(&self) -> &HashSet<Vec<u8>> {
        &self.accumulated
    }

    /// Get the current iteration count.
    pub fn iterations(&self) -> u32 {
        self.iteration_count
    }

    /// Check if the CTE has converged.
    pub fn is_converged(&self) -> bool {
        matches!(self.state, IterationState::Converged { .. })
    }

    /// Check if the CTE has diverged (exceeded max_iterations).
    pub fn is_diverged(&self) -> bool {
        matches!(self.state, IterationState::Diverged { .. })
    }
}

/// Validate recursive CTE configuration.
pub fn validate_recursive_cte(config: &RecursiveCteConfig) -> Result<(), RecursiveCteError> {
    // Unbounded recursions must use shard_count = 1
    if config.bounded_depth.is_none() && config.shard_count > 1 {
        return Err(RecursiveCteError::UnboundedRequiresSingleShard {
            requested_shards: config.shard_count,
        });
    }
    Ok(())
}

/// Detect if a CTE dependency graph contains cycles (indicating recursion).
pub fn detect_recursive_ctes(cte_deps: &[(String, Vec<String>)]) -> Vec<String> {
    let mut recursive = Vec::new();
    for (name, deps) in cte_deps {
        if deps.contains(name) {
            recursive.push(name.clone());
        }
    }
    recursive
}

/// Errors from recursive CTE operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RecursiveCteError {
    #[error(
        "unbounded recursive CTE requires shard_count = 1; requested {requested_shards} shards"
    )]
    UnboundedRequiresSingleShard { requested_shards: u32 },
    #[error("recursive CTE '{cte_name}' exceeded max_iterations ({max})")]
    MaxIterationsExceeded { cte_name: String, max: u32 },
}

/// Incremental update for a recursive CTE: process edge additions/removals.
#[derive(Debug, Clone)]
pub struct RecursiveIncrementalUpdate {
    /// New edges/rows added since last snapshot.
    pub additions: Vec<Vec<u8>>,
    /// Edges/rows removed since last snapshot.
    pub removals: Vec<Vec<u8>>,
}

/// Apply an incremental update to a recursive CTE.
/// Returns the new accumulated result set after re-computing the fixed point.
pub fn apply_incremental_update(
    evaluator: &mut RecursiveCteEvaluator,
    update: &RecursiveIncrementalUpdate,
    apply_fn: impl FnMut(&HashSet<Vec<u8>>) -> Vec<Vec<u8>>,
) -> Result<usize, RecursiveCteError> {
    // For correctness, re-seed with all current edges minus removals plus additions
    let mut base: Vec<Vec<u8>> = evaluator
        .accumulated
        .iter()
        .filter(|r| !update.removals.contains(r))
        .cloned()
        .collect();
    base.extend(update.additions.iter().cloned());

    evaluator.seed(base);
    evaluator.run_to_completion(apply_fn);

    if evaluator.is_diverged() {
        return Err(RecursiveCteError::MaxIterationsExceeded {
            cte_name: evaluator.config.cte_name.clone(),
            max: evaluator.config.max_iterations,
        });
    }

    Ok(evaluator.accumulated.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transitive_closure_converges() {
        let config = RecursiveCteConfig {
            cte_name: "reachable".to_string(),
            max_iterations: 100,
            bounded_depth: None,
            shard_count: 1,
        };

        let mut eval = RecursiveCteEvaluator::new(config);

        // Graph: A→B, B→C, C→D
        let edges: Vec<(Vec<u8>, Vec<u8>)> = vec![
            (b"A".to_vec(), b"B".to_vec()),
            (b"B".to_vec(), b"C".to_vec()),
            (b"C".to_vec(), b"D".to_vec()),
        ];

        // Seed with nodes reachable from A (direct): {B}
        eval.seed(vec![b"B".to_vec()]);

        // Each iteration: for each node in frontier, find its successors
        eval.run_to_completion(|frontier| {
            let mut new_rows = Vec::new();
            for node in frontier {
                for (from, to) in &edges {
                    if from == node {
                        new_rows.push(to.clone());
                    }
                }
            }
            new_rows
        });

        assert!(eval.is_converged());
        assert_eq!(eval.iterations(), 3); // B→C (iter 1), C→D (iter 2), D→∅ (iter 3)
        assert!(eval.result().contains(&b"B".to_vec()));
        assert!(eval.result().contains(&b"C".to_vec()));
        assert!(eval.result().contains(&b"D".to_vec()));
    }

    #[test]
    fn cyclic_graph_converges() {
        let config = RecursiveCteConfig {
            cte_name: "cycle".to_string(),
            max_iterations: 100,
            bounded_depth: None,
            shard_count: 1,
        };

        let mut eval = RecursiveCteEvaluator::new(config);

        // Graph: A→B, B→C, C→A (cycle)
        let edges: Vec<(Vec<u8>, Vec<u8>)> = vec![
            (b"A".to_vec(), b"B".to_vec()),
            (b"B".to_vec(), b"C".to_vec()),
            (b"C".to_vec(), b"A".to_vec()),
        ];

        eval.seed(vec![b"B".to_vec()]);

        eval.run_to_completion(|frontier| {
            let mut new_rows = Vec::new();
            for node in frontier {
                for (from, to) in &edges {
                    if from == node {
                        new_rows.push(to.clone());
                    }
                }
            }
            new_rows
        });

        assert!(eval.is_converged());
        // All nodes reachable from B: C, A, B (cycle discovered)
        assert_eq!(eval.result().len(), 3);
    }

    #[test]
    fn max_iterations_diverges() {
        let config = RecursiveCteConfig {
            cte_name: "infinite".to_string(),
            max_iterations: 5,
            bounded_depth: None,
            shard_count: 1,
        };

        let mut eval = RecursiveCteEvaluator::new(config);
        eval.seed(vec![vec![0]]);

        // Always produce new rows (simulate infinite recursion)
        let mut counter = 1u64;
        eval.run_to_completion(|_frontier| {
            counter += 1;
            vec![counter.to_be_bytes().to_vec()]
        });

        assert!(eval.is_diverged());
        assert_eq!(eval.iterations(), 6); // 5+1 (exceeds on 6th)
    }

    #[test]
    fn validate_unbounded_rejects_multi_shard() {
        let config = RecursiveCteConfig {
            cte_name: "test".to_string(),
            max_iterations: 100,
            bounded_depth: None,
            shard_count: 4,
        };

        assert_eq!(
            validate_recursive_cte(&config),
            Err(RecursiveCteError::UnboundedRequiresSingleShard {
                requested_shards: 4
            })
        );
    }

    #[test]
    fn bounded_depth_allows_multi_shard() {
        let config = RecursiveCteConfig {
            cte_name: "org_chart".to_string(),
            max_iterations: 100,
            bounded_depth: Some(10),
            shard_count: 4,
        };

        assert!(validate_recursive_cte(&config).is_ok());
    }

    #[test]
    fn detect_recursive_ctes_in_deps() {
        let deps = vec![
            ("cte_a".to_string(), vec!["base_table".to_string()]),
            (
                "cte_b".to_string(),
                vec!["cte_a".to_string(), "cte_b".to_string()],
            ), // recursive
        ];

        let recursive = detect_recursive_ctes(&deps);
        assert_eq!(recursive, vec!["cte_b".to_string()]);
    }
}
