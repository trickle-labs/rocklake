//! Versioned, self-contained catalog backup artifacts.

use crate::error::{CatalogError, CatalogResult};
use crate::export::{export_catalog, ExportManifest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use slatedb::Db;
use std::path::{Path, PathBuf};

const MANIFEST_FILE: &str = "manifest.json";
const DATA_FILE: &str = "catalog.ndjson";
/// Current backup artifact format.
pub const BACKUP_FORMAT_VERSION: u32 = 1;

/// Metadata stored beside a catalog backup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupManifest {
    /// Backup artifact format version.
    #[serde(rename = "version")]
    pub version: u32,
    /// Stable identity of the source catalog.
    pub source_identity: String,
    /// RFC 3339 creation timestamp.
    pub created_at: String,
    /// Snapshot represented by the export.
    pub snapshot_id: u64,
    /// Number of exported catalog rows.
    pub row_count: u64,
    /// Size of the NDJSON payload in bytes.
    pub byte_count: u64,
    /// Lowercase SHA-256 digest of the NDJSON payload.
    pub sha256: String,
}

/// Validated backup metadata returned by create and inspect operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupInfo {
    /// Backup directory.
    pub path: PathBuf,
    /// Validated backup manifest.
    pub manifest: BackupManifest,
}

/// Create a backup directory containing a manifest and snapshot-consistent NDJSON.
pub async fn create_backup(
    db: &Db,
    directory: impl AsRef<Path>,
    source_identity: impl Into<String>,
    snapshot_id: Option<u64>,
) -> CatalogResult<BackupInfo> {
    let directory = directory.as_ref();
    tokio::fs::create_dir_all(directory)
        .await
        .map_err(|e| CatalogError::InvalidInput(format!("create backup directory: {e}")))?;
    let mut data = Vec::new();
    let export = export_catalog(db, snapshot_id, &mut data).await?;
    let export_manifest: ExportManifest = data
        .split(|byte| *byte == b'\n')
        .find(|line| !line.is_empty())
        .and_then(|line| serde_json::from_slice(line).ok())
        .ok_or_else(|| CatalogError::Corruption("export did not contain a manifest".into()))?;
    tokio::fs::write(directory.join(DATA_FILE), &data)
        .await
        .map_err(|e| CatalogError::InvalidInput(format!("write backup data: {e}")))?;
    let manifest = BackupManifest {
        version: BACKUP_FORMAT_VERSION,
        source_identity: source_identity.into(),
        created_at: chrono::Utc::now().to_rfc3339(),
        snapshot_id: export_manifest.snapshot_id,
        row_count: export.rows_exported,
        byte_count: data.len() as u64,
        sha256: sha256_hex(&data),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| CatalogError::Internal(format!("serialize backup manifest: {e}")))?;
    tokio::fs::write(directory.join(MANIFEST_FILE), manifest_bytes)
        .await
        .map_err(|e| CatalogError::InvalidInput(format!("write backup manifest: {e}")))?;
    Ok(BackupInfo {
        path: directory.to_path_buf(),
        manifest,
    })
}

/// Inspect and validate a backup directory without opening or mutating a catalog.
pub async fn inspect_backup(directory: impl AsRef<Path>) -> CatalogResult<BackupInfo> {
    let directory = directory.as_ref();
    let manifest: BackupManifest = serde_json::from_slice(
        &tokio::fs::read(directory.join(MANIFEST_FILE))
            .await
            .map_err(|e| CatalogError::InvalidInput(format!("read backup manifest: {e}")))?,
    )
    .map_err(|e| CatalogError::Corruption(format!("invalid backup manifest: {e}")))?;
    if manifest.version != BACKUP_FORMAT_VERSION {
        return Err(CatalogError::Corruption(format!(
            "unsupported backup version {} (expected {})",
            manifest.version, BACKUP_FORMAT_VERSION
        )));
    }
    let data = tokio::fs::read(directory.join(DATA_FILE))
        .await
        .map_err(|e| CatalogError::InvalidInput(format!("read backup data: {e}")))?;
    if manifest.byte_count != data.len() as u64 || manifest.sha256 != sha256_hex(&data) {
        return Err(CatalogError::Corruption(
            "backup checksum or byte count mismatch".into(),
        ));
    }
    let row_count = data
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .count()
        .saturating_sub(1) as u64;
    if manifest.row_count != row_count {
        return Err(CatalogError::Corruption("backup row count mismatch".into()));
    }
    Ok(BackupInfo {
        path: directory.to_path_buf(),
        manifest,
    })
}

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::sha256_hex;
    use object_store::path::Path as ObjectPath;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[test]
    fn sha256_matches_standard_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[tokio::test]
    async fn create_and_inspect_backup_validates_the_artifact() {
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
        let db = slatedb::Db::open(ObjectPath::from("catalog"), store)
            .await
            .unwrap();
        let backup_path = dir.path().join("backup");
        let created = super::create_backup(&db, &backup_path, "local", None)
            .await
            .unwrap();
        let inspected = super::inspect_backup(&backup_path).await.unwrap();
        assert_eq!(created.manifest, inspected.manifest);
        db.close().await.unwrap();
    }
}
