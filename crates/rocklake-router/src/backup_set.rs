//! Portable, metadata-only backup sets for the managed registry.

use crate::{
    CatalogConfig, CatalogId, CatalogLifecycle, CatalogLocation, CatalogMode, CatalogRegistry,
    RegistryBackupManifest, RegistryError, RouterOpenOptions,
};
use rocklake_catalog::{
    create_backup_with_options, inspect_backup, BackupInfo, BackupOptions, CatalogError,
};
use serde::{Deserialize, Serialize};
use slatedb::Db;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use thiserror::Error;

const SET_MANIFEST_FILE: &str = "backup-set.json";
const REGISTRY_DIR: &str = "registry";
const CATALOGS_DIR: &str = "catalogs";

/// Current backup-set manifest version.
pub const BACKUP_SET_FORMAT_VERSION: u32 = rocklake_core::version::BACKUP_SET_MANIFEST_VERSION;

/// One catalog included in a backup set.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BackupSetCatalog {
    /// Stable catalog identity.
    pub id: CatalogId,
    /// Aliases at backup time.
    pub aliases: Vec<String>,
    /// Source catalog metadata location.
    pub catalog: String,
    /// Referenced data location; data bytes are not copied.
    pub data: String,
    /// Source lifecycle at backup time.
    pub lifecycle: CatalogLifecycle,
    /// Relative catalog backup directory.
    pub backup: String,
}

/// Versioned service backup-set manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BackupSetManifest {
    /// Backup-set format version.
    pub version: u32,
    /// Stable manifest discriminator.
    pub kind: String,
    /// Registry generation captured by the set.
    pub registry_generation: u64,
    /// Registry backup manifest.
    pub registry: RegistryBackupManifest,
    /// Included catalogs.
    pub catalogs: Vec<BackupSetCatalog>,
    /// Software version that created the set.
    pub software_version: String,
    /// RFC 3339 creation time.
    pub created_at: String,
}

/// Validated backup-set information.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupSetInfo {
    /// Backup-set directory.
    pub path: PathBuf,
    /// Validated set manifest.
    pub manifest: BackupSetManifest,
    /// Validated catalog backup artifacts.
    pub catalogs: Vec<BackupInfo>,
}

/// Options for creating a backup set.
#[derive(Clone, Default)]
pub struct BackupSetOptions {
    /// Optional subset of stable catalog IDs. Empty means all non-deleted rows.
    pub catalog_ids: Vec<CatalogId>,
    /// Include referenced object paths and catalog sizes in each manifest.
    pub include_data_inventory: bool,
    /// HEAD referenced objects and fail if any are missing.
    pub verify_data: bool,
    /// Object-store provider options used to resolve catalog locations.
    pub router: RouterOpenOptions,
}

/// Backup-set errors.
#[derive(Debug, Error)]
pub enum BackupSetError {
    /// Registry operation failed.
    #[error(transparent)]
    Registry(#[from] RegistryError),
    /// Catalog operation failed.
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    /// SlateDB operation failed.
    #[error("backup set storage error: {0}")]
    Storage(#[from] slatedb::Error),
    /// Filesystem operation failed.
    #[error("backup set I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The set is malformed or does not match its children.
    #[error("invalid backup set: {0}")]
    Invalid(String),
}

/// Create a metadata-only backup set for all or selected managed catalogs.
pub async fn create_backup_set(
    registry: &CatalogRegistry,
    directory: impl AsRef<Path>,
    options: BackupSetOptions,
) -> Result<BackupSetInfo, BackupSetError> {
    let directory = directory.as_ref();
    let snapshot = registry.snapshot().await?;
    let wanted: BTreeSet<_> = options.catalog_ids.iter().cloned().collect();
    let catalogs: Vec<_> = snapshot
        .catalogs
        .iter()
        .filter(|catalog| catalog.lifecycle != CatalogLifecycle::Deleted)
        .filter(|catalog| wanted.is_empty() || wanted.contains(&catalog.id))
        .cloned()
        .collect();
    if catalogs.is_empty() {
        return Err(BackupSetError::Invalid(
            "backup set contains no selected catalogs".into(),
        ));
    }
    if let Some(missing) = wanted
        .iter()
        .find(|id| !catalogs.iter().any(|catalog| &catalog.id == *id))
    {
        return Err(BackupSetError::Invalid(format!(
            "catalog {missing} is not present in the registry"
        )));
    }

    tokio::fs::create_dir_all(directory.join(CATALOGS_DIR)).await?;
    let registry_info = registry.backup(directory.join(REGISTRY_DIR)).await?;
    let mut children = Vec::with_capacity(catalogs.len());
    let mut child_manifests = Vec::with_capacity(catalogs.len());
    for catalog in catalogs {
        let catalog_location = CatalogLocation::parse(&catalog.catalog)
            .map_err(|e| BackupSetError::Invalid(e.to_string()))?;
        let (path, store) = catalog_location
            .object_store_path(&options.router)
            .map_err(|e| BackupSetError::Invalid(e.to_string()))?;
        let db = Db::open(path, store).await?;
        let (data_store, data_root) = if options.include_data_inventory || options.verify_data {
            let data_location = CatalogLocation::parse(&catalog.data)
                .map_err(|e| BackupSetError::Invalid(e.to_string()))?;
            let (data_root, data_store) = data_location
                .object_store_path(&options.router)
                .map_err(|e| BackupSetError::Invalid(e.to_string()))?;
            (Some(data_store), Some(data_root))
        } else {
            (None, None)
        };
        let relative = format!("{CATALOGS_DIR}/{}", catalog.id);
        let info = create_backup_with_options(
            &db,
            directory.join(&relative),
            BackupOptions {
                source_identity: catalog.catalog.clone(),
                data_store,
                data_root,
                include_data_inventory: options.include_data_inventory,
                verify_data: options.verify_data,
                registry_generation: Some(snapshot.generation),
                catalog_ids: vec![catalog.id.to_string()],
                ..BackupOptions::default()
            },
        )
        .await?;
        db.close().await?;
        children.push(BackupSetCatalog {
            id: catalog.id,
            aliases: catalog.aliases.iter().map(ToString::to_string).collect(),
            catalog: info.manifest.source_identity.clone(),
            data: catalog.data,
            lifecycle: catalog.lifecycle,
            backup: relative,
        });
        child_manifests.push(info);
    }

    let manifest = BackupSetManifest {
        version: BACKUP_SET_FORMAT_VERSION,
        kind: "rocklake_backup_set".into(),
        registry_generation: snapshot.generation,
        registry: registry_info.manifest,
        catalogs: children,
        software_version: env!("CARGO_PKG_VERSION").into(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    tokio::fs::write(
        directory.join(SET_MANIFEST_FILE),
        serde_json::to_vec_pretty(&manifest)
            .map_err(|e| BackupSetError::Invalid(format!("serialize manifest: {e}")))?,
    )
    .await?;
    Ok(BackupSetInfo {
        path: directory.to_path_buf(),
        manifest,
        catalogs: child_manifests,
    })
}

/// Inspect a backup set and validate every checksum and generation link.
pub async fn inspect_backup_set(
    directory: impl AsRef<Path>,
) -> Result<BackupSetInfo, BackupSetError> {
    let directory = directory.as_ref();
    let manifest: BackupSetManifest =
        serde_json::from_slice(&tokio::fs::read(directory.join(SET_MANIFEST_FILE)).await?)
            .map_err(|e| BackupSetError::Invalid(format!("invalid set manifest: {e}")))?;
    if manifest.version != BACKUP_SET_FORMAT_VERSION || manifest.kind != "rocklake_backup_set" {
        return Err(BackupSetError::Invalid(format!(
            "unsupported backup-set manifest version {}",
            manifest.version
        )));
    }
    let registry_info = CatalogRegistry::inspect_backup(directory.join(REGISTRY_DIR)).await?;
    if registry_info.manifest != manifest.registry {
        return Err(BackupSetError::Invalid(
            "registry manifest does not match backup-set manifest".into(),
        ));
    }
    if registry_info.manifest.generation != manifest.registry_generation {
        return Err(BackupSetError::Invalid(
            "registry generation does not match backup-set manifest".into(),
        ));
    }
    let mut catalogs = Vec::with_capacity(manifest.catalogs.len());
    for child in &manifest.catalogs {
        let info = inspect_backup(directory.join(&child.backup)).await?;
        if info.manifest.registry_generation != Some(manifest.registry_generation)
            || info.manifest.catalog_ids != vec![child.id.to_string()]
        {
            return Err(BackupSetError::Invalid(format!(
                "catalog {} does not match backup-set generation or identity",
                child.id
            )));
        }
        catalogs.push(info);
    }
    Ok(BackupSetInfo {
        path: directory.to_path_buf(),
        manifest,
        catalogs,
    })
}

/// Build a route configuration for a restored catalog while keeping it read-only.
pub fn restored_catalog_config(
    catalog: &BackupSetCatalog,
    catalog_location: String,
    data_location: String,
) -> Result<CatalogConfig, BackupSetError> {
    Ok(CatalogConfig {
        id: catalog.id.to_string(),
        aliases: catalog.aliases.clone(),
        catalog: catalog_location,
        data: data_location,
        mode: CatalogMode::ReadOnly,
        credential_provider: "restored-backup".into(),
        policy_reference: Some("backup-verified".into()),
        limits: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::local::LocalFileSystem;
    use object_store::path::Path as ObjectPath;
    use rocklake_catalog::{CatalogStore, OpenOptions};
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn backup_set_round_trip_validates_registry_generation_and_children() {
        let dir = TempDir::new().unwrap();
        let registry_path = dir.path().join("registry");
        let catalog_path = dir.path().join("catalog");
        let data_path = dir.path().join("data");
        std::fs::create_dir_all(&catalog_path).unwrap();
        std::fs::create_dir_all(&data_path).unwrap();
        let registry = CatalogRegistry::open_location(
            registry_path.to_str().unwrap(),
            &RouterOpenOptions::default(),
        )
        .await
        .unwrap();
        registry.init().await.unwrap();
        let catalog_store = CatalogStore::open(OpenOptions {
            object_store: Arc::new(LocalFileSystem::new_with_prefix(&catalog_path).unwrap()),
            path: ObjectPath::from(""),
            encryption: None,
        })
        .await
        .unwrap();
        catalog_store.close().await.unwrap();
        let id = CatalogId::new("00000000-0000-4000-8000-000000000001").unwrap();
        registry
            .create(
                CatalogConfig {
                    id: id.to_string(),
                    aliases: vec!["lake".into()],
                    catalog: catalog_path.to_string_lossy().into_owned(),
                    data: data_path.to_string_lossy().into_owned(),
                    mode: CatalogMode::ReadOnly,
                    credential_provider: "env".into(),
                    policy_reference: None,
                    limits: Default::default(),
                },
                "create-lake",
            )
            .await
            .unwrap();
        let set = create_backup_set(
            &registry,
            dir.path().join("set"),
            BackupSetOptions {
                catalog_ids: vec![id],
                ..BackupSetOptions::default()
            },
        )
        .await
        .unwrap();
        let inspected = inspect_backup_set(&set.path).await.unwrap();
        assert_eq!(inspected.manifest.catalogs.len(), 1);
        assert_eq!(inspected.catalogs.len(), 1);
        registry.close().await.unwrap();
    }
}
