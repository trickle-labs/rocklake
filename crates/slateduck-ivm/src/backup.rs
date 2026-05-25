//! State store backup and restore.
//!
//! Provides compaction-pin based backups (a manifest recording pinned SSTs + frontier)
//! and restore operations that reset a shard's active state to a pinned checkpoint.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::dag::FrontierClock;

/// A backup manifest for a shard's state store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    /// Timestamp of backup creation (Unix millis).
    pub timestamp_ms: u64,
    /// Matview ID.
    pub matview_id: u64,
    /// Shard ID.
    pub shard_id: u32,
    /// The frontier at backup time — on restore, the worker resumes from here.
    pub frontier: FrontierClock,
    /// List of pinned SST file references.
    pub pinned_ssts: Vec<String>,
    /// SlateDB checkpoint ID.
    pub checkpoint_id: u64,
    /// Backup state prefix path.
    pub path: String,
}

/// Result of a restore operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreResult {
    /// Restore succeeded; worker should resume from the given frontier.
    Success { frontier: FrontierClock },
    /// The backup manifest was not found.
    ManifestNotFound,
    /// The state store was missing on lease claim.
    StateStoreMissing,
}

/// Backup configuration.
#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// The state prefix for the matview.
    pub state_prefix: String,
    /// Whether auto-rebuild on state loss is enabled.
    pub auto_rebuild_on_loss: bool,
}

impl BackupManifest {
    /// Create a new backup manifest.
    pub fn new(
        matview_id: u64,
        shard_id: u32,
        frontier: FrontierClock,
        pinned_ssts: Vec<String>,
        checkpoint_id: u64,
        state_prefix: &str,
    ) -> Self {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let path = format!(
            "{}/backups/{}/{}/{}.json",
            state_prefix.trim_end_matches('/'),
            matview_id,
            shard_id,
            timestamp_ms,
        );

        Self {
            timestamp_ms,
            matview_id,
            shard_id,
            frontier,
            pinned_ssts,
            checkpoint_id,
            path,
        }
    }

    /// Serialize manifest to JSON bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).unwrap_or_default()
    }

    /// Deserialize from JSON bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }

    /// Path where this manifest should be stored.
    pub fn manifest_path(&self) -> &str {
        &self.path
    }
}

/// Perform a backup: create manifest and pin SSTs.
///
/// In production this would interact with SlateDB's Checkpoint API.
/// Here we create the manifest with the frontier for restore validation.
pub fn create_backup(
    state_prefix: &str,
    matview_id: u64,
    shard_id: u32,
    frontier: FrontierClock,
    current_ssts: Vec<String>,
) -> BackupManifest {
    let checkpoint_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    BackupManifest::new(
        matview_id,
        shard_id,
        frontier,
        current_ssts,
        checkpoint_id,
        state_prefix,
    )
}

/// Attempt restore from a backup manifest.
///
/// On success, returns the frontier the worker should resume from.
/// The worker skips CDC events with seq ≤ frontier[source].
pub fn restore_from_backup(manifest: &BackupManifest) -> RestoreResult {
    // Validate manifest is non-empty.
    if manifest.checkpoint_id == 0 {
        return RestoreResult::ManifestNotFound;
    }
    RestoreResult::Success {
        frontier: manifest.frontier.clone(),
    }
}

/// Handle state store missing at lease-claim time.
///
/// If `auto_rebuild_on_loss` is false (default), returns StateStoreMissing
/// and the worker should wait for operator intervention.
pub fn handle_missing_state_store(config: &BackupConfig) -> RestoreResult {
    if config.auto_rebuild_on_loss {
        // Auto-rebuild: return empty frontier (full recompute).
        RestoreResult::Success {
            frontier: FrontierClock::new(),
        }
    } else {
        tracing::warn!(
            "state_store_missing: state store not found at prefix '{}'. \
             Waiting for operator intervention. Use --auto-rebuild-on-loss to auto-rebuild.",
            config.state_prefix
        );
        RestoreResult::StateStoreMissing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_manifest_roundtrip() {
        let mut frontier = FrontierClock::new();
        frontier.advance(1, 100);
        frontier.advance(2, 200);

        let manifest = create_backup(
            "s3://bucket/state",
            42,
            3,
            frontier.clone(),
            vec!["sst-001.sst".to_string(), "sst-002.sst".to_string()],
        );

        let bytes = manifest.to_bytes();
        let restored = BackupManifest::from_bytes(&bytes).unwrap();
        assert_eq!(restored.matview_id, 42);
        assert_eq!(restored.shard_id, 3);
        assert_eq!(restored.frontier.get(1), 100);
        assert_eq!(restored.frontier.get(2), 200);
        assert_eq!(restored.pinned_ssts.len(), 2);
    }

    #[test]
    fn restore_from_valid_backup() {
        let mut frontier = FrontierClock::new();
        frontier.advance(1, 500);
        let manifest = create_backup("s3://bucket/state", 1, 0, frontier.clone(), vec![]);
        let result = restore_from_backup(&manifest);
        assert_eq!(
            result,
            RestoreResult::Success {
                frontier: frontier.clone()
            }
        );
    }

    #[test]
    fn missing_state_store_without_auto_rebuild() {
        let config = BackupConfig {
            state_prefix: "s3://bucket/state".to_string(),
            auto_rebuild_on_loss: false,
        };
        let result = handle_missing_state_store(&config);
        assert_eq!(result, RestoreResult::StateStoreMissing);
    }

    #[test]
    fn missing_state_store_with_auto_rebuild() {
        let config = BackupConfig {
            state_prefix: "s3://bucket/state".to_string(),
            auto_rebuild_on_loss: true,
        };
        let result = handle_missing_state_store(&config);
        assert_eq!(
            result,
            RestoreResult::Success {
                frontier: FrontierClock::new()
            }
        );
    }
}
