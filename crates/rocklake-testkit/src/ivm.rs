//! IvmWorkerHarness: launches and supervises IVM worker processes for tests.
//!
//! The harness is intentionally lightweight: it manages worker processes,
//! passes through the MinIO catalog configuration, and offers polling helpers
//! for lag and output assertions. The actual worker binary is injected via the
//! `ROCKLAKE_IVM_BINARY` environment variable so the harness stays reusable
//! across process layouts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Child;

#[cfg(feature = "minio-tests")]
use crate::MinioHarness;

/// Harness for one or more IVM worker processes.
pub struct IvmWorkerHarness {
    processes: HashMap<String, Child>,
    pub catalog_path: String,
    pub state_prefix: String,
}

impl IvmWorkerHarness {
    /// Start a single worker and return a harness that can supervise it.
    #[cfg(feature = "minio-tests")]
    pub async fn start_worker(
        id: &str,
        shard_limit: u32,
        minio: &MinioHarness,
    ) -> Result<Self, IvmWorkerHarnessError> {
        let mut harness = Self {
            processes: HashMap::new(),
            catalog_path: format!("{}/catalog", minio.bucket),
            state_prefix: format!("{}_ivm_state", minio.bucket),
        };
        harness.add_worker_internal(id, shard_limit, minio).await?;
        Ok(harness)
    }

    /// Add a new worker process to an existing harness.
    #[cfg(feature = "minio-tests")]
    pub async fn add_worker(
        &mut self,
        id: &str,
        shard_limit: u32,
        minio: &MinioHarness,
    ) -> Result<(), IvmWorkerHarnessError> {
        self.add_worker_internal(id, shard_limit, minio).await
    }

    /// Kill a worker process by ID.
    pub async fn kill_worker(&mut self, id: &str) -> Result<(), IvmWorkerHarnessError> {
        if let Some(mut child) = self.processes.remove(id) {
            child
                .kill()
                .await
                .map_err(|e| IvmWorkerHarnessError::Process(e.to_string()))?;
        }
        Ok(())
    }

    /// Poll a lag status file until the reported lag drops below the threshold.
    pub async fn wait_lag_below_ms(
        &self,
        matview: &str,
        threshold_ms: u64,
        timeout: Duration,
    ) -> Result<(), IvmWorkerHarnessError> {
        let status_path = self.status_path(matview);
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if std::time::Instant::now() > deadline {
                return Err(IvmWorkerHarnessError::Timeout(format!(
                    "timed out waiting for {matview} lag below {threshold_ms}ms"
                )));
            }

            if let Ok(content) = tokio::fs::read_to_string(&status_path).await {
                if let Ok(status) = serde_json::from_str::<serde_json::Value>(&content) {
                    let lag_ms = status
                        .get("matview_lag_ms")
                        .or_else(|| status.get("lag_ms"))
                        .and_then(|value| value.as_u64())
                        .unwrap_or(u64::MAX);
                    if lag_ms < threshold_ms {
                        return Ok(());
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Assert the output file for a materialized view matches the expected SQL text.
    pub async fn assert_output_matches(
        &self,
        matview: &str,
        expected_sql: &str,
    ) -> Result<(), IvmWorkerHarnessError> {
        let output_path = self.output_path(matview);
        let actual = tokio::fs::read_to_string(&output_path).await.map_err(|e| {
            IvmWorkerHarnessError::Output(format!("{}: {e}", output_path.display()))
        })?;
        if actual.trim() == expected_sql.trim() {
            Ok(())
        } else {
            Err(IvmWorkerHarnessError::Output(format!(
                "output mismatch for {matview}: expected {:?}, got {:?}",
                expected_sql.trim(),
                actual.trim()
            )))
        }
    }

    async fn add_worker_internal(
        &mut self,
        id: &str,
        shard_limit: u32,
        minio: &MinioHarness,
    ) -> Result<(), IvmWorkerHarnessError> {
        let binary = std::env::var("ROCKLAKE_IVM_BINARY").map_err(|_| {
            IvmWorkerHarnessError::Config(
                "ROCKLAKE_IVM_BINARY must point to an IVM worker executable".to_string(),
            )
        })?;

        let mut command = tokio::process::Command::new(binary);
        command
            .env("ROCKLAKE_WORKER_ID", id)
            .env("ROCKLAKE_SHARD_LIMIT", shard_limit.to_string())
            .env("ROCKLAKE_MINIO_ENDPOINT", &minio.endpoint)
            .env("ROCKLAKE_MINIO_ACCESS_KEY", &minio.access_key)
            .env("ROCKLAKE_MINIO_SECRET_KEY", &minio.secret_key)
            .env("ROCKLAKE_MINIO_BUCKET", &minio.bucket)
            .env("ROCKLAKE_CATALOG_PATH", &self.catalog_path)
            .env("ROCKLAKE_STATE_PREFIX", &self.state_prefix)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = command
            .spawn()
            .map_err(|e| IvmWorkerHarnessError::Process(e.to_string()))?;
        self.processes.insert(id.to_string(), child);
        Ok(())
    }

    fn status_path(&self, matview: &str) -> PathBuf {
        Path::new(&self.state_prefix).join(format!("{matview}.json"))
    }

    fn output_path(&self, matview: &str) -> PathBuf {
        Path::new(&self.state_prefix).join(format!("{matview}.sql"))
    }
}

/// Errors returned by `IvmWorkerHarness`.
#[derive(Debug, thiserror::Error)]
pub enum IvmWorkerHarnessError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("process error: {0}")]
    Process(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("output error: {0}")]
    Output(String),
}
