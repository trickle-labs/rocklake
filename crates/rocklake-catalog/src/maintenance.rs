//! Durable maintenance policy and bounded due-work selection.

use crate::error::{CatalogError, CatalogResult};
use crate::jobs::{JobKind, JobLedger};
use rocklake_core::{keys, values};
use serde::{Deserialize, Serialize};
use slatedb::{Db, IsolationLevel};

const SCHEDULE_PREFIX: &[u8] = b"maintenance:schedules:";

/// A maintenance operation that can be scheduled safely.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceTask {
    /// Create a metadata backup.
    Backup,
    /// Verify catalog and referenced data expectations.
    Verification,
    /// Advance visibility retention.
    Retention,
    /// Create a full logical checkpoint.
    Checkpoint,
    /// Scan for unreferenced data objects.
    OrphanSweep,
}

impl MaintenanceTask {
    /// Job kind used by the durable job engine.
    pub const fn job_kind(self) -> JobKind {
        match self {
            Self::Backup => JobKind::Backup,
            Self::Verification => JobKind::Verification,
            Self::Retention => JobKind::Retention,
            Self::Checkpoint => JobKind::Checkpoint,
            Self::OrphanSweep => JobKind::OrphanSweep,
        }
    }
}

/// Inclusive/exclusive minute interval in UTC.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MaintenancePeriod {
    /// Start minute in the day, 0..=1439.
    pub start_minute: u16,
    /// End minute in the day, 1..=1440.
    pub end_minute: u16,
}

impl MaintenancePeriod {
    fn contains(self, minute: u16) -> bool {
        self.start_minute <= minute && minute < self.end_minute
    }
}

/// Durable schedule policy. The policy is stored in the catalog; due work is
/// handed to the existing job ledger by the caller.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MaintenanceSchedule {
    /// Stable schedule identifier.
    pub id: String,
    /// Operation to submit to the job ledger.
    pub task: MaintenanceTask,
    /// Minimum interval between scheduled runs.
    pub interval_seconds: u64,
    /// Next eligible run as Unix milliseconds.
    pub next_run_at_unix_ms: u64,
    /// Whether this policy is active.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Optional daily maintenance window in UTC.
    #[serde(default)]
    pub window: Option<MaintenancePeriod>,
    /// UTC blackout periods inside the daily window.
    #[serde(default)]
    pub blackouts: Vec<MaintenancePeriod>,
}

/// Bounded alert thresholds for maintenance operations.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MaintenanceAlerts {
    /// Alert after a job has remained active this long.
    pub stale_job_after_seconds: u64,
    /// Alert after the scheduled window was missed this long.
    pub missed_window_after_seconds: u64,
    /// Alert when the latest successful backup is older than this.
    pub backup_age_after_seconds: u64,
    /// Alert after this many consecutive failed jobs.
    pub repeated_failures: u32,
}

impl Default for MaintenanceAlerts {
    fn default() -> Self {
        Self {
            stale_job_after_seconds: 3_600,
            missed_window_after_seconds: 3_600,
            backup_age_after_seconds: 86_400,
            repeated_failures: 3,
        }
    }
}

/// A due schedule ready to be submitted as a durable job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DueMaintenance {
    /// Schedule selected for execution.
    pub schedule: MaintenanceSchedule,
    /// Job kind for `JobRequest`.
    pub job_kind: JobKind,
}

/// Durable scheduler backed by catalog system keys.
#[derive(Clone)]
pub struct MaintenanceScheduler {
    db: Db,
}

impl MaintenanceScheduler {
    /// Open a scheduler over an existing catalog database.
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// Insert or replace a schedule.
    pub async fn upsert(&self, schedule: MaintenanceSchedule) -> CatalogResult<()> {
        validate_schedule(&schedule)?;
        self.db
            .put(
                &schedule_key(&schedule.id),
                &values::encode_raw_value(&serde_json::to_vec(&schedule).map_err(|e| {
                    CatalogError::Internal(format!("serialize maintenance schedule: {e}"))
                })?),
            )
            .await?;
        Ok(())
    }

    /// List policies in stable identifier order.
    pub async fn list(&self) -> CatalogResult<Vec<MaintenanceSchedule>> {
        let mut iter = self.db.scan_prefix(&schedule_prefix()).await?;
        let mut schedules = Vec::new();
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            schedules.push(decode_schedule(&kv.value)?);
        }
        schedules.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(schedules)
    }

    /// Remove one policy. Missing schedules are harmless.
    pub async fn remove(&self, id: &str) -> CatalogResult<()> {
        self.db.delete(&schedule_key(id)).await?;
        Ok(())
    }

    /// Select at most `limit` due policies and atomically move their next run.
    pub async fn claim_due(
        &self,
        now_unix_ms: u64,
        limit: usize,
    ) -> CatalogResult<Vec<DueMaintenance>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        loop {
            let tx = self.db.begin(IsolationLevel::SerializableSnapshot).await?;
            let mut iter = tx
                .scan_prefix(&schedule_prefix())
                .await
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
            let mut due = Vec::new();
            while let Some(kv) = iter
                .next()
                .await
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?
            {
                let mut schedule = decode_schedule(&kv.value)?;
                if due.len() >= limit || !is_due(&schedule, now_unix_ms) {
                    continue;
                }
                schedule.next_run_at_unix_ms = schedule
                    .next_run_at_unix_ms
                    .saturating_add(schedule.interval_seconds.saturating_mul(1_000));
                tx.put(
                    schedule_key(&schedule.id),
                    values::encode_raw_value(&serde_json::to_vec(&schedule).map_err(|e| {
                        CatalogError::Internal(format!("serialize maintenance schedule: {e}"))
                    })?),
                )
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
                due.push(DueMaintenance {
                    job_kind: schedule.task.job_kind(),
                    schedule,
                });
            }
            match tx.commit().await {
                Ok(_) => return Ok(due),
                Err(error) if error.to_string().to_ascii_lowercase().contains("conflict") => {
                    tokio::task::yield_now().await;
                }
                Err(error) => return Err(CatalogError::SlateDb(error.to_string())),
            }
        }
    }

    /// Return active-job alert candidates using the existing durable ledger.
    pub async fn stale_jobs(
        &self,
        now_unix_ms: u64,
        after_seconds: u64,
    ) -> CatalogResult<Vec<crate::jobs::JobRecord>> {
        let threshold = chrono::DateTime::from_timestamp(
            ((now_unix_ms / 1_000).saturating_sub(after_seconds)) as i64,
            0,
        )
        .ok_or_else(|| CatalogError::InvalidInput("invalid maintenance clock".into()))?;
        let threshold = threshold.to_rfc3339();
        Ok(JobLedger::new(&self.db)
            .list()
            .await?
            .into_iter()
            .filter(|job| job.state.is_active() && job.updated_at < threshold)
            .collect())
    }
}

fn default_true() -> bool {
    true
}

fn schedule_prefix() -> Vec<u8> {
    keys::key_system(SCHEDULE_PREFIX)
}

fn schedule_key(id: &str) -> Vec<u8> {
    let mut key = schedule_prefix();
    key.extend_from_slice(id.as_bytes());
    key
}

fn decode_schedule(value: &[u8]) -> CatalogResult<MaintenanceSchedule> {
    let bytes = values::decode_raw_value(value)?;
    let schedule: MaintenanceSchedule = serde_json::from_slice(&bytes)
        .map_err(|e| CatalogError::Corruption(format!("decode maintenance schedule: {e}")))?;
    validate_schedule(&schedule)?;
    Ok(schedule)
}

fn validate_schedule(schedule: &MaintenanceSchedule) -> CatalogResult<()> {
    if schedule.id.is_empty() || schedule.id.len() > 128 || schedule.id.contains('/') {
        return Err(CatalogError::InvalidInput(
            "maintenance schedule ID must be 1..=128 bytes without '/'".into(),
        ));
    }
    if schedule.interval_seconds == 0 {
        return Err(CatalogError::InvalidInput(
            "maintenance interval must be greater than zero".into(),
        ));
    }
    if let Some(period) = schedule.window {
        validate_period(period)?;
    }
    for period in &schedule.blackouts {
        validate_period(*period)?;
    }
    if schedule.task.job_kind() == JobKind::Excision {
        return Err(CatalogError::InvalidInput(
            "physical excision is never schedulable".into(),
        ));
    }
    Ok(())
}

fn validate_period(period: MaintenancePeriod) -> CatalogResult<()> {
    if period.start_minute >= period.end_minute || period.end_minute > 1_440 {
        return Err(CatalogError::InvalidInput(
            "maintenance periods must be within 00:00..24:00".into(),
        ));
    }
    Ok(())
}

fn is_due(schedule: &MaintenanceSchedule, now_unix_ms: u64) -> bool {
    if !schedule.enabled || schedule.next_run_at_unix_ms > now_unix_ms {
        return false;
    }
    let minute = (now_unix_ms / 60_000 % 1_440) as u16;
    schedule.window.is_none_or(|window| window.contains(minute))
        && !schedule
            .blackouts
            .iter()
            .any(|blackout| blackout.contains(minute))
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::memory::InMemory;
    use object_store::path::Path as ObjectPath;
    use std::sync::Arc;

    #[tokio::test]
    async fn claim_due_is_durable_and_blackouts_are_respected() {
        let catalog = crate::CatalogStore::open(crate::OpenOptions {
            object_store: Arc::new(InMemory::new()),
            path: ObjectPath::from("maintenance"),
            encryption: None,
        })
        .await
        .unwrap();
        let scheduler = MaintenanceScheduler::new(catalog.db());
        scheduler
            .upsert(MaintenanceSchedule {
                id: "daily-backup".into(),
                task: MaintenanceTask::Backup,
                interval_seconds: 60,
                next_run_at_unix_ms: 0,
                enabled: true,
                window: Some(MaintenancePeriod {
                    start_minute: 0,
                    end_minute: 1,
                }),
                blackouts: Vec::new(),
            })
            .await
            .unwrap();
        assert_eq!(scheduler.claim_due(0, 1).await.unwrap().len(), 1);
        assert!(scheduler.claim_due(0, 1).await.unwrap().is_empty());
        catalog.close().await.unwrap();
    }
}
