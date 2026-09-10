//! Machine-readable recovery-drill contracts.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Current recovery report schema.
pub const RECOVERY_REPORT_SCHEMA_VERSION: u32 = 1;

/// Failure scenario covered by a recovery drill.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryDrill {
    /// Process loss with durable catalog and backup state intact.
    LostProcess,
    /// Registry prefix loss followed by registry restore.
    LostRegistryPrefix,
    /// Accidental route deletion followed by re-registration.
    AccidentalRouteDeletion,
    /// Damaged catalog prefix followed by restore to a new location.
    DamagedCatalogPrefix,
    /// Credentials unavailable during recovery.
    LostCredentials,
    /// Region-level recovery into new prefixes.
    RegionRestore,
}

/// Machine-readable result emitted by every recovery drill.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecoveryReport {
    /// Report schema version.
    pub schema_version: u32,
    /// Drill scenario.
    pub drill: RecoveryDrill,
    /// RFC 3339 start time.
    pub started_at: String,
    /// RFC 3339 completion time.
    pub completed_at: String,
    /// Whether the drill completed successfully.
    pub status: String,
    /// Measured durable-commit RPO in seconds.
    pub measured_rpo_seconds: u64,
    /// Measured recovery RTO in seconds.
    pub measured_rto_seconds: u64,
    /// Whether metadata and referenced-data verification passed.
    pub verification_passed: bool,
    /// Short bounded operator detail.
    pub details: String,
}

impl RecoveryReport {
    /// Create a completed report from measured drill results.
    pub fn completed(
        drill: RecoveryDrill,
        started_at: impl Into<String>,
        measured_rpo_seconds: u64,
        measured_rto_seconds: u64,
        verification_passed: bool,
        details: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: RECOVERY_REPORT_SCHEMA_VERSION,
            drill,
            started_at: started_at.into(),
            completed_at: chrono::Utc::now().to_rfc3339(),
            status: if verification_passed {
                "passed"
            } else {
                "failed"
            }
            .into(),
            measured_rpo_seconds,
            measured_rto_seconds,
            verification_passed,
            details: details.into(),
        }
    }

    /// Write the report as pretty JSON.
    pub async fn write_json(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        tokio::fs::write(
            path,
            serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?,
        )
        .await
    }
}
