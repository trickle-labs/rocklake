//! Versioned, self-contained catalog backup artifacts.

use crate::error::{CatalogError, CatalogResult};
use crate::export::{export_catalog, ExportManifest};
use crate::gc::read_pinned_snapshots;
use crate::inspect::inspect_snapshot;
use object_store::path::Path as ObjectPath;
use object_store::ObjectStore;
use rocklake_core::mvcc;
use rocklake_core::rows::DataFileRow;
use rocklake_core::tags::TAG_DATA_FILE;
use rocklake_core::{keys, values};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use slatedb::Db;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;

const MANIFEST_FILE: &str = "manifest.json";
const DATA_FILE: &str = "catalog.ndjson";
const MAX_OBJECT_REFERENCE_INVENTORY: usize = 100_000;
/// Current backup artifact format.
pub const BACKUP_FORMAT_VERSION: u32 = 2;

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
    /// RockLake catalog storage format.
    #[serde(default)]
    pub catalog_format: u32,
    /// Alias for the exported snapshot used by backup-set tooling.
    #[serde(default)]
    pub latest_snapshot: u64,
    /// Visibility retention floor at backup time.
    #[serde(default)]
    pub retention_floor: u64,
    /// Snapshot pins that must survive recovery planning.
    #[serde(default)]
    pub checkpoint_pins: Vec<u64>,
    /// Job records are intentionally excluded from catalog metadata exports.
    #[serde(default)]
    pub job_state_policy: JobStatePolicy,
    /// Referenced data-file inventory, when requested.
    #[serde(default)]
    pub object_references: Option<ObjectReferenceInventory>,
    /// Whether referenced data bytes are part of this artifact.
    #[serde(default)]
    pub data_files_copied: bool,
    /// Non-secret encryption key IDs required by this catalog.
    #[serde(default)]
    pub encryption_key_ids: Vec<String>,
    /// Managed registry generation, when this artifact belongs to a set.
    #[serde(default)]
    pub registry_generation: Option<u64>,
    /// Stable catalog IDs, when this artifact belongs to a set.
    #[serde(default)]
    pub catalog_ids: Vec<String>,
}

/// Policy describing how durable job records are handled by a metadata backup.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatePolicy {
    /// Active jobs are not copied; operators resume them from their source.
    #[default]
    Exclude,
}

/// One referenced data object in a backup inventory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectReference {
    /// Catalog data-file ID.
    pub data_file_id: u64,
    /// Object path relative to the configured data root.
    pub path: String,
    /// Size recorded in the catalog.
    pub expected_size_bytes: u64,
    /// Result of an optional HEAD check.
    #[serde(default)]
    pub head: Option<ObjectHeadResult>,
    /// Non-secret encryption key ID recorded by the catalog.
    #[serde(default)]
    pub encryption_key_id: Option<String>,
}

/// Bounded result of checking one referenced object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectHeadResult {
    /// Whether the object exists.
    pub present: bool,
    /// Object-store size, when present.
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

/// Referenced-data inventory included in a backup manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectReferenceInventory {
    /// Number of references recorded.
    pub count: u64,
    /// Individual references at the selected snapshot.
    pub files: Vec<ObjectReference>,
    /// Whether object HEAD checks were requested.
    pub head_verified: bool,
}

/// Options for creating a versioned catalog backup.
#[derive(Clone, Default)]
pub struct BackupOptions {
    /// Stable source catalog identity.
    pub source_identity: String,
    /// Snapshot to export; latest when omitted.
    pub snapshot_id: Option<u64>,
    /// Object store containing referenced data files.
    pub data_store: Option<Arc<dyn ObjectStore>>,
    /// Prefix containing referenced data files.
    pub data_root: Option<ObjectPath>,
    /// Include a bounded reference inventory in the manifest.
    pub include_data_inventory: bool,
    /// HEAD every referenced object and fail if any is missing.
    pub verify_data: bool,
    /// Managed registry generation, when creating a backup set.
    pub registry_generation: Option<u64>,
    /// Stable catalog IDs, when creating a backup set.
    pub catalog_ids: Vec<String>,
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
    create_backup_with_options(
        db,
        directory,
        BackupOptions {
            source_identity: source_identity.into(),
            snapshot_id,
            ..BackupOptions::default()
        },
    )
    .await
}

/// Create a versioned catalog backup with optional referenced-data evidence.
pub async fn create_backup_with_options(
    db: &Db,
    directory: impl AsRef<Path>,
    options: BackupOptions,
) -> CatalogResult<BackupInfo> {
    let directory = directory.as_ref();
    tokio::fs::create_dir_all(directory)
        .await
        .map_err(|e| CatalogError::InvalidInput(format!("create backup directory: {e}")))?;
    let file = std::fs::File::create(directory.join(DATA_FILE))
        .map_err(|e| CatalogError::InvalidInput(format!("create backup data: {e}")))?;
    let mut writer = DigestWriter::new(file);
    let export = export_catalog(db, options.snapshot_id, &mut writer).await?;
    writer
        .flush()
        .map_err(|e| CatalogError::InvalidInput(format!("flush backup data: {e}")))?;
    let selected_snapshot = writer
        .snapshot_id()
        .or(options.snapshot_id)
        .ok_or_else(|| CatalogError::Corruption("export did not contain a manifest".into()))?;
    let (byte_count, sha256) = writer.finish();
    let state = inspect_snapshot(db).await?;
    let checkpoint_pins = read_pinned_snapshots(db).await?;
    let object_references = if options.include_data_inventory || options.verify_data {
        Some(
            collect_object_references(
                db,
                selected_snapshot,
                options.data_store.as_ref(),
                options.data_root.as_ref(),
                options.verify_data,
            )
            .await?,
        )
    } else {
        None
    };
    let manifest = BackupManifest {
        version: BACKUP_FORMAT_VERSION,
        source_identity: options.source_identity,
        created_at: chrono::Utc::now().to_rfc3339(),
        snapshot_id: selected_snapshot,
        row_count: export.rows_exported,
        byte_count,
        sha256,
        catalog_format: state.format_version,
        latest_snapshot: selected_snapshot,
        retention_floor: state.retain_from,
        checkpoint_pins,
        job_state_policy: JobStatePolicy::Exclude,
        object_references,
        data_files_copied: false,
        encryption_key_ids: encryption_key_ids(db, selected_snapshot).await?,
        registry_generation: options.registry_generation,
        catalog_ids: options.catalog_ids,
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

async fn collect_object_references(
    db: &Db,
    snapshot_id: u64,
    data_store: Option<&Arc<dyn ObjectStore>>,
    data_root: Option<&ObjectPath>,
    verify_data: bool,
) -> CatalogResult<ObjectReferenceInventory> {
    let prefix = keys::prefix_for_tag(TAG_DATA_FILE);
    let mut files = Vec::new();
    let mut iter = db.scan_prefix(&prefix).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let row: DataFileRow = values::decode_value(&kv.value)?;
        if !mvcc::is_visible(
            row.begin_snapshot.unwrap_or(0),
            row.end_snapshot,
            rocklake_core::mvcc::SnapshotId::new(snapshot_id),
        ) {
            continue;
        }
        let head = if verify_data {
            let store = data_store.ok_or_else(|| {
                CatalogError::InvalidInput(
                    "--verify-data requires a referenced data object store".into(),
                )
            })?;
            let path = data_root.map_or_else(
                || ObjectPath::from(row.path.as_str()),
                |root| root.child(row.path.as_str()),
            );
            match store.head(&path).await {
                Ok(meta) => Some(ObjectHeadResult {
                    present: true,
                    size_bytes: Some(meta.size),
                }),
                Err(_) => {
                    return Err(CatalogError::InvalidInput(format!(
                        "referenced data file is missing: {}",
                        row.path
                    )))
                }
            }
        } else {
            None
        };
        // ponytail: cap inventory memory at 100k references; raise the cap or
        // stream inventory to a sidecar file when larger catalogs need it.
        if files.len() >= MAX_OBJECT_REFERENCE_INVENTORY {
            return Err(CatalogError::InvalidInput(format!(
                "referenced data inventory exceeds {MAX_OBJECT_REFERENCE_INVENTORY} files"
            )));
        }
        files.push(ObjectReference {
            data_file_id: row.data_file_id,
            path: row.path,
            expected_size_bytes: row.file_size_bytes,
            head,
            encryption_key_id: row.encryption_key,
        });
    }
    Ok(ObjectReferenceInventory {
        count: files.len() as u64,
        files,
        head_verified: verify_data,
    })
}

async fn encryption_key_ids(db: &Db, snapshot_id: u64) -> CatalogResult<Vec<String>> {
    let inventory = collect_object_references(db, snapshot_id, None, None, false).await?;
    let mut ids: Vec<_> = inventory
        .files
        .into_iter()
        .filter_map(|file| file.encryption_key_id)
        .collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
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
    if manifest.version == 0 || manifest.version > BACKUP_FORMAT_VERSION {
        return Err(CatalogError::Corruption(format!(
            "unsupported backup version {} (supported 1..={})",
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
