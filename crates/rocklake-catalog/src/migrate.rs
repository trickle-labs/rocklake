//! Catalog migration subcommand.
//!
//! Automates the `export → reinitialize-at-new-format-version → import`
//! sequence for forward-incompatible `catalog-format-version` bumps.
//! Supports `--dry-run` mode that reports the number of rows to migrate
//! and estimated duration without making changes.

use serde::{Deserialize, Serialize};
use slatedb::Db;

use crate::error::{CatalogError, CatalogResult};
use crate::export;
use crate::inspect;
use crate::verify;

/// How a registered migration may run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MigrationMode {
    /// The source and target use the same persisted format.
    NoOp,
    /// The migration requires exclusive writer ownership.
    Offline,
}

/// Downgrade behavior for a migration target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DowngradePolicy {
    /// The target cannot be safely downgraded after activation.
    Reject,
}

/// Typed metadata for one supported catalog-format migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MigrationDefinition {
    /// Source RockLake storage format.
    pub source_version: u32,
    /// Target RockLake storage format.
    pub target_version: u32,
    /// Execution class.
    pub mode: MigrationMode,
    /// Whether an external backup is required before mutation.
    pub backup_required: bool,
    /// Migration prerequisites.
    pub prerequisites: &'static [&'static str],
    /// Boundary after which rollback is restore-only.
    pub rollback_boundary: &'static str,
    /// Downgrade behavior after activation.
    pub downgrade_policy: DowngradePolicy,
}

/// The v0.60 migration registry. No format-changing migration is shipped;
/// the registered no-op makes that compatibility promise explicit.
pub const MIGRATION_REGISTRY: &[MigrationDefinition] = &[MigrationDefinition {
    source_version: rocklake_core::version::CATALOG_STORAGE_VERSION,
    target_version: rocklake_core::version::CATALOG_STORAGE_VERSION,
    mode: MigrationMode::NoOp,
    backup_required: false,
    prerequisites: &["format version verified", "writer epoch not required"],
    rollback_boundary: "no-op",
    downgrade_policy: DowngradePolicy::Reject,
}];

/// Return the typed migration registry.
pub const fn migration_registry() -> &'static [MigrationDefinition] {
    MIGRATION_REGISTRY
}

/// A read-only migration plan.
#[derive(Debug, Clone, Serialize)]
pub struct MigrationPlan {
    /// Current format version.
    pub current_version: u32,
    /// Requested target format version.
    pub target_version: u32,
    /// Number of rows affected by the migration.
    pub rows_to_migrate: u64,
    /// Whether an external backup is required.
    pub backup_required: bool,
    /// Whether the migration needs exclusive writer ownership.
    pub mode: MigrationMode,
    /// Rollback boundary.
    pub rollback_boundary: String,
    /// Human-readable description.
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MigrationMarker {
    source_version: u32,
    target_version: u32,
    verified: bool,
    completed_at: String,
}

/// Result of a migration dry-run.
#[derive(Debug, Clone)]
pub struct MigrateDryRunResult {
    /// Current catalog format version.
    pub current_version: u32,
    /// Target catalog format version.
    pub target_version: u32,
    /// Number of rows that would be migrated.
    pub rows_to_migrate: u64,
    /// Estimated duration in seconds (rough estimate based on row count).
    pub estimated_seconds: u64,
    /// Human-readable description of what would happen.
    pub description: String,
    /// Whether a backup is required before mutation.
    pub backup_required: bool,
    /// Whether the migration needs exclusive writer ownership.
    pub mode: MigrationMode,
    /// Rollback boundary after activation.
    pub rollback_boundary: String,
}

/// Result of a completed migration.
#[derive(Debug, Clone)]
pub struct MigrateResult {
    /// Number of rows migrated.
    pub rows_migrated: u64,
    /// New format version.
    pub new_version: u32,
    /// Path to the backup export file created before migration.
    pub backup_path: String,
    /// Whether the target passed full verification.
    pub verification_passed: bool,
    /// Whether a target format marker was activated.
    pub activated: bool,
}

fn plan_for(
    current_version: u32,
    target_version: u32,
    rows_to_migrate: u64,
) -> CatalogResult<MigrationPlan> {
    if target_version < current_version {
        return Err(CatalogError::InvalidInput(format!(
            "downgrade from catalog format {current_version} to {target_version} is rejected before write"
        )));
    }
    let definition = MIGRATION_REGISTRY
        .iter()
        .find(|migration| {
            migration.source_version == current_version
                && migration.target_version == target_version
        })
        .ok_or_else(|| {
            CatalogError::InvalidInput(format!(
                "no registered migration from catalog format {current_version} to {target_version}"
            ))
        })?;
    let description = if current_version == target_version {
        format!("Catalog is already at version {current_version}. No migration needed.")
    } else {
        format!(
            "Migrate catalog format {current_version} to {target_version}; verify before activation."
        )
    };
    Ok(MigrationPlan {
        current_version,
        target_version,
        rows_to_migrate,
        backup_required: definition.backup_required,
        mode: definition.mode,
        rollback_boundary: definition.rollback_boundary.to_string(),
        description,
    })
}

/// Perform a dry-run of catalog migration.
///
/// Reports the number of rows that would be migrated and estimated duration
/// without making any changes to the catalog.
pub async fn migrate_dry_run(db: &Db, target_version: u32) -> CatalogResult<MigrateDryRunResult> {
    let state = inspect::inspect_snapshot(db).await?;
    let current_version = state.format_version;

    // Count rows to migrate (all live catalog rows)
    let rows_to_migrate = state.schema_count
        + state.table_count
        + state.column_count
        + state.data_file_count
        + state.delete_file_count;

    // Rough estimate: ~10ms per row for export + import cycle
    let estimated_seconds = if current_version == target_version {
        0
    } else {
        (rows_to_migrate / 100).max(1)
    };

    let plan = plan_for(current_version, target_version, rows_to_migrate)?;

    Ok(MigrateDryRunResult {
        current_version,
        target_version,
        rows_to_migrate,
        estimated_seconds,
        description: plan.description,
        backup_required: plan.backup_required,
        mode: plan.mode,
        rollback_boundary: plan.rollback_boundary,
    })
}

/// Apply catalog migration: export → reinitialize → import.
///
/// Always verifies the source before making changes and publishes a target
/// marker only after verification succeeds.
pub async fn migrate_apply(
    db: &Db,
    target_version: u32,
    backup_dir: &str,
) -> CatalogResult<MigrateResult> {
    let state = inspect::inspect_snapshot(db).await?;
    let current_version = state.format_version;

    let rows_to_migrate = state.schema_count
        + state.table_count
        + state.column_count
        + state.data_file_count
        + state.delete_file_count;
    let plan = plan_for(current_version, target_version, rows_to_migrate)?;

    if plan.mode == MigrationMode::NoOp {
        let verification = verify::verify_catalog(db).await?;
        if !verification.is_ok() {
            return Err(CatalogError::Corruption(format!(
                "migration verification failed: {}",
                verification.errors.join("; ")
            )));
        }
        return Ok(MigrateResult {
            rows_migrated: 0,
            new_version: current_version,
            backup_path: String::new(),
            verification_passed: true,
            activated: false,
        });
    }

    // Step 1: Export to backup file
    let backup_path = format!("{backup_dir}/migrate-backup-v{current_version}.ndjson");
    let mut backup_file = std::fs::File::create(&backup_path)
        .map_err(|e| CatalogError::Internal(format!("Cannot create backup file: {e}")))?;
    let export_result = export::export_catalog(db, None, &mut backup_file).await?;

    tracing::info!(
        "Exported {} rows to backup {}",
        export_result.rows_exported,
        backup_path
    );

    // Step 2: A future registered migration transforms the catalog here. The
    // current release intentionally has no format-changing migration.
    use rocklake_core::keys;
    use rocklake_core::tags::SYSTEM_CATALOG_FORMAT_VERSION;
    use rocklake_core::values;
    let verification = verify::verify_catalog(db).await?;
    if !verification.is_ok() {
        return Err(CatalogError::Corruption(format!(
            "migration verification failed: {}",
            verification.errors.join("; ")
        )));
    }
    let fv_key = keys::key_system(SYSTEM_CATALOG_FORMAT_VERSION);
    let fv_value = values::encode_format_version(target_version);
    db.put(&fv_key, fv_value).await?;
    let marker = MigrationMarker {
        source_version: current_version,
        target_version,
        verified: true,
        completed_at: chrono::Utc::now().to_rfc3339(),
    };
    db.put(
        &keys::key_system(rocklake_core::tags::SYSTEM_MIGRATION_MARKER),
        values::encode_raw_value(
            &serde_json::to_vec(&marker)
                .map_err(|e| CatalogError::Internal(format!("migration marker: {e}")))?,
        ),
    )
    .await?;

    tracing::info!(
        "Migration complete: {} rows migrated, format version {} → {}",
        export_result.rows_exported,
        current_version,
        target_version
    );

    Ok(MigrateResult {
        rows_migrated: export_result.rows_exported,
        new_version: target_version,
        backup_path,
        verification_passed: true,
        activated: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn migrate_dry_run_same_version_no_op() {
        let dir = TempDir::new().unwrap();
        let path = object_store::path::Path::from("");
        let store = std::sync::Arc::new(
            object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap(),
        );
        let opts = crate::OpenOptions {
            object_store: store,
            path,
            encryption: None,
        };
        let _catalog = crate::CatalogStore::open(opts).await.unwrap();
        let store2 = std::sync::Arc::new(
            object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap(),
        );
        let db = Db::open(object_store::path::Path::from(""), store2)
            .await
            .unwrap();
        let result = migrate_dry_run(&db, 1).await.unwrap();
        assert_eq!(result.rows_to_migrate, 0);
        let applied = migrate_apply(&db, 1, dir.path().to_str().unwrap())
            .await
            .unwrap();
        assert!(applied.verification_passed);
        assert!(!applied.activated);
        assert!(result.description.contains("No migration needed"));
        db.close().await.unwrap();
    }

    #[tokio::test]
    async fn migrate_dry_run_rejects_unregistered_target() {
        let dir = TempDir::new().unwrap();
        let path = object_store::path::Path::from("");
        let store = std::sync::Arc::new(
            object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap(),
        );
        let opts = crate::OpenOptions {
            object_store: store,
            path,
            encryption: None,
        };
        let _catalog = crate::CatalogStore::open(opts).await.unwrap();
        let store2 = std::sync::Arc::new(
            object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap(),
        );
        let db = Db::open(object_store::path::Path::from(""), store2)
            .await
            .unwrap();
        // Format 2 has no typed migration in v0.60.0.
        let result = migrate_dry_run(&db, 2).await.unwrap_err();
        assert!(result.to_string().contains("no registered migration"));
        db.close().await.unwrap();
    }
}
