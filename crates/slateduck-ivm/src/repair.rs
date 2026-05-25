//! REFRESH FULL and per-shard repair operations.
//!
//! `REFRESH INCREMENTAL MATERIALIZED VIEW v FULL` drops state stores and
//! rebuilds from scratch in parallel.
//! `slateduck-ivm repair --matview v --shard N` recomputes a single shard.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// A repair/rebuild operation record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairRecord {
    /// Timestamp of the repair operation.
    pub timestamp_ms: u64,
    /// Matview ID.
    pub matview_id: u64,
    /// Shard ID (None for full rebuild).
    pub shard_id: Option<u32>,
    /// Operation type.
    pub operation: RepairOperation,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Whether the operation was successful.
    pub success: bool,
    /// Error message if not successful.
    pub error: Option<String>,
}

/// Type of repair operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepairOperation {
    /// Full refresh: drop all state stores and rebuild from scratch.
    RefreshFull,
    /// Single shard repair: recompute one shard from base data.
    ShardRepair,
    /// State store reset (after restore or corruption).
    StateReset,
}

/// State of a full rebuild operation.
#[derive(Debug, Clone)]
pub struct RebuildState {
    pub matview_id: u64,
    pub total_shards: u32,
    pub completed_shards: u32,
    pub failed_shards: Vec<u32>,
    pub started_at: u64,
}

impl RebuildState {
    /// Create a new rebuild state.
    pub fn new(matview_id: u64, total_shards: u32) -> Self {
        Self {
            matview_id,
            total_shards,
            completed_shards: 0,
            failed_shards: Vec::new(),
            started_at: now_ms(),
        }
    }

    /// Mark a shard as completed.
    pub fn complete_shard(&mut self, _shard_id: u32) {
        self.completed_shards += 1;
    }

    /// Mark a shard as failed.
    pub fn fail_shard(&mut self, shard_id: u32) {
        self.failed_shards.push(shard_id);
    }

    /// Check if all shards are done (successfully or not).
    pub fn is_done(&self) -> bool {
        self.completed_shards + self.failed_shards.len() as u32 >= self.total_shards
    }

    /// Check if the rebuild was fully successful.
    pub fn is_success(&self) -> bool {
        self.is_done() && self.failed_shards.is_empty()
    }

    /// Progress percentage.
    pub fn progress_pct(&self) -> f64 {
        if self.total_shards == 0 {
            return 100.0;
        }
        (self.completed_shards as f64 / self.total_shards as f64) * 100.0
    }
}

/// Plan a REFRESH FULL operation for a matview.
///
/// Returns the list of shards to rebuild.
pub fn plan_refresh_full(matview_id: u64, shard_count: u32) -> RebuildState {
    RebuildState::new(matview_id, shard_count)
}

/// Plan a single-shard repair.
pub fn plan_shard_repair(matview_id: u64, shard_id: u32) -> RebuildState {
    let state = RebuildState::new(matview_id, 1);
    // Only one shard to repair.
    let _ = shard_id; // Used for targeting.
    state
}

/// Create an audit trail record for a completed repair.
pub fn create_repair_record(
    matview_id: u64,
    shard_id: Option<u32>,
    operation: RepairOperation,
    duration_ms: u64,
    success: bool,
    error: Option<String>,
) -> RepairRecord {
    RepairRecord {
        timestamp_ms: now_ms(),
        matview_id,
        shard_id,
        operation,
        duration_ms,
        success,
        error,
    }
}

/// Doctor report: per-matview health status.
#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub matview_id: u64,
    pub issues: Vec<DoctorIssue>,
}

/// An issue identified by the doctor.
#[derive(Debug, Clone, PartialEq)]
pub enum DoctorIssue {
    /// Shard has a stuck lease (not renewed within expected window).
    StuckLease { shard_id: u32, last_renewal_ms: u64 },
    /// Shard lease has expired.
    ExpiredLease { shard_id: u32 },
    /// Shard frontier is lagging significantly.
    LaggingFrontier { shard_id: u32, lag: u64 },
    /// Cost outlier: estimated cost significantly above median.
    CostOutlier { estimated_monthly_usd: f64 },
}

/// Run doctor diagnostics on matview shards.
pub fn run_doctor(matview_id: u64, shard_states: &HashMap<u32, ShardDiagnostics>) -> DoctorReport {
    let mut issues = Vec::new();

    let now = now_ms();

    for (&shard_id, diag) in shard_states {
        // Check lease health.
        if diag.lease_expired {
            issues.push(DoctorIssue::ExpiredLease { shard_id });
        } else if now - diag.last_lease_renewal_ms > diag.lease_duration_ms * 2 {
            issues.push(DoctorIssue::StuckLease {
                shard_id,
                last_renewal_ms: diag.last_lease_renewal_ms,
            });
        }

        // Check frontier lag.
        if diag.input_frontier > diag.output_frontier + 50 {
            issues.push(DoctorIssue::LaggingFrontier {
                shard_id,
                lag: diag.input_frontier - diag.output_frontier,
            });
        }
    }

    DoctorReport { matview_id, issues }
}

/// Diagnostic data for a single shard.
#[derive(Debug, Clone)]
pub struct ShardDiagnostics {
    pub lease_expired: bool,
    pub last_lease_renewal_ms: u64,
    pub lease_duration_ms: u64,
    pub input_frontier: u64,
    pub output_frontier: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuild_state_progress() {
        let mut state = plan_refresh_full(1, 4);
        assert_eq!(state.progress_pct(), 0.0);

        state.complete_shard(0);
        state.complete_shard(1);
        assert_eq!(state.progress_pct(), 50.0);

        state.complete_shard(2);
        state.complete_shard(3);
        assert!(state.is_done());
        assert!(state.is_success());
    }

    #[test]
    fn rebuild_with_failures() {
        let mut state = plan_refresh_full(1, 4);
        state.complete_shard(0);
        state.complete_shard(1);
        state.fail_shard(2);
        state.complete_shard(3);
        assert!(state.is_done());
        assert!(!state.is_success());
    }

    #[test]
    fn doctor_identifies_issues() {
        let mut shards = HashMap::new();
        shards.insert(
            0,
            ShardDiagnostics {
                lease_expired: true,
                last_lease_renewal_ms: 0,
                lease_duration_ms: 30_000,
                input_frontier: 100,
                output_frontier: 100,
            },
        );
        shards.insert(
            1,
            ShardDiagnostics {
                lease_expired: false,
                last_lease_renewal_ms: now_ms(),
                lease_duration_ms: 30_000,
                input_frontier: 200,
                output_frontier: 100,
            },
        );

        let report = run_doctor(1, &shards);
        assert!(report
            .issues
            .contains(&DoctorIssue::ExpiredLease { shard_id: 0 }));
        assert!(report
            .issues
            .iter()
            .any(|i| matches!(i, DoctorIssue::LaggingFrontier { shard_id: 1, .. })));
    }

    #[test]
    fn repair_record_creation() {
        let record =
            create_repair_record(1, Some(0), RepairOperation::ShardRepair, 5000, true, None);
        assert_eq!(record.matview_id, 1);
        assert_eq!(record.shard_id, Some(0));
        assert!(record.success);
    }
}
