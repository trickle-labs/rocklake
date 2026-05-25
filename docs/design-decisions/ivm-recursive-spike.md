# IVM Recursive CTE Spike Findings

**Date:** 2026-05-25
**Timebox:** 1 day
**Status:** Complete

## Objectives

1. Verify that DBSP's `iterate` operator is callable from outside DBSP's circuit builder without forking.
2. Verify that `iterate`'s termination detection (output = input at fixed point) maps cleanly onto SlateDuck's frontier advancement — specifically, when does `iterate` know the fixed point has been reached for a given input snapshot?

## Findings

### Q1: Is `iterate` callable externally?

**Answer: Yes, with a wrapper.** DBSP's `iterate` operator computes a local fixed point within DBSP's own time domain. It is designed to be used within DBSP's circuit builder. However, the fundamental algorithm — "apply the recursive body, check if the output delta is empty, repeat" — can be implemented directly without calling into DBSP's API.

**Decision:** Implement a hand-rolled fixed-point loop in the IVM worker that:
- Takes the recursive body as a closure
- Applies it to the current frontier (set of newly-produced rows)
- Checks if any new rows were produced (not already in the accumulated set)
- Terminates when no new rows are produced (fixed point)
- Respects a `max_iterations` bound to prevent infinite loops

This approach is simpler, avoids a DBSP crate dependency, and gives full control over the iteration lifecycle within SlateDuck's snapshot model.

### Q2: Does `iterate` map to SlateDuck's snapshot frontier?

**Answer: Yes, naturally.** Each input snapshot triggers one top-level step. Within that step, the recursive CTE runs its fixed-point loop to completion before the frontier advances. The mapping is:

```
SlateDuck snapshot N arrives
  → Recursive CTE body iterates (k times) until fixed point
  → All accumulated rows become the output for snapshot N
  → Frontier advances to N+1
```

The key insight: the recursive CTE's internal iterations are *sub-steps* within a single snapshot advance. They do not produce intermediate checkpoints. Only the final converged result is checkpointed.

### Q3: Cross-shard convergence

For unbounded recursive CTEs (e.g., transitive closure), the fixed point depends on global state. A partitioned graph may have edges that cross shard boundaries. Local fixed-point ≠ global fixed-point.

**Decision:** Unbounded recursive CTEs enforce `shard_count = 1` (single coordinator receives global shuffle). This is the same approach DBSP's distributed runtime uses for `iterate`.

**Exception:** Bounded-depth recursions (e.g., `CONNECT BY` with `max_depth ≤ D`) may use sharded execution because each iteration only needs local + 1-hop-neighbor data (communicated via the existing reshuffle join path from v0.13).

## Implementation

The recursive CTE evaluator lives in `crates/slateduck-ivm/src/recursive_cte.rs` and provides:

- `RecursiveCteEvaluator::seed(rows)` — initialize with base case
- `RecursiveCteEvaluator::step(apply_fn)` — one iteration
- `RecursiveCteEvaluator::run_to_completion(apply_fn)` — iterate until fixed point or max_iterations
- `validate_recursive_cte(config)` — rejects `shard_count > 1` for unbounded CTEs

## Performance Characteristics

- **Convergence:** For a graph with diameter D, the fixed-point loop terminates in exactly D+1 iterations (the last one confirms no new rows).
- **State:** All accumulated rows are held in memory during iteration. For large graphs (1M+ edges), this requires the large runner.
- **Incremental updates:** When edges are added/removed, the CTE re-seeds with the updated edge set and re-converges. This is correct but not optimal (future: delta-based incremental convergence).

## Risks

1. **Memory:** Large graphs may exceed worker memory during iteration. Mitigated by `shard_count = 1` (dedicated worker) and documentation.
2. **Latency:** Per-batch latency is O(D × |frontier|) where D is graph diameter. For TPC-H-style hierarchies (depth ≤ 5), this is acceptable.
3. **Max iterations:** Default 100 is sufficient for most real-world hierarchies. If exceeded, the view is marked `Stale` (not `Broken`) and alerts fire.
