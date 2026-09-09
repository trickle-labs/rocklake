//! Versioned, self-contained catalog backup artifacts.

use crate::error::{CatalogError, CatalogResult};
use crate::export::{export_catalog, ExportManifest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use slatedb::Db;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tokio::io::AsyncBufReadExt;

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
    let file = std::fs::File::create(directory.join(DATA_FILE))
        .map_err(|e| CatalogError::InvalidInput(format!("create backup data: {e}")))?;
    let mut writer = DigestWriter::new(file);
    let export = export_catalog(db, snapshot_id, &mut writer).await?;
    writer
        .flush()
        .map_err(|e| CatalogError::InvalidInput(format!("flush backup data: {e}")))?;
    let selected_snapshot = writer
        .snapshot_id()
        .or(snapshot_id)
        .ok_or_else(|| CatalogError::Corruption("export did not contain a manifest".into()))?;
    let (byte_count, sha256) = writer.finish();
    let manifest = BackupManifest {
        version: BACKUP_FORMAT_VERSION,
        source_identity: source_identity.into(),
        created_at: chrono::Utc::now().to_rfc3339(),
        snapshot_id: selected_snapshot,
        row_count: export.rows_exported,
        byte_count,
        sha256,
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
    let file = tokio::fs::File::open(directory.join(DATA_FILE))
        .await
        .map_err(|e| CatalogError::InvalidInput(format!("read backup data: {e}")))?;
    let mut reader = tokio::io::BufReader::new(file);
    let mut line = String::new();
    let mut byte_count = 0u64;
    let mut row_count = 0u64;
    let mut digest = Sha256::new();
    while reader
        .read_line(&mut line)
        .await
        .map_err(|e| CatalogError::InvalidInput(format!("read backup data: {e}")))?
        > 0
    {
        digest.update(line.as_bytes());
        byte_count += line.len() as u64;
        if !line.trim().is_empty() {
            row_count += 1;
        }
        line.clear();
    }
    if manifest.byte_count != byte_count || manifest.sha256 != digest_hex(digest.finalize()) {
        return Err(CatalogError::Corruption(
            "backup checksum or byte count mismatch".into(),
        ));
    }
    if row_count == 0 || manifest.row_count != row_count - 1 {
        return Err(CatalogError::Corruption("backup row count mismatch".into()));
    }
    Ok(BackupInfo {
        path: directory.to_path_buf(),
        manifest,
    })
}

#[cfg(test)]
fn sha256_hex(data: &[u8]) -> String {
    digest_hex(Sha256::digest(data))
}

fn digest_hex(digest: impl AsRef<[u8]>) -> String {
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

struct DigestWriter {
    file: std::fs::File,
    digest: Sha256,
    bytes_written: u64,
    header: Vec<u8>,
}

impl DigestWriter {
    fn new(file: std::fs::File) -> Self {
        Self {
            file,
            digest: Sha256::new(),
            bytes_written: 0,
            header: Vec::new(),
        }
    }

    fn snapshot_id(&self) -> Option<u64> {
        let line = self.header.split(|byte| *byte == b'\n').next()?;
        serde_json::from_slice::<ExportManifest>(line)
            .ok()
            .map(|manifest| manifest.snapshot_id)
    }

    fn finish(self) -> (u64, String) {
        (self.bytes_written, digest_hex(self.digest.finalize()))
    }
}

impl Write for DigestWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.file.write(bytes)?;
        self.digest.update(&bytes[..written]);
        self.bytes_written += written as u64;
        if !self.header.contains(&b'\n') {
            self.header.extend_from_slice(&bytes[..written]);
            if self.header.len() > 1024 * 1024 {
                self.header.clear();
            }
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
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
