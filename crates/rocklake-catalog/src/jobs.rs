//! Durable, bounded administrative job state.
//!
//! Job records live under the `TAG_SYSTEM | jobs:` namespace. They are kept
//! outside DuckLake table keys so maintenance progress cannot collide with or
//! get confused for catalog data.

use rocklake_core::tags::CATALOG_FORMAT_VERSION;
use rocklake_core::{keys, values};
use serde::{Deserialize, Serialize};
use slatedb::{Db, IsolationLevel};
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

use crate::error::{CatalogError, CatalogResult};

/// Current durable job-ledger schema version.
pub const JOB_LEDGER_FORMAT_VERSION: u32 = 1;
const JOBS_PREFIX: &[u8] = b"jobs:";
const MAX_JOB_PARAMETERS_BYTES: usize = 1024 * 1024;

/// Stable identifier for one administrative job.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JobId(String);

impl JobId {
    /// Generate a new job identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Return the identifier as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for JobId {
    type Err = CatalogError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value)
            .map(|_| Self(value.to_string()))
            .map_err(|_| CatalogError::InvalidInput("job id must be a UUID".to_string()))
    }
}

/// Long-running operation represented by the job ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    /// Create a snapshot-consistent catalog backup.
    Backup,
    /// Restore a catalog from a backup artifact.
    Restore,
    /// Export catalog rows.
    Export,
    /// Import catalog rows.
    Import,
    /// Verify catalog or referenced data-file integrity.
    Verification,
    /// Apply conservative catalog repairs.
    Repair,
    /// Advance the visibility retention floor.
    Retention,
    /// Physically remove facts below a retention floor.
    Excision,
    /// Find and optionally remove unreferenced data files.
    OrphanSweep,
    /// Create a full logical catalog checkpoint.
    Checkpoint,
    /// Rebuild catalog metadata from data files.
    Rebuild,
}

impl JobKind {
    /// Resource class required by this operation.
    pub const fn resource_class(self) -> JobResourceClass {
        match self {
            Self::Backup | Self::Export | Self::Verification => {
                JobResourceClass::AdministrativeOnline
            }
            Self::Restore
            | Self::Import
            | Self::Repair
            | Self::Retention
            | Self::Excision
            | Self::OrphanSweep
            | Self::Rebuild
            | Self::Checkpoint => JobResourceClass::OfflineExclusive,
        }
    }

    /// Whether two jobs may run against the same catalog concurrently.
    pub const fn conflicts_with(self, other: Self) -> bool {
        self.resource_class().conflicts_with(other.resource_class())
            || matches!(
                (self, other),
                (Self::Backup, Self::Retention)
                    | (Self::Retention, Self::Backup)
                    | (Self::Backup, Self::Excision)
                    | (Self::Excision, Self::Backup)
            )
    }
}

/// Admission class for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobResourceClass {
    /// Reserved for normal interactive requests.
    Interactive,
    /// May run while interactive requests are served.
    AdministrativeOnline,
    /// Requires exclusive access to the catalog.
    OfflineExclusive,
}

impl JobResourceClass {
    /// Whether two resource classes can be admitted together.
    pub const fn conflicts_with(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::OfflineExclusive, _) | (_, Self::OfflineExclusive)
        )
    }
}

/// Durable lifecycle state for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    /// Created but not yet started.
    Pending,
    /// Currently executing.
    Running,
    /// Cancellation was requested and the worker should stop at its next safe point.
    CancellationRequested,
    /// Stopped before completion.
    Cancelled,
    /// Completed and has an immutable terminal result.
    Completed,
    /// Failed; an operator may explicitly resume it from its last checkpoint.
    Failed,
}

impl JobState {
    /// Whether the job can conflict with a newly-created job.
    pub const fn is_active(self) -> bool {
        matches!(
            self,
            Self::Pending | Self::Running | Self::CancellationRequested
        )
    }

    /// Whether the state is terminal.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Cancelled | Self::Completed | Self::Failed)
    }
}

/// A resumable checkpoint written after a safe unit of work.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobCheckpoint {
    /// Monotonic checkpoint sequence.
    pub sequence: u64,
    /// Opaque operation-specific cursor.
    pub cursor: Option<String>,
    /// Snapshot associated with the checkpoint, when applicable.
    pub snapshot_id: Option<u64>,
}

/// Bounded progress counters for operator output.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobProgress {
    /// Number of logical items processed.
    pub items_done: u64,
    /// Expected item count when known without a full pre-scan.
    pub items_total: Option<u64>,
    /// Number of bytes processed.
    pub bytes_done: u64,
    /// Short human-readable phase or status message.
    pub message: Option<String>,
}

/// A terminal or retryable job failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobError {
    /// Stable error classification.
    pub code: String,
    /// Human-readable detail.
    pub message: String,
    /// Whether retrying from the checkpoint is safe.
    pub retryable: bool,
}

/// Request to create a durable job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRequest {
    /// Operation represented by the job.
    pub kind: JobKind,
    /// Catalog format observed before execution.
    pub catalog_format: u32,
    /// Snapshot captured when the operation started.
    pub starting_snapshot: Option<u64>,
    /// JSON parameters needed to reproduce the operation.
    pub parameters: serde_json::Value,
    /// Caller-provided retry identity.
    pub idempotency_key: Option<String>,
}

impl JobRequest {
    /// Build a request for the current catalog format.
    pub fn new(
        kind: JobKind,
        starting_snapshot: Option<u64>,
        parameters: serde_json::Value,
        idempotency_key: Option<String>,
    ) -> Self {
        Self {
            kind,
            catalog_format: CATALOG_FORMAT_VERSION,
            starting_snapshot,
            parameters,
            idempotency_key,
        }
    }
}

/// Complete durable state for one job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobRecord {
    /// Ledger schema version.
    pub ledger_version: u32,
    /// Job identifier.
    pub id: JobId,
    /// Operation kind.
    pub kind: JobKind,
    /// Admission class.
    pub resource_class: JobResourceClass,
    /// Lifecycle state.
    pub state: JobState,
    /// Catalog format observed at creation.
    pub catalog_format: u32,
    /// Snapshot captured at creation.
    pub starting_snapshot: Option<u64>,
    /// Reproducible operation parameters.
    pub parameters: serde_json::Value,
    /// Idempotency key, if supplied by the caller.
    pub idempotency_key: Option<String>,
    /// Last safe checkpoint.
    pub checkpoint: JobCheckpoint,
    /// Bounded progress counters.
    pub progress: JobProgress,
    /// Terminal result, if complete.
    pub terminal_result: Option<serde_json::Value>,
    /// Failure detail, if failed.
    pub error: Option<JobError>,
    /// Software version that created the record.
    pub software_version: String,
    /// RFC 3339 creation time.
    pub created_at: String,
    /// RFC 3339 last-update time.
    pub updated_at: String,
}

impl JobRecord {
    fn new(request: JobRequest) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            ledger_version: JOB_LEDGER_FORMAT_VERSION,
            id: JobId::new(),
            kind: request.kind,
            resource_class: request.kind.resource_class(),
            state: JobState::Pending,
            catalog_format: request.catalog_format,
            starting_snapshot: request.starting_snapshot,
            parameters: request.parameters,
            idempotency_key: request.idempotency_key,
            checkpoint: JobCheckpoint::default(),
            progress: JobProgress::default(),
            terminal_result: None,
            error: None,
            software_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

/// Durable job ledger backed by a catalog's maintenance namespace.
#[derive(Clone)]
pub struct JobLedger {
    db: Db,
}

impl JobLedger {
    /// Create a ledger handle without opening a second database.
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// Create a pending job, or return the existing job for an idempotency key.
    pub async fn create(&self, request: JobRequest) -> CatalogResult<JobRecord> {
        let encoded_parameters = serde_json::to_vec(&request.parameters)
            .map_err(|e| CatalogError::InvalidInput(format!("serialize job parameters: {e}")))?;
        if encoded_parameters.len() > MAX_JOB_PARAMETERS_BYTES {
            return Err(CatalogError::InvalidInput(format!(
                "job parameters exceed {MAX_JOB_PARAMETERS_BYTES} bytes"
            )));
        }
        if request
            .idempotency_key
            .as_deref()
            .is_some_and(|key| key.is_empty() || key.len() > 256)
        {
            return Err(CatalogError::InvalidInput(
                "idempotency key must contain 1..=256 bytes".to_string(),
            ));
        }

        loop {
            let tx = self.db.begin(IsolationLevel::SerializableSnapshot).await?;
            let existing = scan_records_tx(&tx).await?;
            if let Some(key) = request.idempotency_key.as_deref() {
                if let Some(record) = existing
                    .iter()
                    .find(|record| record.idempotency_key.as_deref() == Some(key))
                {
                    if record.kind != request.kind {
                        return Err(CatalogError::InvalidInput(format!(
                            "idempotency key already belongs to {}",
                            record.kind.as_str()
                        )));
                    }
                    if record.parameters != request.parameters {
                        return Err(CatalogError::InvalidInput(
                            "idempotency key was reused with different parameters".to_string(),
                        ));
                    }
                    return Ok(record.clone());
                }
            }
            if let Some(record) = existing
                .iter()
                .find(|record| record.state.is_active() && record.kind.conflicts_with(request.kind))
            {
                return Err(CatalogError::InvalidInput(format!(
                    "job {} ({}) conflicts with active {} job {}",
                    record.id,
                    record.kind.as_str(),
                    request.kind.as_str(),
                    record.id
                )));
            }

            let record = JobRecord::new(request.clone());
            let key = job_key(&record.id);
            let value = encode_record(&record)?;
            tx.put(&key, &value)
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
            match tx.commit().await {
                Ok(_) => return Ok(record),
                Err(error) if is_conflict(&error) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }

    /// Return one job by ID.
    pub async fn get(&self, id: &JobId) -> CatalogResult<Option<JobRecord>> {
        self.db
            .get(&job_key(id))
            .await?
            .map(|value| decode_record(&value))
            .transpose()
    }

    /// List jobs in newest-update-first order.
    pub async fn list(&self) -> CatalogResult<Vec<JobRecord>> {
        let mut jobs = scan_records(&self.db).await?;
        jobs.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(jobs)
    }

    /// Mark a pending job as running.
    pub async fn start(&self, id: &JobId) -> CatalogResult<JobRecord> {
        self.mutate(id, |record| match record.state {
            JobState::Pending => {
                record.state = JobState::Running;
                Ok(())
            }
            state => Err(CatalogError::InvalidInput(format!(
                "job {id} cannot start from {state:?}"
            ))),
        })
        .await
    }

    /// Persist progress and the last safe checkpoint for a running job.
    pub async fn update_progress(
        &self,
        id: &JobId,
        progress: JobProgress,
        checkpoint: JobCheckpoint,
    ) -> CatalogResult<JobRecord> {
        self.mutate(id, move |record| {
            if !matches!(
                record.state,
                JobState::Running | JobState::CancellationRequested
            ) {
                return Err(CatalogError::InvalidInput(format!(
                    "job {id} cannot update progress from {:?}",
                    record.state
                )));
            }
            if checkpoint.sequence < record.checkpoint.sequence {
                return Err(CatalogError::InvalidInput(
                    "job checkpoint sequence cannot move backwards".to_string(),
                ));
            }
            record.progress = progress.clone();
            record.checkpoint = checkpoint.clone();
            Ok(())
        })
        .await
    }

    /// Request cancellation. Workers must still acknowledge it at a safe point.
    pub async fn request_cancel(&self, id: &JobId) -> CatalogResult<JobRecord> {
        self.mutate(id, |record| match record.state {
            JobState::Pending | JobState::Running => {
                record.state = JobState::CancellationRequested;
                Ok(())
            }
            JobState::CancellationRequested | JobState::Cancelled => Ok(()),
            state => Err(CatalogError::InvalidInput(format!(
                "job {id} cannot be cancelled from {state:?}"
            ))),
        })
        .await
    }

    /// Acknowledge cancellation at a safe checkpoint.
    pub async fn mark_cancelled(&self, id: &JobId) -> CatalogResult<JobRecord> {
        self.mutate(id, |record| match record.state {
            JobState::Pending | JobState::CancellationRequested => {
                record.state = JobState::Cancelled;
                Ok(())
            }
            JobState::Cancelled => Ok(()),
            state => Err(CatalogError::InvalidInput(format!(
                "job {id} cannot finish cancellation from {state:?}"
            ))),
        })
        .await
    }

    /// Complete a running job with an immutable terminal result.
    pub async fn complete(
        &self,
        id: &JobId,
        result: serde_json::Value,
    ) -> CatalogResult<JobRecord> {
        self.mutate(id, move |record| match record.state {
            JobState::Running => {
                record.state = JobState::Completed;
                record.terminal_result = Some(result.clone());
                record.error = None;
                Ok(())
            }
            JobState::Completed => Ok(()),
            state => Err(CatalogError::InvalidInput(format!(
                "job {id} cannot complete from {state:?}"
            ))),
        })
        .await
    }

    /// Fail a running job while retaining its last safe checkpoint.
    pub async fn fail(&self, id: &JobId, error: JobError) -> CatalogResult<JobRecord> {
        self.mutate(id, move |record| match record.state {
            JobState::Pending | JobState::Running | JobState::CancellationRequested => {
                record.state = JobState::Failed;
                record.error = Some(error.clone());
                Ok(())
            }
            JobState::Failed => Ok(()),
            state => Err(CatalogError::InvalidInput(format!(
                "job {id} cannot fail from {state:?}"
            ))),
        })
        .await
    }

    /// Explicitly requeue a failed, cancelled, or abandoned running job.
    pub async fn resume(&self, id: &JobId) -> CatalogResult<JobRecord> {
        self.mutate(id, |record| match record.state {
            JobState::Cancelled
            | JobState::Failed
            | JobState::Running
            | JobState::CancellationRequested => {
                record.state = JobState::Pending;
                record.error = None;
                record.terminal_result = None;
                Ok(())
            }
            JobState::Pending => Ok(()),
            JobState::Completed => Err(CatalogError::InvalidInput(
                "completed jobs cannot be resumed".to_string(),
            )),
        })
        .await
    }

    async fn mutate<F>(&self, id: &JobId, operation: F) -> CatalogResult<JobRecord>
    where
        F: Fn(&mut JobRecord) -> CatalogResult<()> + Send + Sync,
    {
        loop {
            let tx = self.db.begin(IsolationLevel::SerializableSnapshot).await?;
            let value = tx
                .get(&job_key(id))
                .await
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?
                .ok_or_else(|| CatalogError::NotFound(format!("job {id}")))?;
            let mut record = decode_record(&value)?;
            operation(&mut record)?;
            record.updated_at = chrono::Utc::now().to_rfc3339();
            let encoded = encode_record(&record)?;
            tx.put(job_key(id), &encoded)
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
            match tx.commit().await {
                Ok(_) => return Ok(record),
                Err(error) if is_conflict(&error) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
}

/// In-process admission pools that keep maintenance work from consuming all query capacity.
#[derive(Clone)]
pub struct JobAdmission {
    interactive: Arc<Semaphore>,
    administrative_online: Arc<Semaphore>,
    offline_exclusive: Arc<Semaphore>,
}

impl JobAdmission {
    /// Build admission pools. Each value must be greater than zero.
    pub fn new(
        interactive: usize,
        administrative_online: usize,
        offline_exclusive: usize,
    ) -> CatalogResult<Self> {
        if interactive == 0 || administrative_online == 0 || offline_exclusive == 0 {
            return Err(CatalogError::InvalidInput(
                "job admission capacities must be greater than zero".to_string(),
            ));
        }
        Ok(Self {
            interactive: Arc::new(Semaphore::new(interactive)),
            administrative_online: Arc::new(Semaphore::new(administrative_online)),
            offline_exclusive: Arc::new(Semaphore::new(offline_exclusive)),
        })
    }

    /// Acquire one permit for a resource class.
    pub async fn acquire(&self, class: JobResourceClass) -> CatalogResult<OwnedSemaphorePermit> {
        let semaphore = match class {
            JobResourceClass::Interactive => &self.interactive,
            JobResourceClass::AdministrativeOnline => &self.administrative_online,
            JobResourceClass::OfflineExclusive => &self.offline_exclusive,
        };
        semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| CatalogError::Internal("job admission pool closed".to_string()))
    }
}

impl JobKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Backup => "backup",
            Self::Restore => "restore",
            Self::Export => "export",
            Self::Import => "import",
            Self::Verification => "verification",
            Self::Repair => "repair",
            Self::Retention => "retention",
            Self::Excision => "excision",
            Self::OrphanSweep => "orphan_sweep",
            Self::Rebuild => "rebuild",
            Self::Checkpoint => "checkpoint",
        }
    }
}

fn jobs_prefix() -> Vec<u8> {
    keys::key_system(JOBS_PREFIX)
}

fn job_key(id: &JobId) -> Vec<u8> {
    let mut key = jobs_prefix();
    key.extend_from_slice(id.as_str().as_bytes());
    key
}

fn encode_record(record: &JobRecord) -> CatalogResult<Vec<u8>> {
    let bytes = serde_json::to_vec(record)
        .map_err(|e| CatalogError::Internal(format!("serialize job record: {e}")))?;
    Ok(values::encode_raw_value(&bytes))
}

fn decode_record(value: &[u8]) -> CatalogResult<JobRecord> {
    let bytes = values::decode_raw_value(value)?;
    let record: JobRecord = serde_json::from_slice(&bytes)
        .map_err(|e| CatalogError::Corruption(format!("decode job record: {e}")))?;
    if record.ledger_version != JOB_LEDGER_FORMAT_VERSION {
        return Err(CatalogError::Corruption(format!(
            "unsupported job ledger version {} (expected {})",
            record.ledger_version, JOB_LEDGER_FORMAT_VERSION
        )));
    }
    Ok(record)
}

async fn scan_records(db: &Db) -> CatalogResult<Vec<JobRecord>> {
    let mut iter = db.scan_prefix(&jobs_prefix()).await?;
    let mut records = Vec::new();
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        records.push(decode_record(&kv.value)?);
    }
    Ok(records)
}

async fn scan_records_tx(tx: &slatedb::DbTransaction) -> CatalogResult<Vec<JobRecord>> {
    let mut iter = tx
        .scan_prefix(&jobs_prefix())
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
    let mut records = Vec::new();
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        records.push(decode_record(&kv.value)?);
    }
    Ok(records)
}

fn is_conflict(error: &slatedb::Error) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("conflict") || message.contains("concurrent") || message.contains("cas")
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::path::Path as ObjectPath;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let store =
            Arc::new(object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap());
        let catalog = crate::CatalogStore::open(crate::OpenOptions {
            object_store: store,
            path: ObjectPath::from("catalog"),
            encryption: None,
        })
        .await
        .unwrap();
        catalog.close().await.unwrap();
        let store =
            Arc::new(object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap());
        let db = Db::open(ObjectPath::from("catalog"), store).await.unwrap();
        (dir, db)
    }

    #[tokio::test]
    async fn job_lifecycle_persists_checkpoint_and_resume() {
        let (_dir, db) = db().await;
        let ledger = JobLedger::new(&db);
        let request = JobRequest::new(
            JobKind::Export,
            Some(7),
            serde_json::json!({"path":"out.ndjson"}),
            Some("export-7".to_string()),
        );
        let created = ledger.create(request.clone()).await.unwrap();
        assert_eq!(ledger.create(request).await.unwrap().id, created.id);
        ledger.start(&created.id).await.unwrap();
        ledger
            .update_progress(
                &created.id,
                JobProgress {
                    items_done: 3,
                    items_total: Some(10),
                    bytes_done: 42,
                    message: Some("rows".to_string()),
                },
                JobCheckpoint {
                    sequence: 1,
                    cursor: Some("0b:03".to_string()),
                    snapshot_id: Some(7),
                },
            )
            .await
            .unwrap();
        ledger
            .fail(
                &created.id,
                JobError {
                    code: "transient".to_string(),
                    message: "retry".to_string(),
                    retryable: true,
                },
            )
            .await
            .unwrap();
        let resumed = ledger.resume(&created.id).await.unwrap();
        assert_eq!(resumed.state, JobState::Pending);
        assert_eq!(resumed.checkpoint.sequence, 1);
        db.close().await.unwrap();
    }

    #[tokio::test]
    async fn offline_jobs_conflict_and_cancel_is_explicit() {
        let (_dir, db) = db().await;
        let ledger = JobLedger::new(&db);
        let first = ledger
            .create(JobRequest::new(
                JobKind::Restore,
                None,
                serde_json::json!({}),
                None,
            ))
            .await
            .unwrap();
        ledger.start(&first.id).await.unwrap();
        let second = ledger
            .create(JobRequest::new(
                JobKind::Rebuild,
                None,
                serde_json::json!({}),
                None,
            ))
            .await;
        assert!(second.is_err());
        assert_eq!(
            ledger.request_cancel(&first.id).await.unwrap().state,
            JobState::CancellationRequested
        );
        assert_eq!(
            ledger.mark_cancelled(&first.id).await.unwrap().state,
            JobState::Cancelled
        );
        db.close().await.unwrap();
    }
}
