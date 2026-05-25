# SQL Planner Migration Decision

**Status:** Decided (May 2026)  
**Decision:** Deferred to v0.16 — keep sqlparser-based IvmPlan for v0.14–v0.15  
**Gate:** Pre-v0.14 Architecture Gate 5

## Context

The IVM planner (`crates/slateduck-ivm/src/plan.rs`) parses view SQL directly
via `sqlparser` and produces an `IvmPlan` struct.  This works well for v0.14
scope (GROUP BY, COUNT/SUM/MIN/MAX, equality joins) but cannot support v0.16's
correlated subqueries because:

1. Decorrelation requires rewrite passes (`PullUpCorrelatedPredicates`,
   `DecorrelatePredicateSubquery`) that operate on a `LogicalPlan` tree.
2. DataFusion already implements these passes.
3. The ad-hoc `IvmPlan` struct has no equivalent tree representation.

## Options

### Option A: Migrate incrementally starting in v0.14 (REJECTED)

Start building a `LogicalPlan`-based IVM planner now, using DataFusion's
planner infrastructure to parse view SQL into a `LogicalPlan`, then convert
the relevant subset into `IvmPlan`.

**Rejected because:**
- v0.14–v0.15 features (EC-01, aggregate tiers, volatility, persistence)
  don't need LogicalPlan
- Adding a DataFusion dependency to `slateduck-ivm` creates a large
  dependency chain increase (DataFusion 45 pulls Arrow, Parquet, etc.)
- Would force two planner paths to coexist during v0.14–v0.15 development

### Option B: Defer to v0.16 all-at-once (CHOSEN)

Keep the sqlparser-based `IvmPlan::parse()` for v0.14–v0.15.  In v0.16,
introduce a new `IvmLogicalPlan` module that:

1. Uses DataFusion's SQL parser to produce a `LogicalPlan`
2. Runs decorrelation optimizer passes
3. Extracts the IVM-relevant subset into the existing `IvmPlan` (or a richer
   successor type)

**Chosen because:**
- No unnecessary complexity in v0.14–v0.15
- DataFusion 45 is already in the workspace (via `slateduck-datafusion`)
- Clean separation: read-side uses DataFusion now; write-side (IVM) adopts it
  only when correlated subqueries demand it
- v0.16 is explicitly the "hard phase" — planner migration fits there

## Consequences

- `crates/slateduck-ivm` does NOT depend on DataFusion until v0.16
- `IvmPlan::parse()` remains the entry point for v0.14–v0.15
- v0.16 will introduce `crates/slateduck-ivm/src/logical_plan.rs` (or similar)
  that bridges DataFusion's LogicalPlan → IvmPlan
- Recursive CTEs (v0.16) will use DataFusion's SQL parser for the CTE syntax,
  then implement bounded iteration in the circuit step loop (per Gate 1 decision)

## Pre-conditions for v0.16 migration

When we start v0.16, the following must be in place:
1. DataFusion 45+ available (already in workspace)
2. `IvmPlan` extended with a `subquery` variant (or replaced with a richer AST)
3. IvmOracle tests covering correlated subquery patterns
4. Benchmark: plan parse time must not regress >2x for simple GROUP BY queries
