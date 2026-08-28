//! Parquet data-file cleanup operations.
//!
//! - Orphaned-file sweep: scan object-store paths not referenced by any catalog row.
//! - Scheduled deletion: files marked for cleanup after no retained snapshot references them.
//! - verify_data_files: HEAD every referenced file and flag missing ones.

use object_store::path::Path as ObjectPath;
use object_store::ObjectStore;
use rocklake_core::keys;
use rocklake_core::rows::*;
use rocklake_core::tags::*;
use rocklake_core::values;
use slatedb::Db;
use std::collections::HashSet;
use std::sync::Arc;

use crate::error::{CatalogError, CatalogResult};

/// Result of orphaned-file sweep.
#[derive(Debug, Clone)]
pub struct OrphanedFileSweepResult {
    /// Files found in object store that are not referenced by any catalog row.
    pub orphaned_files: Vec<String>,
    /// Files that were deleted (only if apply=true).
    pub deleted_files: Vec<String>,
    /// Total files scanned.
    pub total_files_scanned: u64,
    /// Files whose deletion failed in apply mode.
    pub deletion_failures: Vec<(String, String)>,
}

/// Result of data-file verification.
#[derive(Debug, Clone)]
pub struct VerifyDataFilesResult {
    /// Files that exist and are accessible.
    pub files_ok: u64,
    /// Files that are missing from object store.
    pub files_missing: Vec<String>,
    /// Files that returned errors (permissions, etc.).
    pub files_error: Vec<(String, String)>,
    /// Total files checked.
    pub total_checked: u64,
}

/// Collect all data file paths referenced in the catalog.
pub async fn collect_referenced_paths(db: &Db) -> CatalogResult<HashSet<String>> {
    let mut paths = HashSet::new();

    // Scan data files
    let prefix = keys::prefix_for_tag(TAG_DATA_FILE);
    let mut iter = db.scan_prefix(&prefix).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let row: DataFileRow = values::decode_value(&kv.value)?;
        paths.insert(row.path);
    }

    // Scan delete files
    let prefix = keys::prefix_for_tag(TAG_DELETE_FILE);
    let mut iter = db.scan_prefix(&prefix).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let row: DeleteFileRow = values::decode_value(&kv.value)?;
        paths.insert(row.path);
    }

    Ok(paths)
}

/// Collect referenced paths in the same canonical namespace used by an object-store scan.
pub async fn collect_referenced_paths_at(
    db: &Db,
    data_prefix: &ObjectPath,
) -> CatalogResult<HashSet<String>> {
    let mut paths = HashSet::new();
    for tag in [TAG_DATA_FILE, TAG_DELETE_FILE] {
        let mut iter = db.scan_prefix(&keys::prefix_for_tag(tag)).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let path = if tag == TAG_DATA_FILE {
                let row: DataFileRow = values::decode_value(&kv.value)?;
                canonical_object_path(data_prefix, &row.path, row.path_is_relative)?
            } else {
                let row: DeleteFileRow = values::decode_value(&kv.value)?;
                canonical_object_path(data_prefix, &row.path, row.path_is_relative)?
            };
            paths.insert(path.to_string());
        }
    }
    Ok(paths)
}

/// Scan object store for orphaned files not referenced in the catalog.
pub async fn orphaned_file_sweep(
    db: &Db,
    object_store: &Arc<dyn ObjectStore>,
    data_prefix: &ObjectPath,
    grace_period_secs: u64,
    apply: bool,
) -> CatalogResult<OrphanedFileSweepResult> {
    let referenced = collect_referenced_paths_at(db, data_prefix).await?;

    let mut orphaned_files = Vec::new();
    let mut deleted_files = Vec::new();
    let mut total_files_scanned = 0u64;
    let mut deletion_failures = Vec::new();

    // List all objects under the data prefix
    let mut objects = object_store.list(Some(data_prefix));
    while let Some(obj) = objects
        .try_next()
        .await
        .map_err(|e| CatalogError::ObjectStorePermanent(format!("failed to list objects: {e}")))?
    {
        let path_str = obj.location.to_string();

        // Only consider Parquet and Arrow IPC data files as potential
        // orphans. SlateDB infrastructure files (.sst, .manifest, .wal,
        // .db, etc.) are never orphans and must not be deleted.
        let is_data_file = path_str.ends_with(".parquet")
            || path_str.ends_with(".arrow")
            || path_str.ends_with(".avro");
        if !is_data_file {
            continue;
        }

        total_files_scanned += 1;

        if !referenced.contains(&path_str) {
            // Check grace period
            let file_age_secs = chrono::Utc::now()
                .signed_duration_since(obj.last_modified)
                .num_seconds()
                .max(0) as u64;

            if file_age_secs >= grace_period_secs {
                orphaned_files.push(path_str.clone());
                if apply {
                    crate::fault_injection::trigger(
                        crate::fault_injection::WriteFaultPoint::BeforeCleanupObjectDelete,
                    )
                    .await?;
                    match object_store.delete(&obj.location).await {
                        Ok(_) => deleted_files.push(path_str),
                        Err(e) => deletion_failures.push((path_str, e.to_string())),
                    }
                }
            }
        }
    }

    Ok(OrphanedFileSweepResult {
        orphaned_files,
        deleted_files,
        total_files_scanned,
        deletion_failures,
    })
}

/// Verify that all referenced data files exist in the object store.
pub async fn verify_data_files(
    db: &Db,
    object_store: &Arc<dyn ObjectStore>,
) -> CatalogResult<VerifyDataFilesResult> {
    verify_data_files_at(db, object_store, &ObjectPath::from("")).await
}

/// Verify referenced data and delete files under a canonical object-store prefix.
pub async fn verify_data_files_at(
    db: &Db,
    object_store: &Arc<dyn ObjectStore>,
    data_prefix: &ObjectPath,
) -> CatalogResult<VerifyDataFilesResult> {
    let referenced = collect_referenced_paths_at(db, data_prefix).await?;

    let mut files_ok = 0u64;
    let mut files_missing = Vec::new();
    let mut files_error = Vec::new();
    let total_checked = referenced.len() as u64;

    for path_str in &referenced {
        let path = ObjectPath::from(path_str.as_str());
        match object_store.head(&path).await {
            Ok(_) => files_ok += 1,
            Err(object_store::Error::NotFound { .. }) => {
                files_missing.push(path_str.clone());
            }
            Err(e) => {
                files_error.push((path_str.clone(), e.to_string()));
            }
        }
    }

    Ok(VerifyDataFilesResult {
        files_ok,
        files_missing,
        files_error,
        total_checked,
    })
}

/// Result of processing scheduled object deletions.
#[derive(Debug, Clone, Default)]
pub struct ScheduledDeletionResult {
    /// Number of object files deleted or already absent.
    pub deleted: u64,
    /// Object-store deletion failures.
    pub deletion_failures: Vec<(String, String)>,
    /// Catalog schedule rows that could not be removed.
    pub catalog_failures: Vec<(String, String)>,
    /// Rows retained because their file retirement could not be proven.
    pub skipped: u64,
}

/// Process scheduled file deletions.
pub async fn process_scheduled_deletions(
    db: &Db,
    object_store: &Arc<dyn ObjectStore>,
    retain_from: u64,
) -> CatalogResult<u64> {
    Ok(
        process_scheduled_deletions_report(db, object_store, retain_from)
            .await?
            .deleted,
    )
}

/// Process scheduled deletions using file MVCC, not the schedule timestamp, as
/// the safety decision. `schedule_start` only prevents future-dated rows from
/// being processed.
pub async fn process_scheduled_deletions_report(
    db: &Db,
    object_store: &Arc<dyn ObjectStore>,
    retain_from: u64,
) -> CatalogResult<ScheduledDeletionResult> {
    process_scheduled_deletions_report_at(db, object_store, &ObjectPath::from(""), retain_from)
        .await
}

/// Process scheduled deletions under a canonical data prefix.
pub async fn process_scheduled_deletions_report_at(
    db: &Db,
    object_store: &Arc<dyn ObjectStore>,
    data_prefix: &ObjectPath,
    retain_from: u64,
) -> CatalogResult<ScheduledDeletionResult> {
    let prefix = keys::prefix_for_tag(TAG_FILES_SCHEDULED_FOR_DELETION);
    let mut result = ScheduledDeletionResult::default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut iter = db.scan_prefix(&prefix).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let row: FilesScheduledForDeletionRow = values::decode_value(&kv.value)?;

        if retain_from == 0
            || row.schedule_start > now
            || !file_retired_at(db, &row, retain_from).await?
        {
            result.skipped += 1;
            continue;
        }
        let path = canonical_object_path(data_prefix, &row.path, row.path_is_relative)?;
        crate::fault_injection::trigger(
            crate::fault_injection::WriteFaultPoint::BeforeCleanupObjectDelete,
        )
        .await?;
        let object_deleted = match object_store.delete(&path).await {
            Ok(_) => {
                result.deleted += 1;
                true
            }
            Err(object_store::Error::NotFound { .. }) => {
                result.deleted += 1;
                true
            }
            Err(e) => {
                result
                    .deletion_failures
                    .push((row.path.clone(), e.to_string()));
                false
            }
        };
        if object_deleted {
            crate::fault_injection::trigger(
                crate::fault_injection::WriteFaultPoint::BeforeCleanupCatalogDelete,
            )
            .await?;
            if let Err(e) = db.delete(&kv.key).await {
                result
                    .catalog_failures
                    .push((row.path.clone(), e.to_string()));
            }
        }
    }

    Ok(result)
}

async fn file_retired_at(
    db: &Db,
    scheduled: &FilesScheduledForDeletionRow,
    retain_from: u64,
) -> CatalogResult<bool> {
    let mut iter = db.scan_prefix(&keys::prefix_for_tag(TAG_DATA_FILE)).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let row: DataFileRow = values::decode_value(&kv.value)?;
        if row.data_file_id == scheduled.data_file_id
            && row.path == scheduled.path
            && row.end_snapshot.is_some_and(|end| end <= retain_from)
        {
            return Ok(true);
        }
    }

    let mut iter = db
        .scan_prefix(&keys::prefix_for_tag(TAG_DELETE_FILE))
        .await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let row: DeleteFileRow = values::decode_value(&kv.value)?;
        if row.data_file_id == scheduled.data_file_id
            && row.path == scheduled.path
            && row.end_snapshot.is_some_and(|end| end <= retain_from)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn canonical_object_path(
    data_prefix: &ObjectPath,
    stored_path: &str,
    path_is_relative: Option<bool>,
) -> CatalogResult<ObjectPath> {
    rocklake_core::path::resolve_object_path(data_prefix.as_ref(), stored_path, path_is_relative)
        .map(ObjectPath::from)
        .map_err(|e| CatalogError::InvalidInput(e.to_string()))
}

use futures::TryStreamExt;

#[cfg(test)]
mod tests {
    use super::canonical_object_path;
    use object_store::path::Path;

    #[test]
    fn canonical_paths_keep_nested_prefixes_and_reject_unsafe_rows() {
        assert_eq!(
            canonical_object_path(
                &Path::from("data/warehouse/nested"),
                "table/file.parquet",
                Some(true)
            )
            .unwrap(),
            Path::from("data/warehouse/nested/table/file.parquet")
        );
        assert!(
            canonical_object_path(&Path::from("data/warehouse"), "../outside", Some(true)).is_err()
        );
        assert!(canonical_object_path(
            &Path::from("data/warehouse"),
            "data/other/file",
            Some(false)
        )
        .is_err());
    }
}
