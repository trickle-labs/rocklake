//! Durable managed catalog routing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use object_store::path::Path as ObjectPath;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use slatedb::{Db, ErrorKind, IsolationLevel};
use thiserror::Error;

use super::{
    CatalogAlias, CatalogConfig, CatalogDescriptor, CatalogId, CatalogLimits, CatalogLocation,
    CatalogMode, RouterError, RouterOpenOptions, RouterSettings, StaticConfig,
};

/// Current managed-registry format.
pub const REGISTRY_FORMAT_VERSION: u32 = 1;

const FORMAT_KEY: &[u8] = b"rocklake.registry/format";
const STATE_KEY: &[u8] = b"rocklake.registry/state";
const AUDIT_PREFIX: &[u8] = b"rocklake.registry/audit/";
const BACKUP_DATA_FILE: &str = "registry.json";
const BACKUP_MANIFEST_FILE: &str = "manifest.json";

/// Managed registry configuration embedded in `rocklake.toml`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrySettings {
    /// Registry location URI.
    pub location: String,
    /// Permit serving from the recovery file when the registry is unavailable.
    #[serde(default)]
    pub emergency_read_only: bool,
    /// Optional static TOML recovery file.
    pub recovery_file: Option<PathBuf>,
}

/// Options for opening the registry database.
#[derive(Clone)]
pub struct RegistryOpenOptions {
    /// Object store backing the registry.
    pub object_store: Arc<dyn object_store::ObjectStore>,
    /// SlateDB path within the object store.
    pub path: ObjectPath,
    /// Canonical registry location, used to prove tenant-prefix separation.
    pub registry_location: Option<CatalogLocation>,
    /// Open for verification/recovery only.
    pub read_only: bool,
}

impl RegistryOpenOptions {
    /// Construct options from an already resolved object store and path.
    pub fn new(object_store: Arc<dyn object_store::ObjectStore>, path: ObjectPath) -> Self {
        Self {
            object_store,
            path,
            registry_location: None,
            read_only: false,
        }
    }

    /// Resolve a local or cloud registry location using the router's provider settings.
    pub fn from_location(
        location: &str,
        router_options: &RouterOpenOptions,
    ) -> Result<Self, RouterError> {
        let location = CatalogLocation::parse(location)?;
        if location.scheme() == "file" {
            std::fs::create_dir_all(location.prefix())
                .map_err(|error| RouterError::Open(error.to_string()))?;
        }
        let (path, object_store) = location.object_store_path(router_options)?;
        Ok(Self {
            object_store,
            path,
            registry_location: Some(location),
            read_only: false,
        })
    }

    /// Make this a read-only registry handle.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }
}

/// A catalog's durable lifecycle state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogLifecycle {
    /// The catalog is being prepared.
    Creating,
    /// The catalog accepts normal reads and writes according to its mode.
    Active,
    /// The catalog is serving reads only.
    ReadOnly,
    /// The catalog is detached from routing but retained for recovery.
    Disabled,
    /// Deletion has been requested but routing is detached.
    Deleting,
    /// The route is removed; physical bytes are untouched.
    Deleted,
    /// The catalog needs operator repair before it can be routed.
    Error,
}

impl CatalogLifecycle {
    fn routes(self) -> bool {
        matches!(self, Self::Active | Self::ReadOnly)
    }
}

/// One versioned catalog row in the managed registry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RegistryCatalog {
    /// Stable catalog identity.
    pub id: CatalogId,
    /// Mutable PostgreSQL aliases.
    pub aliases: Vec<CatalogAlias>,
    /// Catalog metadata location.
    pub catalog: String,
    /// Referenced data location.
    pub data: String,
    /// Write mode.
    pub mode: CatalogMode,
    /// Lifecycle state.
    pub lifecycle: CatalogLifecycle,
    /// Named policy reference, never policy contents.
    pub policy_reference: Option<String>,
    /// Named credential provider, never credentials.
    pub credential_provider: String,
    /// Per-catalog limits.
    pub limits: CatalogLimits,
    /// RFC 3339 creation timestamp.
    pub created_at: String,
    /// Registry generation that last changed this row.
    pub updated_generation: u64,
    /// Whether this row is a deletion tombstone.
    pub tombstone: bool,
}

impl RegistryCatalog {
    fn descriptor(&self) -> Result<CatalogDescriptor, RegistryError> {
        CatalogConfig {
            id: self.id.to_string(),
            aliases: self.aliases.iter().map(ToString::to_string).collect(),
            catalog: self.catalog.clone(),
            data: self.data.clone(),
            mode: self.mode,
            credential_provider: self.credential_provider.clone(),
            policy_reference: self.policy_reference.clone(),
            limits: self.limits.clone(),
        }
        .try_into()
        .map_err(RegistryError::Router)
    }

    fn from_config(
        config: CatalogConfig,
        lifecycle: CatalogLifecycle,
    ) -> Result<Self, RegistryError> {
        let descriptor: CatalogDescriptor = config.try_into().map_err(RegistryError::Router)?;
        let mut aliases = BTreeSet::new();
        for alias in &descriptor.aliases {
            if !aliases.insert(alias.clone()) {
                return Err(RegistryError::InvalidInput(format!(
                    "duplicate alias '{alias}'"
                )));
            }
        }
        Ok(Self {
            id: descriptor.id,
            aliases: aliases.into_iter().collect(),
            catalog: descriptor.catalog.display_uri(),
            data: descriptor.data.display_uri(),
            mode: descriptor.mode,
            lifecycle,
            policy_reference: descriptor.policy_reference,
            credential_provider: descriptor.credential_provider,
            limits: descriptor.limits,
            created_at: Utc::now().to_rfc3339(),
            updated_generation: 0,
            tombstone: false,
        })
    }
}

/// One immutable registry audit event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RegistryAuditEntry {
    /// Monotonically increasing sequence number.
    pub sequence: u64,
    /// Management operation name.
    pub action: String,
    /// Affected stable catalog identity, if any.
    pub catalog_id: Option<CatalogId>,
    /// Caller-provided idempotency key.
    pub request_id: String,
    /// Resulting registry generation.
    pub generation: u64,
    /// RFC 3339 event timestamp.
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RegistryRequest {
    action: String,
    catalog_id: Option<CatalogId>,
    generation: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RegistryState {
    format_version: u32,
    generation: u64,
    next_audit_sequence: u64,
    default_catalog: Option<CatalogAlias>,
    catalogs: BTreeMap<CatalogId, RegistryCatalog>,
    alias_index: BTreeMap<CatalogAlias, CatalogId>,
    tombstones: BTreeMap<CatalogAlias, u64>,
    // ponytail: request dedupe is an in-state map; compact it when management volume needs it.
    requests: BTreeMap<String, RegistryRequest>,
}

impl RegistryState {
    fn empty() -> Self {
        Self {
            format_version: REGISTRY_FORMAT_VERSION,
            generation: 0,
            next_audit_sequence: 1,
            default_catalog: None,
            catalogs: BTreeMap::new(),
            alias_index: BTreeMap::new(),
            tombstones: BTreeMap::new(),
            requests: BTreeMap::new(),
        }
    }
}

/// A point-in-time managed registry view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrySnapshot {
    /// Registry format version.
    pub format_version: u32,
    /// Monotonic snapshot generation.
    pub generation: u64,
    /// Default routed alias.
    pub default_catalog: Option<CatalogAlias>,
    /// All rows, including disabled and deleted tombstones.
    pub catalogs: Vec<RegistryCatalog>,
    /// Next audit sequence number.
    pub next_audit_sequence: u64,
}

/// Summary returned by registry status and verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryStatus {
    /// Registry format version.
    pub format_version: u32,
    /// Current generation.
    pub generation: u64,
    /// Total durable rows.
    pub catalogs: usize,
    /// Rows currently published to routing.
    pub routed_catalogs: usize,
    /// Next audit sequence number.
    pub next_audit_sequence: u64,
}

/// A mutation result, including whether a request was replayed idempotently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryMutation {
    /// Resulting generation.
    pub generation: u64,
    /// Affected catalog identity, if any.
    pub catalog_id: Option<CatalogId>,
    /// Current lifecycle state, if a catalog was affected.
    pub lifecycle: Option<CatalogLifecycle>,
    /// True when the request ID was already committed.
    pub replayed: bool,
}

/// A versioned registry backup manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RegistryBackupManifest {
    /// Backup format version.
    pub version: u32,
    /// Registry format included in the payload.
    pub registry_format_version: u32,
    /// Registry generation included in the payload.
    pub generation: u64,
    /// RFC 3339 creation timestamp.
    pub created_at: String,
    /// Payload byte count.
    pub byte_count: u64,
    /// Lowercase SHA-256 payload digest.
    pub sha256: String,
}

/// Validated registry backup information.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryBackupInfo {
    /// Backup directory.
    pub path: PathBuf,
    /// Validated manifest.
    pub manifest: RegistryBackupManifest,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RegistryBackupPayload {
    state: RegistryState,
    audit: Vec<RegistryAuditEntry>,
}

/// Registry errors.
#[derive(Debug, Error)]
pub enum RegistryError {
    /// Router validation failed.
    #[error(transparent)]
    Router(#[from] RouterError),
    /// SlateDB returned an error.
    #[error("registry storage error: {0}")]
    Storage(#[from] slatedb::Error),
    /// Registry state is malformed or unsafe.
    #[error("registry corruption: {0}")]
    Corrupt(String),
    /// Registry has not been initialized.
    #[error("registry is not initialized")]
    NotInitialized,
    /// The requested operation is not allowed in this lifecycle state.
    #[error("invalid catalog lifecycle transition for {id}: {state:?}")]
    InvalidTransition {
        id: CatalogId,
        state: CatalogLifecycle,
    },
    /// A stable ID already exists.
    #[error("catalog already exists: {0}")]
    CatalogExists(CatalogId),
    /// A requested catalog was not found.
    #[error("catalog not found: {0}")]
    CatalogNotFound(String),
    /// An alias is already reserved by another durable row.
    #[error("catalog alias already reserved: {0}")]
    AliasExists(CatalogAlias),
    /// A request ID was reused for another operation.
    #[error("request ID was already used for another operation: {0}")]
    RequestConflict(String),
    /// The caller's expected generation did not match.
    #[error("registry generation conflict")]
    GenerationConflict,
    /// Concurrent registry commits could not complete within the retry budget.
    #[error("registry transaction conflict")]
    Conflict,
    /// A mutation was attempted through a recovery-only handle.
    #[error("registry is read-only")]
    ReadOnly,
    /// A caller supplied invalid input.
    #[error("invalid registry input: {0}")]
    InvalidInput(String),
}

/// Versioned object-store-backed catalog registry.
pub struct CatalogRegistry {
    db: Db,
    options: RegistryOpenOptions,
}

impl CatalogRegistry {
    /// Open a registry database without changing its state.
    pub async fn open(options: RegistryOpenOptions) -> Result<Self, RegistryError> {
        let db = Db::open(options.path.clone(), options.object_store.clone()).await?;
        Ok(Self { db, options })
    }

    /// Open a registry directly from a location URI.
    pub async fn open_location(
        location: &str,
        router_options: &RouterOpenOptions,
    ) -> Result<Self, RegistryError> {
        Self::open(RegistryOpenOptions::from_location(
            location,
            router_options,
        )?)
        .await
    }

    /// Initialize an empty registry. Repeated initialization is safe.
    pub async fn init(&self) -> Result<RegistrySnapshot, RegistryError> {
        if self.options.read_only {
            return Err(RegistryError::ReadOnly);
        }
        for _ in 0..16 {
            let tx = self.db.begin(IsolationLevel::SerializableSnapshot).await?;
            let format = tx.get(FORMAT_KEY).await?;
            let state = tx.get(STATE_KEY).await?;
            if let Some(format) = format {
                let found = decode_u32(&format)?;
                if found != REGISTRY_FORMAT_VERSION {
                    return Err(RegistryError::Corrupt(format!(
                        "unsupported registry format {found} (expected {REGISTRY_FORMAT_VERSION})"
                    )));
                }
                let state = state.ok_or_else(|| RegistryError::Corrupt("missing state".into()))?;
                let state = decode_state(&state)?;
                validate_state(&state, self.options.registry_location.as_ref())?;
                return Ok(snapshot_from_state(&state));
            }
            if state.is_some() {
                return Err(RegistryError::Corrupt(
                    "state exists without format marker".into(),
                ));
            }
            tx.put(FORMAT_KEY, REGISTRY_FORMAT_VERSION.to_be_bytes())?;
            tx.put(STATE_KEY, encode(&RegistryState::empty())?)?;
            match tx.commit().await {
                Ok(_) => return Ok(snapshot_from_state(&RegistryState::empty())),
                Err(error) if error.kind() == ErrorKind::Transaction => {
                    tokio::task::yield_now().await;
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(RegistryError::Conflict)
    }

    /// Return the current durable registry snapshot.
    pub async fn snapshot(&self) -> Result<RegistrySnapshot, RegistryError> {
        Ok(snapshot_from_state(&self.load_state().await?))
    }

    /// Return current registry counters.
    pub async fn status(&self) -> Result<RegistryStatus, RegistryError> {
        let state = self.load_state().await?;
        Ok(RegistryStatus {
            format_version: state.format_version,
            generation: state.generation,
            catalogs: state.catalogs.len(),
            routed_catalogs: state
                .catalogs
                .values()
                .filter(|row| row.lifecycle.routes())
                .count(),
            next_audit_sequence: state.next_audit_sequence,
        })
    }

    /// Return immutable audit entries in sequence order.
    pub async fn audit(&self) -> Result<Vec<RegistryAuditEntry>, RegistryError> {
        self.load_state().await?;
        let mut iter = self.db.scan_prefix(AUDIT_PREFIX).await?;
        let mut entries = Vec::new();
        while let Some(kv) = iter.next().await? {
            entries.push(serde_json::from_slice(&kv.value).map_err(|error| {
                RegistryError::Corrupt(format!("invalid audit entry: {error}"))
            })?);
        }
        entries.sort_by_key(|entry: &RegistryAuditEntry| entry.sequence);
        Ok(entries)
    }

    /// Create and publish a new lazily-created catalog route.
    pub async fn create(
        &self,
        config: CatalogConfig,
        request_id: &str,
    ) -> Result<RegistryMutation, RegistryError> {
        self.add_catalog("create", config, request_id).await
    }

    /// Register an existing catalog route without copying or deleting data.
    pub async fn register(
        &self,
        config: CatalogConfig,
        request_id: &str,
    ) -> Result<RegistryMutation, RegistryError> {
        self.add_catalog("register", config, request_id).await
    }

    async fn add_catalog(
        &self,
        action: &str,
        config: CatalogConfig,
        request_id: &str,
    ) -> Result<RegistryMutation, RegistryError> {
        let lifecycle = if config.mode == CatalogMode::ReadOnly {
            CatalogLifecycle::ReadOnly
        } else {
            CatalogLifecycle::Active
        };
        let row = RegistryCatalog::from_config(config, lifecycle)?;
        let id = row.id.clone();
        self.mutate(request_id, None, action, move |state| {
            if state.catalogs.contains_key(&id) {
                return Err(RegistryError::CatalogExists(id.clone()));
            }
            ensure_aliases_free(state, &row.aliases, None)?;
            state.catalogs.insert(id.clone(), row.clone());
            if state.default_catalog.is_none() {
                state.default_catalog = row.aliases.first().cloned();
            }
            Ok(Some(id.clone()))
        })
        .await
    }

    /// Rename a catalog's routing alias while preserving its stable identity and location.
    pub async fn rename(
        &self,
        id: &CatalogId,
        alias: &str,
        request_id: &str,
    ) -> Result<RegistryMutation, RegistryError> {
        let alias = CatalogAlias::new(alias)?;
        let id = id.clone();
        self.mutate(request_id, None, "rename", move |state| {
            let row = state
                .catalogs
                .get(&id)
                .ok_or_else(|| RegistryError::CatalogNotFound(id.to_string()))?;
            if row.lifecycle == CatalogLifecycle::Deleted {
                return Err(RegistryError::InvalidTransition {
                    id: id.clone(),
                    state: row.lifecycle,
                });
            }
            let was_default = state
                .default_catalog
                .as_ref()
                .is_some_and(|default| row.aliases.contains(default));
            ensure_aliases_free(state, std::slice::from_ref(&alias), Some(&id))?;
            let row = state.catalogs.get_mut(&id).expect("row checked above");
            row.aliases = vec![alias.clone()];
            if was_default {
                state.default_catalog = Some(alias.clone());
            }
            Ok(Some(id.clone()))
        })
        .await
    }

    /// Change a catalog's read/write mode.
    pub async fn set_mode(
        &self,
        id: &CatalogId,
        mode: CatalogMode,
        request_id: &str,
    ) -> Result<RegistryMutation, RegistryError> {
        let id = id.clone();
        self.mutate(request_id, None, "set-mode", move |state| {
            let row = state
                .catalogs
                .get_mut(&id)
                .ok_or_else(|| RegistryError::CatalogNotFound(id.to_string()))?;
            if matches!(
                row.lifecycle,
                CatalogLifecycle::Deleted | CatalogLifecycle::Deleting
            ) {
                return Err(RegistryError::InvalidTransition {
                    id: id.clone(),
                    state: row.lifecycle,
                });
            }
            row.mode = mode;
            if row.lifecycle.routes() {
                row.lifecycle = if mode == CatalogMode::ReadOnly {
                    CatalogLifecycle::ReadOnly
                } else {
                    CatalogLifecycle::Active
                };
            }
            Ok(Some(id.clone()))
        })
        .await
    }

    /// Disable a route without touching catalog or data bytes.
    pub async fn disable(
        &self,
        id: &CatalogId,
        request_id: &str,
    ) -> Result<RegistryMutation, RegistryError> {
        self.transition(id, CatalogLifecycle::Disabled, request_id, "disable")
            .await
    }

    /// Re-enable a disabled route using its configured mode.
    pub async fn enable(
        &self,
        id: &CatalogId,
        request_id: &str,
    ) -> Result<RegistryMutation, RegistryError> {
        let id = id.clone();
        self.mutate(request_id, None, "enable", move |state| {
            let aliases = state
                .catalogs
                .get(&id)
                .ok_or_else(|| RegistryError::CatalogNotFound(id.to_string()))?;
            if aliases.lifecycle != CatalogLifecycle::Disabled {
                return Err(RegistryError::InvalidTransition {
                    id: id.clone(),
                    state: aliases.lifecycle,
                });
            }
            let aliases = aliases.aliases.clone();
            ensure_aliases_free(state, &aliases, Some(&id))?;
            let row = state.catalogs.get_mut(&id).expect("row checked above");
            row.lifecycle = if row.mode == CatalogMode::ReadOnly {
                CatalogLifecycle::ReadOnly
            } else {
                CatalogLifecycle::Active
            };
            Ok(Some(id.clone()))
        })
        .await
    }

    /// Remove a route and retain a durable tombstone; no physical deletion occurs.
    pub async fn remove(
        &self,
        id: &CatalogId,
        request_id: &str,
    ) -> Result<RegistryMutation, RegistryError> {
        let id = id.clone();
        self.mutate(request_id, None, "remove", move |state| {
            let row = state
                .catalogs
                .get_mut(&id)
                .ok_or_else(|| RegistryError::CatalogNotFound(id.to_string()))?;
            if row.lifecycle == CatalogLifecycle::Deleted {
                return Err(RegistryError::InvalidTransition {
                    id: id.clone(),
                    state: row.lifecycle,
                });
            }
            for alias in row.aliases.drain(..) {
                state.tombstones.insert(alias, state.generation + 1);
            }
            row.lifecycle = CatalogLifecycle::Deleted;
            row.tombstone = true;
            if state
                .default_catalog
                .as_ref()
                .is_some_and(|alias| !state.alias_index.contains_key(alias))
            {
                state.default_catalog = None;
            }
            Ok(Some(id.clone()))
        })
        .await
    }

    /// Import the v0.54 static route table into an empty managed registry.
    pub async fn migrate_static(
        &self,
        config: StaticConfig,
        request_id: &str,
    ) -> Result<RegistryMutation, RegistryError> {
        self.mutate(request_id, None, "migrate-static", move |state| {
            if !state.catalogs.is_empty() {
                return Err(RegistryError::InvalidInput(
                    "static migration requires an empty registry".into(),
                ));
            }
            for descriptor in &config.catalogs {
                let row = RegistryCatalog::from_config(
                    CatalogConfig {
                        id: descriptor.id.to_string(),
                        aliases: descriptor.aliases.iter().map(ToString::to_string).collect(),
                        catalog: descriptor.catalog.display_uri(),
                        data: descriptor.data.display_uri(),
                        mode: descriptor.mode,
                        credential_provider: descriptor.credential_provider.clone(),
                        policy_reference: descriptor.policy_reference.clone(),
                        limits: descriptor.limits.clone(),
                    },
                    if descriptor.mode == CatalogMode::ReadOnly {
                        CatalogLifecycle::ReadOnly
                    } else {
                        CatalogLifecycle::Active
                    },
                )?;
                ensure_aliases_free(state, &row.aliases, None)?;
                state.catalogs.insert(row.id.clone(), row);
            }
            state.default_catalog = config
                .settings
                .default_catalog
                .as_deref()
                .map(CatalogAlias::new)
                .transpose()?;
            Ok(None)
        })
        .await
    }

    /// Load routable registry rows as an immutable router configuration.
    pub async fn static_config(&self) -> Result<StaticConfig, RegistryError> {
        let state = self.load_state().await?;
        let descriptors = state
            .catalogs
            .values()
            .filter(|row| row.lifecycle.routes())
            .map(RegistryCatalog::descriptor)
            .collect::<Result<Vec<_>, _>>()?;
        if descriptors.is_empty() {
            return Err(RegistryError::InvalidInput(
                "registry has no enabled catalogs".into(),
            ));
        }
        Ok(StaticConfig::from_descriptors_at_generation(
            RouterSettings {
                mode: "registry".into(),
                default_catalog: state.default_catalog.map(|alias| alias.to_string()),
                ..RouterSettings::default()
            },
            descriptors,
            state.generation,
        )?)
    }

    /// Create a self-contained local registry backup.
    pub async fn backup(
        &self,
        directory: impl AsRef<Path>,
    ) -> Result<RegistryBackupInfo, RegistryError> {
        let directory = directory.as_ref();
        let state = self.load_state().await?;
        let audit = self.audit().await?;
        validate_audit(&state, &audit)?;
        let payload = encode(&RegistryBackupPayload {
            state: state.clone(),
            audit,
        })?;
        tokio::fs::create_dir_all(directory)
            .await
            .map_err(|error| {
                RegistryError::InvalidInput(format!("create backup directory: {error}"))
            })?;
        tokio::fs::write(directory.join(BACKUP_DATA_FILE), &payload)
            .await
            .map_err(|error| RegistryError::InvalidInput(format!("write backup data: {error}")))?;
        let manifest = RegistryBackupManifest {
            version: REGISTRY_FORMAT_VERSION,
            registry_format_version: state.format_version,
            generation: state.generation,
            created_at: Utc::now().to_rfc3339(),
            byte_count: payload.len() as u64,
            sha256: digest_hex(Sha256::digest(&payload)),
        };
        tokio::fs::write(
            directory.join(BACKUP_MANIFEST_FILE),
            serde_json::to_vec_pretty(&manifest).map_err(|error| {
                RegistryError::InvalidInput(format!("serialize backup manifest: {error}"))
            })?,
        )
        .await
        .map_err(|error| RegistryError::InvalidInput(format!("write backup manifest: {error}")))?;
        Ok(RegistryBackupInfo {
            path: directory.to_path_buf(),
            manifest,
        })
    }

    /// Inspect a local registry backup without opening a registry.
    pub async fn inspect_backup(
        directory: impl AsRef<Path>,
    ) -> Result<RegistryBackupInfo, RegistryError> {
        let directory = directory.as_ref();
        let manifest: RegistryBackupManifest = serde_json::from_slice(
            &tokio::fs::read(directory.join(BACKUP_MANIFEST_FILE))
                .await
                .map_err(|error| {
                    RegistryError::InvalidInput(format!("read backup manifest: {error}"))
                })?,
        )
        .map_err(|error| RegistryError::Corrupt(format!("invalid backup manifest: {error}")))?;
        if manifest.version != REGISTRY_FORMAT_VERSION {
            return Err(RegistryError::Corrupt(format!(
                "unsupported backup version {} (expected {REGISTRY_FORMAT_VERSION})",
                manifest.version
            )));
        }
        let payload = tokio::fs::read(directory.join(BACKUP_DATA_FILE))
            .await
            .map_err(|error| RegistryError::InvalidInput(format!("read backup data: {error}")))?;
        if manifest.byte_count != payload.len() as u64
            || manifest.sha256 != digest_hex(Sha256::digest(&payload))
        {
            return Err(RegistryError::Corrupt(
                "backup checksum or byte count mismatch".into(),
            ));
        }
        let payload: RegistryBackupPayload = serde_json::from_slice(&payload)
            .map_err(|error| RegistryError::Corrupt(format!("invalid backup payload: {error}")))?;
        if payload.state.format_version != manifest.registry_format_version
            || payload.state.generation != manifest.generation
        {
            return Err(RegistryError::Corrupt(
                "backup manifest does not match payload".into(),
            ));
        }
        validate_state(&payload.state, None)?;
        validate_audit(&payload.state, &payload.audit)?;
        Ok(RegistryBackupInfo {
            path: directory.to_path_buf(),
            manifest,
        })
    }

    /// Restore a verified backup into an uninitialized registry.
    pub async fn restore(
        &self,
        directory: impl AsRef<Path>,
    ) -> Result<RegistrySnapshot, RegistryError> {
        if self.options.read_only {
            return Err(RegistryError::ReadOnly);
        }
        let directory = directory.as_ref();
        Self::inspect_backup(directory).await?;
        let payload: RegistryBackupPayload = serde_json::from_slice(
            &tokio::fs::read(directory.join(BACKUP_DATA_FILE))
                .await
                .map_err(|error| {
                    RegistryError::InvalidInput(format!("read backup data: {error}"))
                })?,
        )
        .map_err(|error| RegistryError::Corrupt(format!("invalid backup payload: {error}")))?;
        validate_state(&payload.state, self.options.registry_location.as_ref())?;
        for _ in 0..16 {
            let tx = self.db.begin(IsolationLevel::SerializableSnapshot).await?;
            if tx.get(STATE_KEY).await?.is_some() || tx.get(FORMAT_KEY).await?.is_some() {
                return Err(RegistryError::InvalidInput(
                    "restore destination must be an uninitialized registry".into(),
                ));
            }
            tx.put(FORMAT_KEY, payload.state.format_version.to_be_bytes())?;
            tx.put(STATE_KEY, encode(&payload.state)?)?;
            for entry in &payload.audit {
                tx.put(audit_key(entry.sequence), encode(entry)?)?;
            }
            match tx.commit().await {
                Ok(_) => return Ok(snapshot_from_state(&payload.state)),
                Err(error) if error.kind() == ErrorKind::Transaction => {
                    tokio::task::yield_now().await;
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(RegistryError::Conflict)
    }

    /// Verify state, indexes, audit sequence, and prefix isolation.
    pub async fn verify(&self) -> Result<RegistryStatus, RegistryError> {
        let state = self.load_state().await?;
        let audit = self.audit().await?;
        validate_audit(&state, &audit)?;
        Ok(RegistryStatus {
            format_version: state.format_version,
            generation: state.generation,
            catalogs: state.catalogs.len(),
            routed_catalogs: state
                .catalogs
                .values()
                .filter(|row| row.lifecycle.routes())
                .count(),
            next_audit_sequence: state.next_audit_sequence,
        })
    }

    /// Close the underlying SlateDB registry.
    pub async fn close(self) -> Result<(), RegistryError> {
        self.db.close().await?;
        Ok(())
    }

    async fn load_state(&self) -> Result<RegistryState, RegistryError> {
        let format = self
            .db
            .get(FORMAT_KEY)
            .await?
            .ok_or(RegistryError::NotInitialized)?;
        let found = decode_u32(&format)?;
        if found != REGISTRY_FORMAT_VERSION {
            return Err(RegistryError::Corrupt(format!(
                "unsupported registry format {found} (expected {REGISTRY_FORMAT_VERSION})"
            )));
        }
        let state = self
            .db
            .get(STATE_KEY)
            .await?
            .ok_or_else(|| RegistryError::Corrupt("missing state".into()))?;
        let state = decode_state(&state)?;
        validate_state(&state, self.options.registry_location.as_ref())?;
        Ok(state)
    }

    async fn transition(
        &self,
        id: &CatalogId,
        target: CatalogLifecycle,
        request_id: &str,
        action: &str,
    ) -> Result<RegistryMutation, RegistryError> {
        let id = id.clone();
        self.mutate(request_id, None, action, move |state| {
            let row = state
                .catalogs
                .get_mut(&id)
                .ok_or_else(|| RegistryError::CatalogNotFound(id.to_string()))?;
            if !matches!(
                (row.lifecycle, target),
                (
                    CatalogLifecycle::Active | CatalogLifecycle::ReadOnly,
                    CatalogLifecycle::Disabled
                )
            ) {
                return Err(RegistryError::InvalidTransition {
                    id: id.clone(),
                    state: row.lifecycle,
                });
            }
            row.lifecycle = target;
            Ok(Some(id.clone()))
        })
        .await
    }

    async fn mutate<F>(
        &self,
        request_id: &str,
        expected_generation: Option<u64>,
        action: &str,
        apply: F,
    ) -> Result<RegistryMutation, RegistryError>
    where
        F: Fn(&mut RegistryState) -> Result<Option<CatalogId>, RegistryError> + Send + Sync,
    {
        if self.options.read_only {
            return Err(RegistryError::ReadOnly);
        }
        validate_request_id(request_id)?;
        for _ in 0..16 {
            let tx = self.db.begin(IsolationLevel::SerializableSnapshot).await?;
            let state_bytes = tx
                .get(STATE_KEY)
                .await?
                .ok_or(RegistryError::NotInitialized)?;
            let mut state = decode_state(&state_bytes)?;
            validate_state(&state, self.options.registry_location.as_ref())?;
            if let Some(request) = state.requests.get(request_id) {
                if request.action != action {
                    return Err(RegistryError::RequestConflict(request_id.into()));
                }
                return Ok(mutation_from_state(&state, request, true));
            }
            if expected_generation.is_some_and(|generation| generation != state.generation) {
                return Err(RegistryError::GenerationConflict);
            }
            let catalog_id = apply(&mut state)?;
            state.generation = state.generation.saturating_add(1);
            if let Some(id) = &catalog_id {
                if let Some(row) = state.catalogs.get_mut(id) {
                    row.updated_generation = state.generation;
                }
            }
            rebuild_alias_index(&mut state)?;
            validate_state(&state, self.options.registry_location.as_ref())?;
            let request = RegistryRequest {
                action: action.into(),
                catalog_id: catalog_id.clone(),
                generation: state.generation,
            };
            state.requests.insert(request_id.into(), request.clone());
            let audit = RegistryAuditEntry {
                sequence: state.next_audit_sequence,
                action: action.into(),
                catalog_id: catalog_id.clone(),
                request_id: request_id.into(),
                generation: state.generation,
                created_at: Utc::now().to_rfc3339(),
            };
            state.next_audit_sequence = state.next_audit_sequence.saturating_add(1);
            tx.put(STATE_KEY, encode(&state)?)?;
            tx.put(audit_key(audit.sequence), encode(&audit)?)?;
            match tx.commit().await {
                Ok(_) => {
                    return Ok(mutation_from_state(&state, &request, false));
                }
                Err(error) if error.kind() == ErrorKind::Transaction => {
                    tokio::task::yield_now().await;
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(RegistryError::Conflict)
    }
}

fn mutation_from_state(
    state: &RegistryState,
    request: &RegistryRequest,
    replayed: bool,
) -> RegistryMutation {
    RegistryMutation {
        generation: request.generation,
        catalog_id: request.catalog_id.clone(),
        lifecycle: request
            .catalog_id
            .as_ref()
            .and_then(|id| state.catalogs.get(id))
            .map(|row| row.lifecycle),
        replayed,
    }
}

fn validate_request_id(request_id: &str) -> Result<(), RegistryError> {
    if request_id.is_empty()
        || request_id.len() > 128
        || !request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
    {
        return Err(RegistryError::InvalidInput(
            "request ID must be 1-128 ASCII letters, digits, '.', '_', ':', or '-'".into(),
        ));
    }
    Ok(())
}

fn ensure_aliases_free(
    state: &RegistryState,
    aliases: &[CatalogAlias],
    except: Option<&CatalogId>,
) -> Result<(), RegistryError> {
    for alias in aliases {
        for (id, row) in &state.catalogs {
            if except.is_some_and(|except| except == id)
                || row.lifecycle == CatalogLifecycle::Deleted
            {
                continue;
            }
            if row.aliases.contains(alias) {
                return Err(RegistryError::AliasExists(alias.clone()));
            }
        }
    }
    Ok(())
}

fn rebuild_alias_index(state: &mut RegistryState) -> Result<(), RegistryError> {
    let mut index = BTreeMap::new();
    for (id, row) in &state.catalogs {
        if !row.lifecycle.routes() {
            continue;
        }
        for alias in &row.aliases {
            if index.insert(alias.clone(), id.clone()).is_some() {
                return Err(RegistryError::AliasExists(alias.clone()));
            }
        }
    }
    state.alias_index = index;
    if state
        .default_catalog
        .as_ref()
        .is_some_and(|alias| !state.alias_index.contains_key(alias))
    {
        state.default_catalog = state.alias_index.keys().next().cloned();
    }
    Ok(())
}

fn validate_state(
    state: &RegistryState,
    registry_location: Option<&CatalogLocation>,
) -> Result<(), RegistryError> {
    if state.format_version != REGISTRY_FORMAT_VERSION {
        return Err(RegistryError::Corrupt(format!(
            "unsupported registry format {} (expected {REGISTRY_FORMAT_VERSION})",
            state.format_version
        )));
    }
    let mut expected_index = BTreeMap::new();
    let mut reserved = BTreeMap::new();
    let mut locations = Vec::new();
    for (id, row) in &state.catalogs {
        if id != &row.id {
            return Err(RegistryError::Corrupt(format!(
                "catalog row key {id} does not match row ID {}",
                row.id
            )));
        }
        if row.tombstone != (row.lifecycle == CatalogLifecycle::Deleted) {
            return Err(RegistryError::Corrupt(format!(
                "catalog {id} has inconsistent tombstone state"
            )));
        }
        let (catalog_location, data_location) = if row.lifecycle == CatalogLifecycle::Deleted {
            (
                CatalogLocation::parse(&row.catalog)?,
                CatalogLocation::parse(&row.data)?,
            )
        } else {
            let descriptor = row.descriptor()?;
            (descriptor.catalog, descriptor.data)
        };
        if row.lifecycle != CatalogLifecycle::Deleted && row.aliases.is_empty() {
            return Err(RegistryError::Corrupt(format!(
                "catalog {id} has no aliases"
            )));
        }
        for alias in &row.aliases {
            if row.lifecycle != CatalogLifecycle::Deleted
                && reserved.insert(alias.clone(), id.clone()).is_some()
            {
                return Err(RegistryError::Corrupt(format!(
                    "alias {alias} is reserved by multiple catalogs"
                )));
            }
            if row.lifecycle.routes() && expected_index.insert(alias.clone(), id.clone()).is_some()
            {
                return Err(RegistryError::Corrupt(format!(
                    "routed alias {alias} is assigned more than once"
                )));
            }
        }
        if let Some(registry_location) = registry_location {
            if catalog_location.overlaps(registry_location)
                || data_location.overlaps(registry_location)
            {
                return Err(RegistryError::Corrupt(format!(
                    "catalog {id} overlaps the registry location"
                )));
            }
        }
        locations.push(catalog_location);
        locations.push(data_location);
    }
    for (left_index, left) in locations.iter().enumerate() {
        if locations
            .iter()
            .skip(left_index + 1)
            .any(|right| left.overlaps(right))
        {
            return Err(RegistryError::Corrupt(format!(
                "catalog location {} overlaps another location",
                left.display_uri()
            )));
        }
    }
    if expected_index != state.alias_index {
        return Err(RegistryError::Corrupt("alias index is stale".into()));
    }
    if state
        .default_catalog
        .as_ref()
        .is_some_and(|alias| !expected_index.contains_key(alias))
    {
        return Err(RegistryError::Corrupt(
            "default catalog is not routed".into(),
        ));
    }
    Ok(())
}

fn validate_audit(
    state: &RegistryState,
    audit: &[RegistryAuditEntry],
) -> Result<(), RegistryError> {
    let expected = state.next_audit_sequence.saturating_sub(1) as usize;
    if audit.len() != expected {
        return Err(RegistryError::Corrupt("audit sequence has a gap".into()));
    }
    for (index, entry) in audit.iter().enumerate() {
        if entry.sequence != (index + 1) as u64 || entry.generation == 0 {
            return Err(RegistryError::Corrupt(
                "audit sequence is not contiguous".into(),
            ));
        }
    }
    Ok(())
}

fn snapshot_from_state(state: &RegistryState) -> RegistrySnapshot {
    RegistrySnapshot {
        format_version: state.format_version,
        generation: state.generation,
        default_catalog: state.default_catalog.clone(),
        catalogs: state.catalogs.values().cloned().collect(),
        next_audit_sequence: state.next_audit_sequence,
    }
}

fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, RegistryError> {
    serde_json::to_vec(value)
        .map_err(|error| RegistryError::Corrupt(format!("serialize registry state: {error}")))
}

fn decode_state(bytes: &[u8]) -> Result<RegistryState, RegistryError> {
    serde_json::from_slice(bytes)
        .map_err(|error| RegistryError::Corrupt(format!("invalid registry state: {error}")))
}

fn decode_u32(bytes: &[u8]) -> Result<u32, RegistryError> {
    let bytes: [u8; 4] = bytes
        .try_into()
        .map_err(|_| RegistryError::Corrupt("invalid registry format marker".into()))?;
    Ok(u32::from_be_bytes(bytes))
}

fn audit_key(sequence: u64) -> Vec<u8> {
    format!("{}{:020}", String::from_utf8_lossy(AUDIT_PREFIX), sequence).into_bytes()
}

fn digest_hex(digest: impl AsRef<[u8]>) -> String {
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn config(id: &str, alias: &str, root: &Path) -> CatalogConfig {
        CatalogConfig {
            id: id.into(),
            aliases: vec![alias.into()],
            catalog: root.join("catalog").to_string_lossy().into_owned(),
            data: root.join("data").to_string_lossy().into_owned(),
            mode: CatalogMode::ReadWrite,
            credential_provider: "env".into(),
            policy_reference: Some("policy-a".into()),
            limits: CatalogLimits::default(),
        }
    }

    async fn registry() -> (TempDir, CatalogRegistry) {
        let dir = tempfile::tempdir().unwrap();
        let location = dir.path().join("registry");
        let router_options = RouterOpenOptions::default();
        let registry = CatalogRegistry::open_location(location.to_str().unwrap(), &router_options)
            .await
            .unwrap();
        registry.init().await.unwrap();
        (dir, registry)
    }

    #[tokio::test]
    async fn lifecycle_is_idempotent_and_remove_preserves_tombstone() {
        let (dir, registry) = registry().await;
        let row = config("018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9", "main", dir.path());
        let created = registry.create(row, "req-1").await.unwrap();
        let replay = registry
            .create(
                config("018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9", "main", dir.path()),
                "req-1",
            )
            .await
            .unwrap();
        assert!(!created.replayed);
        assert!(replay.replayed);
        let id: CatalogId = "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9".parse().unwrap();
        registry.disable(&id, "req-2").await.unwrap();
        registry.enable(&id, "req-3").await.unwrap();
        registry.remove(&id, "req-4").await.unwrap();
        let snapshot = registry.snapshot().await.unwrap();
        assert_eq!(snapshot.catalogs[0].lifecycle, CatalogLifecycle::Deleted);
        assert!(snapshot.catalogs[0].aliases.is_empty());
        assert_eq!(registry.audit().await.unwrap().len(), 4);
        registry
            .create(
                CatalogConfig {
                    id: "018f4f4d-d520-7d91-b9f0-7018b7b50d13".into(),
                    aliases: vec!["main".into()],
                    catalog: dir
                        .path()
                        .join("replacement-catalog")
                        .to_string_lossy()
                        .into_owned(),
                    data: dir
                        .path()
                        .join("replacement-data")
                        .to_string_lossy()
                        .into_owned(),
                    mode: CatalogMode::ReadWrite,
                    credential_provider: "env".into(),
                    policy_reference: None,
                    limits: CatalogLimits::default(),
                },
                "req-5",
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn backup_restore_round_trip_and_prefixes_are_separate() {
        let (dir, registry) = registry().await;
        let row = config(
            "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9",
            "main",
            dir.path().join("tenant").as_path(),
        );
        registry.create(row, "req-1").await.unwrap();
        let backup_dir = dir.path().join("backup");
        let info = registry.backup(&backup_dir).await.unwrap();
        assert_eq!(
            CatalogRegistry::inspect_backup(&backup_dir)
                .await
                .unwrap()
                .manifest,
            info.manifest
        );
        let restore_location = dir.path().join("restored");
        let restored = CatalogRegistry::open_location(
            restore_location.to_str().unwrap(),
            &RouterOpenOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(restored.restore(&backup_dir).await.unwrap().generation, 1);
        assert_eq!(restored.verify().await.unwrap().catalogs, 1);
    }

    #[tokio::test]
    async fn concurrent_mutations_use_generation_cas() {
        let (dir, registry) = registry().await;
        let left = config(
            "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9",
            "left",
            dir.path().join("left").as_path(),
        );
        let right = config(
            "018f4f4d-d520-7d91-b9f0-7018b7b50d13",
            "right",
            dir.path().join("right").as_path(),
        );
        let (left, right) = tokio::join!(
            registry.create(left, "left-request"),
            registry.create(right, "right-request")
        );
        assert!(left.is_ok());
        assert!(right.is_ok());
        assert_eq!(registry.status().await.unwrap().generation, 2);
    }

    #[test]
    fn rejects_registry_tenant_overlap() {
        let root = tempfile::tempdir().unwrap();
        let registry_location = CatalogLocation::parse(root.path().to_str().unwrap()).unwrap();
        let row = config("018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9", "main", root.path());
        let row = RegistryCatalog::from_config(row, CatalogLifecycle::Active).unwrap();
        let descriptor = row.descriptor().unwrap();
        assert!(
            descriptor.catalog.overlaps(&registry_location),
            "{} vs {}",
            descriptor.catalog.display_uri(),
            registry_location.display_uri()
        );
        let mut state = RegistryState::empty();
        state.catalogs.insert(row.id.clone(), row);
        rebuild_alias_index(&mut state).unwrap();
        assert!(validate_state(&state, Some(&registry_location)).is_err());
    }
}
