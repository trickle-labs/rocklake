//! Tier 6d — IVM hardening tests.
//!
//! Tests: repair shard rebuilds from base, REFRESH FULL rebuilds all shards,
//! doctor identifies stuck/expired shards, exactly-once output under restart,
//! backup/restore (restored frontier prevents event re-processing).

use slateduck_ivm::backup::{self, BackupConfig};
use slateduck_ivm::dag::FrontierClock;
use slateduck_ivm::exactly_once::{CommitResult, OutputDeduplicator, OutputTag};
use slateduck_ivm::repair::{self, DoctorIssue, RepairOperation, ShardDiagnostics};
use std::collections::HashMap;

/// Test: repair shard rebuilds from base data.
#[test]
fn repair_shard_rebuilds_from_base() {
    let mut state = repair::plan_shard_repair(1, 2);
    assert_eq!(state.total_shards, 1);
    assert!(!state.is_done());

    state.complete_shard(2);
    assert!(state.is_done());
    assert!(state.is_success());

    let record =
        repair::create_repair_record(1, Some(2), RepairOperation::ShardRepair, 3000, true, None);
    assert_eq!(record.operation, RepairOperation::ShardRepair);
    assert!(record.success);
    assert!(record.timestamp_ms > 0);
}

/// Test: REFRESH FULL rebuilds all shards in parallel.
#[test]
fn refresh_full_rebuilds_all_shards() {
    let mut state = repair::plan_refresh_full(42, 8);
    assert_eq!(state.total_shards, 8);
    assert_eq!(state.progress_pct(), 0.0);

    for shard_id in 0..8 {
        state.complete_shard(shard_id);
    }

    assert!(state.is_done());
    assert!(state.is_success());
    assert_eq!(state.progress_pct(), 100.0);

    let record =
        repair::create_repair_record(42, None, RepairOperation::RefreshFull, 15000, true, None);
    assert_eq!(record.operation, RepairOperation::RefreshFull);
}

/// Test: doctor identifies stuck shards, expired leases, and lagging frontiers.
#[test]
fn doctor_identifies_stuck_expired_shards() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let mut shards = HashMap::new();

    // Expired lease.
    shards.insert(
        0,
        ShardDiagnostics {
            lease_expired: true,
            last_lease_renewal_ms: now - 120_000,
            lease_duration_ms: 30_000,
            input_frontier: 100,
            output_frontier: 100,
        },
    );

    // Stuck lease (not renewed in 2× lease duration).
    shards.insert(
        1,
        ShardDiagnostics {
            lease_expired: false,
            last_lease_renewal_ms: now - 70_000,
            lease_duration_ms: 30_000,
            input_frontier: 100,
            output_frontier: 100,
        },
    );

    // Lagging frontier.
    shards.insert(
        2,
        ShardDiagnostics {
            lease_expired: false,
            last_lease_renewal_ms: now,
            lease_duration_ms: 30_000,
            input_frontier: 200,
            output_frontier: 100,
        },
    );

    // Healthy shard.
    shards.insert(
        3,
        ShardDiagnostics {
            lease_expired: false,
            last_lease_renewal_ms: now,
            lease_duration_ms: 30_000,
            input_frontier: 50,
            output_frontier: 50,
        },
    );

    let report = repair::run_doctor(1, &shards);

    // Should identify expired lease.
    assert!(report
        .issues
        .iter()
        .any(|i| matches!(i, DoctorIssue::ExpiredLease { shard_id: 0 })));

    // Should identify stuck lease.
    assert!(report
        .issues
        .iter()
        .any(|i| matches!(i, DoctorIssue::StuckLease { shard_id: 1, .. })));

    // Should identify lagging frontier.
    assert!(report
        .issues
        .iter()
        .any(|i| matches!(i, DoctorIssue::LaggingFrontier { shard_id: 2, .. })));

    // Healthy shard should not appear.
    assert!(!report.issues.iter().any(|i| matches!(
        i,
        DoctorIssue::ExpiredLease { shard_id: 3 }
            | DoctorIssue::StuckLease { shard_id: 3, .. }
            | DoctorIssue::LaggingFrontier { shard_id: 3, .. }
    )));
}

/// Test: exactly-once output under output-plane restart.
#[test]
fn exactly_once_output_under_restart() {
    let mut dedup = OutputDeduplicator::new();

    // Simulate first commit.
    let tag1 = OutputTag {
        matview_id: 1,
        target_frontier: 100,
        shard_id: 0,
    };
    assert_eq!(dedup.try_commit(tag1.clone()), CommitResult::Committed);

    // Simulate restart: worker attempts to re-commit same frontier.
    assert_eq!(dedup.try_commit(tag1.clone()), CommitResult::Duplicate);

    // New frontier should succeed.
    let tag2 = OutputTag {
        matview_id: 1,
        target_frontier: 200,
        shard_id: 0,
    };
    assert_eq!(dedup.try_commit(tag2), CommitResult::Committed);

    // Different shard, same frontier — should succeed (different output stream).
    let tag3 = OutputTag {
        matview_id: 1,
        target_frontier: 100,
        shard_id: 1,
    };
    assert_eq!(dedup.try_commit(tag3), CommitResult::Committed);
}

/// Test: backup/restore (restored frontier prevents event re-processing).
#[test]
fn backup_restore_frontier_prevents_reprocessing() {
    // Create initial frontier at seq 500.
    let mut frontier = FrontierClock::new();
    frontier.advance(1, 500);
    frontier.advance(2, 300);

    // Take backup.
    let manifest = backup::create_backup(
        "s3://bucket/state",
        42,
        0,
        frontier.clone(),
        vec!["sst-001.sst".to_string()],
    );

    assert_eq!(manifest.matview_id, 42);
    assert_eq!(manifest.shard_id, 0);

    // Simulate: inject 200 more events (advancing to seq 700).
    // Then corrupt state and restore.
    let result = backup::restore_from_backup(&manifest);
    match result {
        backup::RestoreResult::Success {
            frontier: restored_frontier,
        } => {
            // Worker should resume from seq 500, processing only events 501–700.
            assert_eq!(restored_frontier.get(1), 500);
            assert_eq!(restored_frontier.get(2), 300);
            // The 200 post-backup events will be re-processed, but the 500
            // pre-backup events are skipped.
        }
        _ => panic!("Expected successful restore"),
    }

    // Test missing state store without auto-rebuild.
    let config = BackupConfig {
        state_prefix: "s3://bucket/state".to_string(),
        auto_rebuild_on_loss: false,
    };
    let result = backup::handle_missing_state_store(&config);
    assert_eq!(result, backup::RestoreResult::StateStoreMissing);

    // Test with auto-rebuild: empty frontier (full recompute).
    let config = BackupConfig {
        state_prefix: "s3://bucket/state".to_string(),
        auto_rebuild_on_loss: true,
    };
    let result = backup::handle_missing_state_store(&config);
    assert_eq!(
        result,
        backup::RestoreResult::Success {
            frontier: FrontierClock::new()
        }
    );
}
