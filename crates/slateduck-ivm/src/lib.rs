//! slateduck-ivm: Incremental View Maintenance (IVM) engine for SlateDuck.
//!
//! This crate implements the IVM runtime:
//!   - `IvmPlan`        — parses a view SQL into GROUP BY + aggregation + JOIN plan
//!   - `IvmWorker`      — drives the incremental computation loop
//!   - `IvmTrace`       — maintains aggregate state between checkpoints
//!   - `IvmJoinCircuit` — multi-input join + aggregation circuit (v0.13)
//!   - `volatility`     — function volatility gate (v0.14)
//!   - `dag`            — multi-view DAG with frontier coordination (v0.15)
//!   - `slatedb_trace`  — native SlateDB-backed trace (v0.15)
//!   - CLI binary       — `slateduck-ivm serve`
//!
//! ## Architecture
//! The IVM computation uses a pure-Rust incremental GROUP BY engine inspired
//! by DBSP (Feldera) semantics. The DBSP crate is listed as a workspace
//! dependency and provides the foundational algebraic model; this crate
//! implements a lightweight compatibility shim in `circuit.rs` that adapts
//! SlateDuck's append-only CDC stream to the DBSP Zset/Z-difference model.
//!
//! ## v0.13: Joins
//! Three join strategies are supported:
//!   - **Broadcast** (`join::JoinStrategy::Broadcast`) — small dimension table
//!     fully replicated to every shard.
//!   - **CoPartitioned** (`join::JoinStrategy::CoPartitioned`) — both inputs
//!     share the same shard key; join is entirely local.
//!   - **Reshuffle** (`join::JoinStrategy::Reshuffle`) — one side is
//!     repartitioned through a temporary exchange buffer.
//!
//! ## v0.14: Join Correctness & Aggregate Tiers
//! - EC-01 phantom-row fix: asymmetric delta branches for join inserts/deletes
//! - Aggregate tier classification: Algebraic, SemiAlgebraic, GroupRescan
//! - Volatility validation at view creation time
//! - Property-based "differential ≡ full" oracle
//!
//! ## v0.15: IVM Operational Hardening
//! - Multi-view DAG with Kahn's topological sort and diamond detection
//! - Native `SlateDbTrace` with flush coalescing and cost-mode propagation
//! - Cost guardrails: estimation, budgets, freshness degradation
//! - Backpressure protocol with per-shard publication modes
//! - Schema evolution detection (stale/broken view states)
//! - Exactly-once output snapshots via CAS deduplication
//! - REFRESH FULL, per-shard repair, and doctor diagnostics
//! - Delta optimizations: change-buffer compaction, predicate pushdown,
//!   semi-join key pre-filter, append-only fast path
//! - PG-Wire rate limiting (connection + auth failure)
//! - State store backup and restore with compaction pins

pub mod backpressure;
pub mod backup;
pub mod circuit;
pub mod config;
pub mod cost;
pub mod dag;
pub mod delete_files;
pub mod delta_opt;
pub mod exactly_once;
pub mod heartbeat;
pub mod join;
pub mod observability;
pub mod output;
pub mod parquet;
pub mod plan;
pub mod rate_limit;
pub mod repair;
pub mod schema_evolution;
pub mod shard_key;
pub mod shutdown;
pub mod slatedb_trace;
pub mod source;
pub mod state_store;
pub mod trace;
pub mod volatility;
pub mod worker;

pub use backpressure::{BackpressureConfig, BackpressureState, OutputMode};
pub use backup::{BackupConfig, BackupManifest, RestoreResult};
pub use circuit::{IvmCircuit, IvmJoinCircuit, ZDelta};
pub use config::{CostMode, WorkerConfig};
pub use cost::{CostBudget, CostEstimate, CostEstimateParams};
pub use dag::{DiamondApex, FrontierClock, ViewDag};
pub use delta_opt::{AppendOnlyDetector, CompactionResult, SortKeyConfig};
pub use exactly_once::{CommitResult, OutputDeduplicator, OutputTag};
pub use heartbeat::{HeartbeatHandle, LeaseRegistry};
pub use join::{
    hash_join_batch, select_strategy, ExchangeBuffer, HashJoinState, JoinClause, JoinStrategy,
    DEFAULT_BROADCAST_THRESHOLD,
};
pub use parquet::{CompactionPolicy, ParquetOutputConfig};
pub use plan::{Aggregate, AggregateKind, AggregateTier, IvmPlan};
pub use rate_limit::{RateLimitConfig, RateLimitResult, RateLimiter};
pub use repair::{DoctorIssue, DoctorReport, RebuildState, RepairOperation, RepairRecord};
pub use schema_evolution::{SchemaChange, ViewStatus};
pub use shard_key::{compute_key_ranges, hash_key_value, shard_index_for, ShardKeyRange};
pub use shutdown::ShutdownSignal;
pub use slatedb_trace::{SlateDbTrace, SlateDbTraceConfig};
pub use source::MatviewInputSource;
pub use state_store::ShardStateStore;
pub use trace::IvmTrace;
pub use volatility::Volatility;
pub use worker::IvmWorker;
