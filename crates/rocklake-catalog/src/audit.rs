//! Audit log: structured entries for every snapshot commit.
//!
//! Records who committed, when, and what changed in each snapshot.
//! Stored under `0xFF | "audit"` prefix for accumulation without overwriting.

use rocklake_core::keys;
use rocklake_core::values;
use sha2::{Digest, Sha256};
use slatedb::Db;

use crate::error::{CatalogError, CatalogResult};

/// An audit log entry for a snapshot commit.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditEntry {
    /// The snapshot ID committed.
    pub snapshot_id: u64,
    /// Timestamp of the commit (RFC 3339).
    pub committed_at: String,
    /// Who performed the commit (author or system).
    pub committed_by: String,
    /// Summary of what changed.
    pub changes: Vec<AuditChange>,
}

/// A single change recorded in the audit log.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditChange {
    /// Type of change: "create_schema", "create_table", "register_data_file", etc.
    pub change_type: String,
    /// Optional detail (e.g., table name, file path).
    pub detail: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AuditChainLink {
    schema_version: u32,
    snapshot_id: u64,
    previous_hash: String,
    hash: String,
}

/// Write an audit log entry for a snapshot commit.
pub async fn write_audit_entry(db: &Db, entry: &AuditEntry) -> CatalogResult<()> {
    let key = keys::key_audit(entry.snapshot_id);
    if db.get(&key).await?.is_some() {
        return Err(CatalogError::Duplicate(format!(
            "audit entry for snapshot {}",
            entry.snapshot_id
        )));
    }
    let (previous_hash, last_snapshot) = last_chain_link(db).await?;
    if last_snapshot != 0 && entry.snapshot_id <= last_snapshot {
        return Err(CatalogError::InvalidInput(
            "audit snapshots must be appended in order".to_string(),
        ));
    }
    let value = serde_json::to_vec(entry)
        .map_err(|e| CatalogError::Internal(format!("audit entry serialize: {e}")))?;
    let encoded = values::encode_raw_value(&value);
    db.put(&key, &encoded).await?;

    let link = AuditChainLink {
        schema_version: rocklake_core::version::AUDIT_SCHEMA_VERSION,
        snapshot_id: entry.snapshot_id,
        previous_hash: previous_hash.clone(),
        hash: audit_hash(&previous_hash, &value),
    };
    db.put(
        &chain_key(entry.snapshot_id),
        &serde_json::to_vec(&link)
            .map_err(|e| CatalogError::Internal(format!("audit chain serialize: {e}")))?,
    )
    .await?;
    Ok(())
}

/// Verify the hash chain for v0.60.0 audit entries.
pub async fn verify_audit_chain(db: &Db) -> CatalogResult<()> {
    let prefix = chain_prefix();
    let mut links = Vec::new();
    let mut iter = db.scan_prefix(&prefix).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        links.push(
            serde_json::from_slice::<AuditChainLink>(&kv.value)
                .map_err(|e| CatalogError::Corruption(format!("invalid audit chain link: {e}")))?,
        );
    }
    links.sort_by_key(|link| link.snapshot_id);
    let mut previous_hash = String::new();
    for link in links {
        if link.schema_version != rocklake_core::version::AUDIT_SCHEMA_VERSION
            || link.previous_hash != previous_hash
        {
            return Err(CatalogError::Corruption(
                "audit hash chain is broken".to_string(),
            ));
        }
        let value = db
            .get(&keys::key_audit(link.snapshot_id))
            .await?
            .ok_or_else(|| CatalogError::Corruption("audit entry is missing".to_string()))?;
        let raw = values::decode_raw_value(&value)?;
        let expected = audit_hash(&previous_hash, &raw);
        if link.hash != expected {
            return Err(CatalogError::Corruption(format!(
                "audit entry {} failed integrity verification",
                link.snapshot_id
            )));
        }
        previous_hash = link.hash;
    }
    Ok(())
}

async fn last_chain_link(db: &Db) -> CatalogResult<(String, u64)> {
    let mut iter = db.scan_prefix(&chain_prefix()).await?;
    let mut last = None;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let link: AuditChainLink = serde_json::from_slice(&kv.value)
            .map_err(|e| CatalogError::Corruption(format!("invalid audit chain link: {e}")))?;
        if last
            .as_ref()
            .is_none_or(|(_, snapshot_id)| link.snapshot_id > *snapshot_id)
        {
            last = Some((link.hash, link.snapshot_id));
        }
    }
    Ok(last.unwrap_or_default())
}

fn chain_prefix() -> Vec<u8> {
    keys::key_system(b"security-audit/")
}

fn chain_key(snapshot_id: u64) -> Vec<u8> {
    let mut key = chain_prefix();
    key.extend_from_slice(&keys::encode_u64(snapshot_id));
    key
}

fn audit_hash(previous_hash: &str, value: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(previous_hash.as_bytes());
    digest.update(value);
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Read all audit log entries.
pub async fn list_audit_entries(db: &Db) -> CatalogResult<Vec<AuditEntry>> {
    let prefix = keys::audit_prefix();
    let mut entries = Vec::new();

    let mut iter = db.scan_prefix(&prefix).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let raw = values::decode_raw_value(&kv.value)?;
        if let Ok(entry) = serde_json::from_slice::<AuditEntry>(&raw) {
            entries.push(entry);
        }
    }

    Ok(entries)
}

/// Read an audit log entry for a specific snapshot.
pub async fn get_audit_entry(db: &Db, snapshot_id: u64) -> CatalogResult<Option<AuditEntry>> {
    let key = keys::key_audit(snapshot_id);
    match db.get(&key).await? {
        Some(value) => {
            let raw = values::decode_raw_value(&value)?;
            let entry = serde_json::from_slice::<AuditEntry>(&raw)
                .map_err(|e| CatalogError::Internal(format!("audit entry deserialize: {e}")))?;
            Ok(Some(entry))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::local::LocalFileSystem;
    use object_store::path::Path as ObjectPath;
    use std::sync::Arc;

    #[tokio::test]
    async fn audit_chain_detects_payload_tampering() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(LocalFileSystem::new_with_prefix(dir.path()).unwrap());
        let db = Db::open(ObjectPath::from(""), store).await.unwrap();
        let entry = AuditEntry {
            snapshot_id: 1,
            committed_at: "2026-09-10T00:00:00Z".into(),
            committed_by: "test".into(),
            changes: Vec::new(),
        };
        write_audit_entry(&db, &entry).await.unwrap();
        verify_audit_chain(&db).await.unwrap();

        let tampered = serde_json::to_vec(&AuditEntry {
            committed_by: "attacker".into(),
            ..entry
        })
        .unwrap();
        db.put(&keys::key_audit(1), &values::encode_raw_value(&tampered))
            .await
            .unwrap();
        assert!(verify_audit_chain(&db).await.is_err());
        db.close().await.unwrap();
    }
}
