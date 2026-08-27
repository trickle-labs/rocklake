//! Checkpoint management: create, list, and restore catalog checkpoints.
//!
//! Thin wrapper around SlateDB's checkpoint functionality.

#![allow(missing_docs)]

use rocklake_core::keys;
use rocklake_core::rows::*;
use rocklake_core::tags::*;
use rocklake_core::values;
use slatedb::{Db, IsolationLevel};

use crate::error::{CatalogError, CatalogResult};

/// Information about a checkpoint.
#[derive(Debug, Clone)]
pub struct CheckpointInfo {
    /// Checkpoint ID (timestamp-based).
    pub id: u64,
    /// When the checkpoint was created.
    pub created_at: String,
    /// Snapshot ID at checkpoint time.
    pub snapshot_id: u64,
    /// Human-readable label.
    pub label: Option<String>,
    /// Snapshot created by a restore operation, if this is its result.
    pub restore_snapshot_id: Option<u64>,
}

/// Checkpoint metadata stored under system keys.
#[derive(Clone, PartialEq, prost::Message)]
pub struct CheckpointMetadata {
    #[prost(uint64, tag = "1")]
    pub id: u64,
    #[prost(string, tag = "2")]
    pub created_at: String,
    #[prost(uint64, tag = "3")]
    pub snapshot_id: u64,
    #[prost(string, optional, tag = "4")]
    pub label: Option<String>,
    /// Version of the full-state checkpoint representation.
    #[prost(uint32, tag = "5")]
    pub state_version: u32,
}

/// Create a new checkpoint of the current catalog state.
pub async fn create_checkpoint(db: &Db, label: Option<&str>) -> CatalogResult<CheckpointInfo> {
    let created_at = chrono::Utc::now().to_rfc3339();

    loop {
        let tx = db
            .begin(IsolationLevel::SerializableSnapshot)
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?;

        let next_snapshot = read_counter_in_tx(&tx, COUNTER_NEXT_SNAPSHOT_ID, 1).await?;
        let current_snapshot = next_snapshot.saturating_sub(1);
        let next_checkpoint = read_counter_in_tx(&tx, COUNTER_NEXT_CHECKPOINT_ID, 0).await?;
        let clock_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let id = clock_id.max(next_checkpoint);
        let next_id = id
            .checked_add(1)
            .ok_or_else(|| CatalogError::Internal("checkpoint ID overflow".to_string()))?;

        // ponytail: duplicate the logical key/value set per checkpoint; use native
        // SlateDB snapshots if checkpoint volume makes this storage-bound.
        let mut state = Vec::new();
        let mut iter = tx
            .scan_prefix(&[])
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            if !is_checkpoint_state_key(&kv.key) {
                state.push((kv.key.to_vec(), kv.value.to_vec()));
            }
        }

        let meta = CheckpointMetadata {
            id,
            created_at: created_at.clone(),
            snapshot_id: current_snapshot,
            label: label.map(str::to_string),
            state_version: 1,
        };
        tx.put(
            &keys::key_counter(COUNTER_NEXT_CHECKPOINT_ID),
            values::encode_counter(next_id),
        )
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        tx.put(&checkpoint_key(id), values::encode_value(&meta))
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        for (key, value) in state {
            tx.put(&checkpoint_state_key(id, &key), value)
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        }

        match tx.commit().await {
            Ok(_) => {
                return Ok(CheckpointInfo {
                    id,
                    created_at,
                    snapshot_id: current_snapshot,
                    label: label.map(str::to_string),
                    restore_snapshot_id: None,
                });
            }
            Err(e) if e.to_string().to_ascii_lowercase().contains("conflict") => continue,
            Err(e) => return Err(CatalogError::SlateDb(e.to_string())),
        }
    }
}

/// List all available checkpoints.
pub async fn list_checkpoints(db: &Db) -> CatalogResult<Vec<CheckpointInfo>> {
    let prefix = checkpoint_prefix();
    let mut checkpoints = Vec::new();
    let mut iter = db.scan_prefix(&prefix).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let meta: CheckpointMetadata = values::decode_value(&kv.value)?;
        checkpoints.push(CheckpointInfo {
            id: meta.id,
            created_at: meta.created_at,
            snapshot_id: meta.snapshot_id,
            label: meta.label,
            restore_snapshot_id: None,
        });
    }
    checkpoints.sort_by_key(|c| c.id);
    Ok(checkpoints)
}

/// Restore the checkpoint's complete logical catalog state as a fresh snapshot.
/// Snapshot counters remain monotonic so new writes cannot reuse historical IDs.
pub async fn restore_checkpoint(db: &Db, checkpoint_id: u64) -> CatalogResult<CheckpointInfo> {
    loop {
        let tx = db
            .begin(IsolationLevel::SerializableSnapshot)
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        let data = tx
            .get(checkpoint_key(checkpoint_id))
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
            .ok_or_else(|| CatalogError::NotFound(format!("checkpoint {checkpoint_id}")))?;
        let meta: CheckpointMetadata = values::decode_value(&data)?;
        if meta.state_version != 1 {
            return Err(CatalogError::InvalidInput(format!(
                "checkpoint {checkpoint_id} predates full-state restore; create a new checkpoint"
            )));
        }

        let current_next_snapshot = read_counter_in_tx(&tx, COUNTER_NEXT_SNAPSHOT_ID, 1).await?;
        let restore_snapshot_id = current_next_snapshot.max(
            meta.snapshot_id
                .checked_add(1)
                .ok_or_else(|| CatalogError::Internal("snapshot ID overflow".to_string()))?,
        );
        let next_snapshot_id = restore_snapshot_id
            .checked_add(1)
            .ok_or_else(|| CatalogError::Internal("snapshot counter overflow".to_string()))?;
        let schema_version =
            checkpoint_schema_version(&tx, checkpoint_id, meta.snapshot_id).await?;

        let state_prefix = checkpoint_state_prefix(checkpoint_id);
        let mut checkpoint_state = Vec::new();
        let mut state_iter = tx
            .scan_prefix(&state_prefix)
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        while let Some(kv) = state_iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            checkpoint_state.push((kv.key[state_prefix.len()..].to_vec(), kv.value.to_vec()));
        }

        let mut current_state_keys = Vec::new();
        let mut current_iter = tx
            .scan_prefix(&[])
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        while let Some(kv) = current_iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            if is_checkpoint_state_key(&kv.key) {
                continue;
            }
            current_state_keys.push(kv.key.to_vec());
        }
        for key in current_state_keys {
            tx.delete(&key)
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        }
        for (key, value) in checkpoint_state {
            tx.put(&key, value)
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        }

        let next_catalog_id = read_counter_in_tx(&tx, COUNTER_NEXT_CATALOG_ID, 1).await?;
        let next_file_id = read_counter_in_tx(&tx, COUNTER_NEXT_FILE_ID, 1).await?;
        let snapshot = SnapshotRow {
            snapshot_id: restore_snapshot_id,
            schema_version,
            snapshot_time: chrono::Utc::now().to_rfc3339(),
            author: Some("rocklake".to_string()),
            message: Some(format!("restore checkpoint {checkpoint_id}")),
            next_catalog_id: Some(next_catalog_id),
            next_file_id: Some(next_file_id),
        };
        let changes = SnapshotChangesRow {
            snapshot_id: restore_snapshot_id,
            change_type: "restore".to_string(),
            change_info: Some(format!("checkpoint_id={checkpoint_id}")),
            schema_id: None,
            table_id: None,
            author: Some("rocklake".to_string()),
            commit_message: Some(format!("restore checkpoint {checkpoint_id}")),
            commit_extra_info: None,
            changes_made: Some("full logical catalog state restored".to_string()),
        };
        tx.put(
            &keys::key_snapshot(restore_snapshot_id),
            values::encode_value(&snapshot),
        )
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        tx.put(
            &keys::key_snapshot_changes(restore_snapshot_id),
            values::encode_value(&changes),
        )
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        tx.put(
            &keys::key_counter(COUNTER_NEXT_SNAPSHOT_ID),
            values::encode_counter(next_snapshot_id),
        )
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?;

        match tx.commit().await {
            Ok(_) => {
                return Ok(CheckpointInfo {
                    id: meta.id,
                    created_at: meta.created_at,
                    snapshot_id: meta.snapshot_id,
                    label: meta.label,
                    restore_snapshot_id: Some(restore_snapshot_id),
                });
            }
            Err(e) if e.to_string().to_ascii_lowercase().contains("conflict") => continue,
            Err(e) => return Err(CatalogError::SlateDb(e.to_string())),
        }
    }
}

// ─── Checkpoint Pin API ────────────────────────────────────────────────────

/// Information about a named checkpoint pin.
#[derive(Debug, Clone)]
pub struct CheckpointPin {
    /// User-assigned name for this pin.
    pub name: String,
    /// The `dl_snapshot_id` this pin is anchored to.
    pub snapshot_id: u64,
    /// RFC-3339 creation timestamp.
    pub created_at: String,
}

/// Pin a named checkpoint at a specific `dl_snapshot_id`.
///
/// The pin is stored under `TAG_SYSTEM | "checkpoint-pin:" | name`.
/// It survives process restart and prevents GC from advancing past the
/// pinned snapshot.
pub async fn pin_checkpoint(db: &Db, name: &str, snapshot_id: u64) -> CatalogResult<CheckpointPin> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let key = checkpoint_pin_key(name);

    // Re-use CheckpointMetadata with id=0 (not used for pins) and label=name.
    let meta = CheckpointMetadata {
        id: 0,
        created_at: created_at.clone(),
        snapshot_id,
        label: Some(name.to_string()),
        state_version: 0,
    };

    let value = values::encode_value(&meta);
    loop {
        let tx = db
            .begin(IsolationLevel::SerializableSnapshot)
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        tx.put(&key, value.clone())
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        match tx.commit().await {
            Ok(_) => break,
            Err(e) if e.to_string().to_ascii_lowercase().contains("conflict") => continue,
            Err(e) => return Err(CatalogError::SlateDb(e.to_string())),
        }
    }

    Ok(CheckpointPin {
        name: name.to_string(),
        snapshot_id,
        created_at,
    })
}

/// Remove a named checkpoint pin.
///
/// Returns `CatalogError::NotFound` if no pin with the given name exists.
pub async fn unpin_checkpoint(db: &Db, name: &str) -> CatalogResult<()> {
    let key = checkpoint_pin_key(name);
    loop {
        let tx = db
            .begin(IsolationLevel::SerializableSnapshot)
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        tx.get(&key)
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
            .ok_or_else(|| CatalogError::NotFound(format!("checkpoint pin '{name}'")))?;
        tx.delete(&key)
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
        match tx.commit().await {
            Ok(_) => return Ok(()),
            Err(e) if e.to_string().to_ascii_lowercase().contains("conflict") => continue,
            Err(e) => return Err(CatalogError::SlateDb(e.to_string())),
        }
    }
}

/// List all named checkpoint pins.
pub async fn list_checkpoint_pins(db: &Db) -> CatalogResult<Vec<CheckpointPin>> {
    let prefix = checkpoint_pin_prefix();
    let mut pins = Vec::new();
    let mut iter = db.scan_prefix(&prefix).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let meta: CheckpointMetadata = values::decode_value(&kv.value)?;
        // Extract the name from the key suffix (skip prefix bytes).
        let prefix_len = prefix.len();
        let name = String::from_utf8_lossy(&kv.key[prefix_len..]).to_string();
        pins.push(CheckpointPin {
            name,
            snapshot_id: meta.snapshot_id,
            created_at: meta.created_at,
        });
    }
    pins.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(pins)
}

// ─── Helpers ───────────────────────────────────────────────────────────────

fn checkpoint_prefix() -> Vec<u8> {
    let mut buf = Vec::with_capacity(1 + 11);
    buf.push(TAG_SYSTEM);
    buf.extend_from_slice(b"checkpoint:");
    buf
}

fn checkpoint_key(id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(1 + 11 + 8);
    buf.push(TAG_SYSTEM);
    buf.extend_from_slice(b"checkpoint:");
    buf.extend_from_slice(&id.to_be_bytes());
    buf
}

fn checkpoint_state_prefix(id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(1 + 17 + 8);
    buf.push(TAG_SYSTEM);
    buf.extend_from_slice(b"checkpoint-state:");
    buf.extend_from_slice(&id.to_be_bytes());
    buf
}

fn checkpoint_state_key(id: u64, key: &[u8]) -> Vec<u8> {
    let mut buf = checkpoint_state_prefix(id);
    buf.extend_from_slice(key);
    buf
}

fn is_checkpoint_state_key(key: &[u8]) -> bool {
    matches!(key.first(), Some(&TAG_SYSTEM) | Some(&TAG_COUNTERS))
}

async fn read_counter_in_tx(
    tx: &slatedb::DbTransaction,
    counter: u8,
    default: u64,
) -> CatalogResult<u64> {
    Ok(tx
        .get(keys::key_counter(counter))
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        .map(|data| values::decode_counter(&data))
        .transpose()
        .map(|value| value.unwrap_or(default))?)
}

async fn checkpoint_schema_version(
    tx: &slatedb::DbTransaction,
    checkpoint_id: u64,
    snapshot_id: u64,
) -> CatalogResult<u64> {
    let key = checkpoint_state_key(checkpoint_id, &keys::key_snapshot(snapshot_id));
    Ok(tx
        .get(key)
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        .map(|data| values::decode_value::<SnapshotRow>(&data).map(|row| row.schema_version))
        .transpose()
        .map(|value| value.unwrap_or(0))?)
}

fn checkpoint_pin_prefix() -> Vec<u8> {
    let mut buf = Vec::with_capacity(1 + 16);
    buf.push(TAG_SYSTEM);
    buf.extend_from_slice(b"checkpoint-pin:");
    buf
}

fn checkpoint_pin_key(name: &str) -> Vec<u8> {
    let mut buf = checkpoint_pin_prefix();
    buf.extend_from_slice(name.as_bytes());
    buf
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use object_store::local::LocalFileSystem;
    use object_store::path::Path as ObjectPath;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn open_test_db(dir: &std::path::Path) -> slatedb::Db {
        let fs: Arc<dyn object_store::ObjectStore> =
            Arc::new(LocalFileSystem::new_with_prefix(dir).unwrap());
        slatedb::Db::open(ObjectPath::from("catalog"), fs)
            .await
            .unwrap()
    }

    /// Two checkpoints created in rapid succession must have distinct,
    /// monotonically increasing IDs even if the wall clock has not advanced.
    #[tokio::test]
    async fn two_rapid_checkpoints_have_distinct_ids() {
        let dir = TempDir::new().unwrap();
        let db = open_test_db(dir.path()).await;

        let c1 = create_checkpoint(&db, None).await.unwrap();
        let c2 = create_checkpoint(&db, None).await.unwrap();

        assert_ne!(
            c1.id, c2.id,
            "consecutive checkpoints must have distinct IDs"
        );
        assert!(c2.id > c1.id, "checkpoint IDs must be strictly increasing");

        db.close().await.unwrap();
    }

    /// Pin and list checkpoint pins.
    #[tokio::test]
    async fn pin_and_list_checkpoint_pins() {
        let dir = TempDir::new().unwrap();
        let db = open_test_db(dir.path()).await;

        pin_checkpoint(&db, "alpha", 10).await.unwrap();
        pin_checkpoint(&db, "beta", 20).await.unwrap();

        let pins = list_checkpoint_pins(&db).await.unwrap();
        assert_eq!(pins.len(), 2);
        assert_eq!(pins[0].name, "alpha");
        assert_eq!(pins[0].snapshot_id, 10);
        assert_eq!(pins[1].name, "beta");
        assert_eq!(pins[1].snapshot_id, 20);

        db.close().await.unwrap();
    }

    /// Unpin removes the named pin and returns NotFound on re-unpin.
    #[tokio::test]
    async fn unpin_removes_pin() {
        let dir = TempDir::new().unwrap();
        let db = open_test_db(dir.path()).await;

        pin_checkpoint(&db, "gamma", 5).await.unwrap();
        unpin_checkpoint(&db, "gamma").await.unwrap();

        let pins = list_checkpoint_pins(&db).await.unwrap();
        assert!(pins.is_empty(), "no pins should remain after unpin");

        // Second unpin should return NotFound.
        let err = unpin_checkpoint(&db, "gamma").await.unwrap_err();
        assert!(
            matches!(err, CatalogError::NotFound(_)),
            "expected NotFound, got {err:?}"
        );

        db.close().await.unwrap();
    }
}
