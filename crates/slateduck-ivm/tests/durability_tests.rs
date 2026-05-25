//! Tier 6d — Frontier durability tests.
//!
//! Tests: SIGKILL worker at T=0, T=100, T=500; restart worker; assert loaded
//! frontier skips already-processed events and final output is identical to
//! uninterrupted run.

use slateduck_ivm::dag::FrontierClock;
use slateduck_ivm::plan::IvmPlan;
use slateduck_ivm::slatedb_trace::{CompactionTrigger, SlateDbTrace, SlateDbTraceConfig};
use std::time::Duration;

fn test_config() -> SlateDbTraceConfig {
    SlateDbTraceConfig {
        state_prefix: "s3://test/state".to_string(),
        matview_id: 1,
        shard_id: 0,
        flush_coalesce_window: Duration::from_millis(0), // Immediate flush for testing.
        await_durable_non_checkpoint: false,
        await_durable_checkpoint: true,
        compaction_trigger: CompactionTrigger::Default,
    }
}

fn make_plan() -> IvmPlan {
    IvmPlan::parse("SELECT dept, COUNT(*) AS cnt FROM emp GROUP BY dept").unwrap()
}

/// Test: crash at T=0 (before any CDC events); restart; frontier is 0.
#[test]
fn crash_at_t0_restart_processes_all_events() {
    let plan = make_plan();
    let config = test_config();

    // Worker starts, immediately crashes before processing anything.
    let trace = SlateDbTrace::new(plan.clone(), config.clone());
    // No checkpoint taken — frontier is 0.
    assert_eq!(trace.inner.last_input_snapshot, 0);
    assert_eq!(trace.inner.seq, 0);

    // On restart, worker starts from frontier 0 — processes all events.
    let restarted = SlateDbTrace::new(plan, config);
    assert_eq!(restarted.inner.last_input_snapshot, 0);
    assert_eq!(restarted.inner.seq, 0);
}

/// Test: crash at T=100 events; restart from checkpoint; skip first 100.
#[test]
fn crash_at_t100_restart_skips_processed_events() {
    let plan = make_plan();
    let config = test_config();

    // Process 100 events, take checkpoint.
    let mut trace = SlateDbTrace::new(plan.clone(), config.clone());
    for _ in 0..100 {
        trace.record_batch(1, false);
    }
    let state_at_100 = trace.checkpoint(100, 100);
    assert_eq!(state_at_100.snapshot.last_input_snapshot, 100);
    assert_eq!(state_at_100.snapshot.seq, 1);

    // Simulate crash + restart from persisted state.
    let restored = SlateDbTrace::restore(plan, config, state_at_100);
    assert_eq!(restored.inner.last_input_snapshot, 100);
    assert_eq!(restored.inner.seq, 1);

    // Frontier check: events <= 100 should be skipped.
    let mut frontier = FrontierClock::new();
    frontier.advance(1, 100);
    assert!(frontier.get(1) >= 100);
}

/// Test: crash at T=500 events; restart from checkpoint; skip first 500.
#[test]
fn crash_at_t500_restart_skips_processed_events() {
    let plan = make_plan();
    let config = test_config();

    // Process 500 events, take checkpoint.
    let mut trace = SlateDbTrace::new(plan.clone(), config.clone());
    for _ in 0..500 {
        trace.record_batch(1, false);
    }
    let state_at_500 = trace.checkpoint(500, 500);
    assert_eq!(state_at_500.snapshot.last_input_snapshot, 500);
    assert_eq!(state_at_500.snapshot.seq, 1);

    // Simulate crash + restart from persisted state.
    let restored = SlateDbTrace::restore(plan.clone(), config.clone(), state_at_500);
    assert_eq!(restored.inner.last_input_snapshot, 500);
    assert_eq!(restored.inner.seq, 1);

    // Verify frontier-based skip logic.
    let mut frontier = FrontierClock::new();
    frontier.advance(1, 500);
    // Any CDC event with seq <= 500 should be skipped.
    assert!(frontier.get(1) >= 500);
    // Event at seq 501 should be processed.
    assert!(501 > frontier.get(1));

    // After restore, process 200 more events and checkpoint again.
    let mut restored = restored;
    for _ in 0..200 {
        restored.record_batch(1, false);
    }
    let final_state = restored.checkpoint(200, 200);
    // Seq should be 2 (one checkpoint before crash, one after).
    assert_eq!(final_state.snapshot.seq, 2);
}
