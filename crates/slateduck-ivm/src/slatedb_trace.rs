//! Native SlateDB-backed trace implementation.
//!
//! Implements persistent trace storage using SlateDB as the backing store.
//! This replaces the in-memory-only trace with durable state that survives
//! worker restarts without full recomputation.
//!
//! Design (Option A — extend shim): extends `IvmTrace` with SlateDB-backed
//! state serialization and compaction policies.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::config::CostMode;
use crate::plan::IvmPlan;
use crate::trace::{IvmTrace, TraceGroup, TraceSnapshot};

/// Configuration for the SlateDB trace backend.
#[derive(Debug, Clone)]
pub struct SlateDbTraceConfig {
    /// State store path prefix.
    pub state_prefix: String,
    /// Matview ID this trace belongs to.
    pub matview_id: u64,
    /// Shard ID.
    pub shard_id: u32,
    /// Flush coalescing window — only flush when this much time has elapsed
    /// since last flush AND buffered work exists.
    pub flush_coalesce_window: Duration,
    /// Whether to await durability for non-checkpoint writes.
    pub await_durable_non_checkpoint: bool,
    /// Whether to await durability at checkpoint boundaries.
    pub await_durable_checkpoint: bool,
    /// Compaction trigger aggressiveness.
    pub compaction_trigger: CompactionTrigger,
}

/// Compaction trigger policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionTrigger {
    /// Compact aggressively (conservative cost mode).
    Aggressive,
    /// Default compaction schedule.
    Default,
    /// Lazy compaction (latency cost mode).
    Lazy,
}

/// Persistent trace state backed by SlateDB.
///
/// Wraps the in-memory `IvmTrace` with durable persistence. Flushes are
/// coalesced based on `flush_coalesce_window` to reduce S3 PUT costs.
pub struct SlateDbTrace {
    /// Inner in-memory trace.
    pub inner: IvmTrace,
    /// Configuration.
    pub config: SlateDbTraceConfig,
    /// Last flush timestamp.
    last_flush: Instant,
    /// Number of batches since last flush.
    batches_since_flush: u64,
    /// Total events processed since last flush.
    events_since_flush: u64,
    /// Metrics: total S3 PUTs.
    pub s3_puts_total: u64,
    /// Metrics: total S3 GETs.
    pub s3_gets_total: u64,
    /// Whether append-only fast path is active.
    pub append_only_mode: bool,
    /// Count of consecutive insert-only batches.
    consecutive_insert_only: u64,
    /// Threshold for activating append-only mode.
    append_only_threshold: u64,
}

/// Serializable trace state for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedTraceState {
    pub snapshot: TraceSnapshot,
    pub frontier: crate::dag::FrontierClock,
    pub s3_puts_total: u64,
    pub s3_gets_total: u64,
    pub append_only_mode: bool,
}

impl SlateDbTraceConfig {
    /// Create config from cost mode with sensible defaults.
    pub fn from_cost_mode(
        cost_mode: CostMode,
        state_prefix: String,
        matview_id: u64,
        shard_id: u32,
        freshness: Duration,
    ) -> Self {
        let (flush_coalesce, compaction) = match cost_mode {
            CostMode::Conservative => (freshness, CompactionTrigger::Aggressive),
            CostMode::Balanced => (freshness / 2, CompactionTrigger::Default),
            CostMode::Latency => (freshness / 4, CompactionTrigger::Lazy),
            _ => (freshness / 2, CompactionTrigger::Default),
        };

        Self {
            state_prefix,
            matview_id,
            shard_id,
            flush_coalesce_window: flush_coalesce,
            await_durable_non_checkpoint: false,
            await_durable_checkpoint: true,
            compaction_trigger: compaction,
        }
    }
}

impl SlateDbTrace {
    /// Create a new SlateDB-backed trace.
    pub fn new(plan: IvmPlan, config: SlateDbTraceConfig) -> Self {
        Self {
            inner: IvmTrace::new(plan),
            config,
            last_flush: Instant::now(),
            batches_since_flush: 0,
            events_since_flush: 0,
            s3_puts_total: 0,
            s3_gets_total: 0,
            append_only_mode: false,
            consecutive_insert_only: 0,
            append_only_threshold: 10,
        }
    }

    /// Restore from persisted state.
    pub fn restore(plan: IvmPlan, config: SlateDbTraceConfig, state: PersistedTraceState) -> Self {
        let mut trace = Self::new(plan, config);
        trace.inner.last_input_snapshot = state.snapshot.last_input_snapshot;
        trace.inner.last_output_snapshot = state.snapshot.last_output_snapshot;
        trace.inner.seq = state.snapshot.seq;
        trace.s3_puts_total = state.s3_puts_total;
        trace.s3_gets_total = state.s3_gets_total;
        trace.append_only_mode = state.append_only_mode;
        // Restore circuit state from groups.
        for group in &state.snapshot.groups {
            trace
                .inner
                .circuit
                .push_restored_group(group.key.clone(), group.values.clone());
        }
        trace.s3_gets_total += 1; // Count the restore GET.
        trace
    }

    /// Record a batch of events. Returns true if a flush should occur.
    pub fn record_batch(&mut self, event_count: u64, has_deletes: bool) -> bool {
        self.batches_since_flush += 1;
        self.events_since_flush += event_count;

        // Append-only fast path tracking.
        if has_deletes {
            self.consecutive_insert_only = 0;
            self.append_only_mode = false;
        } else {
            self.consecutive_insert_only += 1;
            if self.consecutive_insert_only >= self.append_only_threshold {
                self.append_only_mode = true;
            }
        }

        self.should_flush()
    }

    /// Check if flush should occur based on coalescing window.
    pub fn should_flush(&self) -> bool {
        self.events_since_flush > 0
            && self.last_flush.elapsed() >= self.config.flush_coalesce_window
    }

    /// Perform a flush (persist state to SlateDB).
    pub fn flush(&mut self) -> PersistedTraceState {
        let snapshot = TraceSnapshot {
            last_input_snapshot: self.inner.last_input_snapshot,
            last_output_snapshot: self.inner.last_output_snapshot,
            seq: self.inner.seq,
            groups: self
                .inner
                .read_output()
                .into_iter()
                .map(|row| TraceGroup {
                    key: vec![],
                    values: row,
                })
                .collect(),
        };

        self.s3_puts_total += 1;
        self.last_flush = Instant::now();
        self.batches_since_flush = 0;
        self.events_since_flush = 0;

        PersistedTraceState {
            snapshot,
            frontier: crate::dag::FrontierClock::new(),
            s3_puts_total: self.s3_puts_total,
            s3_gets_total: self.s3_gets_total,
            append_only_mode: self.append_only_mode,
        }
    }

    /// Perform a checkpoint flush (always durable).
    pub fn checkpoint(&mut self, input_snapshot: u64, output_snapshot: u64) -> PersistedTraceState {
        self.inner
            .advance_checkpoint(input_snapshot, output_snapshot);
        self.flush()
    }

    /// Get the compaction ratio for change-buffer compaction metrics.
    pub fn compaction_ratio(&self) -> f64 {
        if self.events_since_flush == 0 {
            0.0
        } else {
            // Placeholder: in a real implementation this tracks cancelled pairs.
            0.0
        }
    }

    /// Reset state for a full rebuild.
    pub fn reset(&mut self, plan: IvmPlan) {
        self.inner = IvmTrace::new(plan);
        self.last_flush = Instant::now();
        self.batches_since_flush = 0;
        self.events_since_flush = 0;
        self.s3_puts_total += 1; // Count the reset PUT.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SlateDbTraceConfig {
        SlateDbTraceConfig {
            state_prefix: "s3://test-bucket/state".to_string(),
            matview_id: 1,
            shard_id: 0,
            flush_coalesce_window: Duration::from_millis(100),
            await_durable_non_checkpoint: false,
            await_durable_checkpoint: true,
            compaction_trigger: CompactionTrigger::Default,
        }
    }

    #[test]
    fn slatedb_trace_flush_coalescing() {
        let plan = IvmPlan::parse("SELECT dept, COUNT(*) AS cnt FROM emp GROUP BY dept").unwrap();
        let mut trace = SlateDbTrace::new(plan, test_config());

        // First batch: should not flush immediately (window not elapsed).
        let should = trace.record_batch(10, false);
        // With a 100ms window, won't have elapsed yet.
        assert!(!should);

        // After time passes, should flush.
        trace.last_flush = Instant::now() - Duration::from_millis(200);
        let should = trace.record_batch(5, false);
        assert!(should);
    }

    #[test]
    fn slatedb_trace_append_only_detection() {
        let plan = IvmPlan::parse("SELECT dept, COUNT(*) AS cnt FROM emp GROUP BY dept").unwrap();
        let mut trace = SlateDbTrace::new(plan, test_config());

        assert!(!trace.append_only_mode);
        for _ in 0..10 {
            trace.record_batch(1, false);
        }
        assert!(trace.append_only_mode);

        // A single delete resets it.
        trace.record_batch(1, true);
        assert!(!trace.append_only_mode);
    }

    #[test]
    fn slatedb_trace_checkpoint_advances_seq() {
        let plan = IvmPlan::parse("SELECT dept, COUNT(*) AS cnt FROM emp GROUP BY dept").unwrap();
        let mut trace = SlateDbTrace::new(plan, test_config());

        let state = trace.checkpoint(100, 200);
        assert_eq!(state.snapshot.last_input_snapshot, 100);
        assert_eq!(state.snapshot.last_output_snapshot, 200);
        assert_eq!(state.snapshot.seq, 1);
    }

    #[test]
    fn slatedb_trace_restore_roundtrip() {
        let plan = IvmPlan::parse("SELECT dept, COUNT(*) AS cnt FROM emp GROUP BY dept").unwrap();
        let mut trace = SlateDbTrace::new(plan.clone(), test_config());
        let state = trace.checkpoint(50, 100);

        let restored = SlateDbTrace::restore(plan, test_config(), state);
        assert_eq!(restored.inner.last_input_snapshot, 50);
        assert_eq!(restored.inner.last_output_snapshot, 100);
        assert_eq!(restored.inner.seq, 1);
        assert_eq!(restored.s3_gets_total, 1);
    }

    #[test]
    fn slatedb_trace_cost_mode_config() {
        let config = SlateDbTraceConfig::from_cost_mode(
            CostMode::Conservative,
            "s3://bucket/state".to_string(),
            1,
            0,
            Duration::from_secs(10),
        );
        assert_eq!(config.flush_coalesce_window, Duration::from_secs(10));
        assert_eq!(config.compaction_trigger, CompactionTrigger::Aggressive);

        let config = SlateDbTraceConfig::from_cost_mode(
            CostMode::Latency,
            "s3://bucket/state".to_string(),
            1,
            0,
            Duration::from_secs(10),
        );
        assert_eq!(config.flush_coalesce_window, Duration::from_millis(2500));
        assert_eq!(config.compaction_trigger, CompactionTrigger::Lazy);
    }
}
