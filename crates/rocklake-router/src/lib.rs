//! Routing for independent RockLake catalogs.

mod authorization;
mod registry;

pub use authorization::*;
pub use registry::*;

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use object_store::path::Path as ObjectPath;
use rocklake_catalog::{CatalogStore, EncryptionConfig, OpenOptions};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{Mutex, Notify};

/// A stable opaque catalog identity. It is never derived from an alias or path.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CatalogId(String);

impl CatalogId {
    /// Validate and construct an ID from its canonical UUID text.
    pub fn new(value: impl Into<String>) -> Result<Self, RouterError> {
        let value = value.into();
        let parsed = uuid::Uuid::parse_str(&value)
            .map_err(|_| RouterError::InvalidConfig(format!("invalid catalog id '{value}'")))?;
        Ok(Self(parsed.to_string()))
    }

    /// Return the canonical UUID text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CatalogId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for CatalogId {
    type Err = RouterError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

/// A safe PostgreSQL database/catalog alias.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CatalogAlias(String);

impl CatalogAlias {
    /// Validate and construct an alias.
    pub fn new(value: impl Into<String>) -> Result<Self, RouterError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 63
            && value.as_bytes()[0].is_ascii_alphabetic()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
        if !valid {
            return Err(RouterError::InvalidConfig(format!(
                "invalid catalog alias '{value}'"
            )));
        }
        Ok(Self(value))
    }

    /// Return the alias text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CatalogAlias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for CatalogAlias {
    type Err = RouterError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

/// Whether a catalog accepts writes.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CatalogMode {
    #[default]
    ReadWrite,
    ReadOnly,
}

/// Per-catalog resource limits.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct CatalogLimits {
    /// Maximum sessions assigned to this catalog, when configured.
    pub max_sessions: Option<usize>,
    /// Maximum active scans assigned to this catalog, when configured.
    pub max_active_scans: Option<usize>,
    /// Maximum queued requests assigned to this catalog, when configured.
    pub max_queued_requests: Option<usize>,
    /// Maximum concurrent writer transactions, when configured.
    pub max_writer_transactions: Option<usize>,
    /// Maximum concurrent administrative jobs, when configured.
    pub max_admin_jobs: Option<usize>,
    /// Maximum open handles owned by this catalog, when configured.
    pub max_open_handles: Option<usize>,
    /// Maximum in-flight response bytes, report-only until atomic accounting exists.
    pub max_in_flight_response_bytes: Option<usize>,
}

/// A canonical storage location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogLocation {
    scheme: String,
    authority: String,
    prefix: String,
    local_path: Option<PathBuf>,
}

impl CatalogLocation {
    /// Parse and canonicalize a local or object-store URI.
    pub fn parse(input: &str) -> Result<Self, RouterError> {
        if input.trim().is_empty() {
            return Err(RouterError::InvalidConfig(
                "location must not be empty".into(),
            ));
        }
        let (scheme, rest) = match input.split_once("://") {
            Some((scheme, rest)) => (scheme.to_ascii_lowercase(), rest),
            None => ("file".to_string(), input),
        };
        let scheme = match scheme.as_str() {
            "file" | "s3" | "gs" | "az" => scheme,
            "gcs" => "gs".to_string(),
            "azure" | "abfs" | "abfss" => "az".to_string(),
            _ => {
                return Err(RouterError::InvalidConfig(format!(
                    "unsupported location scheme '{scheme}'"
                )))
            }
        };
        if rest.contains('@') {
            return Err(RouterError::InvalidConfig(
                "locations must not contain embedded credentials".into(),
            ));
        }
        if input.contains("://") && (rest.contains('?') || rest.contains('#')) {
            return Err(RouterError::InvalidConfig(
                "locations must not contain a query or fragment".into(),
            ));
        }
        if input.contains("://")
            && (rest.bytes().any(|byte| byte == b'\\')
                || rest.to_ascii_lowercase().contains("%2e")
                || rest.to_ascii_lowercase().contains("%2f")
                || rest.to_ascii_lowercase().contains("%5c"))
        {
            return Err(RouterError::InvalidConfig(format!(
                "location '{input}' contains traversal or platform-specific separators"
            )));
        }

        if scheme == "file" && !input.contains("://") {
            let path = canonicalize_local_path(input);
            let prefix = path.to_string_lossy().replace('\\', "/");
            return Self::from_parts(scheme, String::new(), &prefix, Some(path));
        }

        let (authority, path) = match rest.split_once('/') {
            Some((authority, path)) => (authority, path),
            None => (rest, ""),
        };
        let authority = authority.to_ascii_lowercase();
        if scheme != "file" && authority.is_empty() {
            return Err(RouterError::InvalidConfig(format!(
                "location '{input}' has no bucket or container"
            )));
        }
        if scheme == "file" && !authority.is_empty() && authority != "localhost" {
            return Err(RouterError::InvalidConfig(format!(
                "file location '{input}' has an unsupported authority"
            )));
        }
        let prefix = if scheme == "file" {
            canonicalize_local_path(&format!("/{path}"))
                .to_string_lossy()
                .replace('\\', "/")
        } else {
            path.to_string()
        };
        let is_file = scheme == "file";
        Self::from_parts(
            scheme,
            if authority == "localhost" {
                String::new()
            } else {
                authority
            },
            &prefix,
            is_file.then(|| PathBuf::from(&prefix)),
        )
    }

    fn from_parts(
        scheme: String,
        authority: String,
        prefix: &str,
        local_path: Option<PathBuf>,
    ) -> Result<Self, RouterError> {
        let segments: Vec<&str> = prefix
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        if segments
            .iter()
            .any(|segment| *segment == "." || *segment == "..")
        {
            return Err(RouterError::InvalidConfig(
                "location contains a traversal segment".into(),
            ));
        }
        let prefix = if scheme == "file" {
            format!("/{}", segments.join("/"))
        } else {
            segments.join("/")
        };
        Ok(Self {
            scheme,
            authority,
            prefix,
            local_path,
        })
    }

    /// Canonical scheme.
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    /// Canonical authority, such as a bucket or container.
    pub fn authority(&self) -> &str {
        &self.authority
    }

    /// Canonical path/prefix.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub(crate) fn local_path(&self) -> Option<&Path> {
        self.local_path.as_deref()
    }

    /// Canonical URI safe for diagnostics.
    pub fn display_uri(&self) -> String {
        if self.scheme == "file" {
            if let Some(path) = &self.local_path {
                let path = path.to_string_lossy();
                if path.starts_with(r"\\?\") {
                    return path.into_owned();
                }
            }
            format!("file://{}", self.prefix)
        } else if self.prefix.is_empty() {
            format!("{}://{}", self.scheme, self.authority)
        } else {
            format!("{}://{}/{}", self.scheme, self.authority, self.prefix)
        }
    }

    /// Whether this location is an ancestor, descendant, or exact match of another.
    pub fn overlaps(&self, other: &Self) -> bool {
        if self.scheme != other.scheme || self.authority != other.authority {
            return false;
        }
        prefix_overlaps(&self.prefix, &other.prefix)
    }

    pub(crate) fn object_store_path(
        &self,
        options: &RouterOpenOptions,
    ) -> Result<(ObjectPath, Arc<dyn object_store::ObjectStore>), RouterError> {
        match self.scheme.as_str() {
            "file" => {
                let path = self
                    .local_path
                    .clone()
                    .unwrap_or_else(|| PathBuf::from(&self.prefix));
                let store = object_store::local::LocalFileSystem::new_with_prefix(path)
                    .map_err(|error| RouterError::Open(error.to_string()))?;
                Ok((ObjectPath::from(""), Arc::new(store)))
            }
            "s3" => {
                let mut builder = object_store::aws::AmazonS3Builder::from_env()
                    .with_bucket_name(&self.authority);
                if let Some(endpoint) = &options.s3_endpoint {
                    builder = builder.with_endpoint(endpoint);
                }
                if options.s3_path_style {
                    builder = builder.with_virtual_hosted_style_request(false);
                }
                let store = builder
                    .build()
                    .map_err(|error| RouterError::Open(error.to_string()))?;
                Ok((ObjectPath::from(self.prefix.clone()), Arc::new(store)))
            }
            "gs" | "gcs" => {
                let store = object_store::gcp::GoogleCloudStorageBuilder::from_env()
                    .with_bucket_name(&self.authority)
                    .build()
                    .map_err(|error| RouterError::Open(error.to_string()))?;
                Ok((ObjectPath::from(self.prefix.clone()), Arc::new(store)))
            }
            "az" | "azure" | "abfs" | "abfss" => {
                let store = object_store::azure::MicrosoftAzureBuilder::from_env()
                    .with_container_name(&self.authority)
                    .build()
                    .map_err(|error| RouterError::Open(error.to_string()))?;
                Ok((ObjectPath::from(self.prefix.clone()), Arc::new(store)))
            }
            _ => unreachable!("validated location scheme"),
        }
    }
}

fn canonicalize_local_path(input: &str) -> PathBuf {
    let path = PathBuf::from(input);
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path))
            .unwrap_or_else(|_| PathBuf::from(input))
    };
    if let Ok(path) = std::fs::canonicalize(&absolute) {
        return path;
    }
    let mut missing = Vec::new();
    let mut existing = absolute.clone();
    while !existing.exists() {
        let Some(name) = existing.file_name().map(ToOwned::to_owned) else {
            return absolute;
        };
        missing.push(name);
        if !existing.pop() {
            return absolute;
        }
    }
    let mut canonical = std::fs::canonicalize(&existing).unwrap_or(existing);
    for name in missing.into_iter().rev() {
        canonical.push(name);
    }
    canonical
}

impl TryFrom<&str> for CatalogLocation {
    type Error = RouterError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

fn prefix_overlaps(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|rest| rest.starts_with('/'))
        || right
            .strip_prefix(left)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// A raw catalog entry as read from TOML.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogConfig {
    /// Stable UUID text.
    pub id: String,
    /// PostgreSQL database aliases.
    pub aliases: Vec<String>,
    /// SlateDB catalog location.
    pub catalog: String,
    /// Data-plane location, retained for isolation validation and diagnostics.
    pub data: String,
    /// Access mode.
    #[serde(default)]
    pub mode: CatalogMode,
    /// Named credential provider. Credentials in URLs are never accepted.
    pub credential_provider: String,
    /// Optional policy identifier; the policy contents are stored elsewhere.
    #[serde(default)]
    pub policy_reference: Option<String>,
    /// Optional per-catalog limits.
    #[serde(default)]
    pub limits: CatalogLimits,
}

/// A validated catalog descriptor.
#[derive(Clone, Debug)]
pub struct CatalogDescriptor {
    /// Stable ID.
    pub id: CatalogId,
    /// Validated aliases.
    pub aliases: Vec<CatalogAlias>,
    /// Canonical catalog location.
    pub catalog: CatalogLocation,
    /// Canonical data location.
    pub data: CatalogLocation,
    /// Access mode.
    pub mode: CatalogMode,
    /// Credential provider name.
    pub credential_provider: String,
    /// Optional policy identifier; raw policy material is never stored here.
    pub policy_reference: Option<String>,
    /// Per-catalog limits.
    pub limits: CatalogLimits,
}

impl TryFrom<CatalogConfig> for CatalogDescriptor {
    type Error = RouterError;

    fn try_from(config: CatalogConfig) -> Result<Self, Self::Error> {
        if config.credential_provider.trim().is_empty() {
            return Err(RouterError::InvalidConfig(
                "credential_provider must not be empty".into(),
            ));
        }
        let id = CatalogId::new(config.id)?;
        let aliases = config
            .aliases
            .into_iter()
            .map(CatalogAlias::new)
            .collect::<Result<Vec<_>, _>>()?;
        if aliases.is_empty() {
            return Err(RouterError::InvalidConfig(format!(
                "catalog {id} must define at least one alias"
            )));
        }
        authorization::validate_limits(&config.limits)?;
        let catalog = CatalogLocation::parse(&config.catalog)?;
        let data = CatalogLocation::parse(&config.data)?;
        Ok(Self {
            id,
            aliases,
            catalog,
            data,
            mode: config.mode,
            credential_provider: config.credential_provider,
            policy_reference: config.policy_reference,
            limits: config.limits,
        })
    }
}

/// Static router settings.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RouterSettings {
    /// Router mode (`static` or the managed `registry`).
    #[serde(default = "default_router_mode")]
    pub mode: String,
    /// Alias used when a client omits the database parameter.
    pub default_catalog: Option<String>,
    /// Maximum cached open catalog handles.
    #[serde(default = "default_max_open_catalogs")]
    pub max_open_catalogs: usize,
    /// Idle duration before an unused handle may be evicted.
    #[serde(default = "default_catalog_idle_timeout")]
    pub catalog_idle_timeout: u64,
}

fn default_router_mode() -> String {
    "static".into()
}

fn default_max_open_catalogs() -> usize {
    64
}

fn default_catalog_idle_timeout() -> u64 {
    300
}

impl Default for RouterSettings {
    fn default() -> Self {
        Self {
            mode: default_router_mode(),
            default_catalog: None,
            max_open_catalogs: default_max_open_catalogs(),
            catalog_idle_timeout: default_catalog_idle_timeout(),
        }
    }
}

/// A complete validated static router configuration.
#[derive(Clone, Debug)]
pub struct StaticConfig {
    /// Router settings.
    pub settings: RouterSettings,
    /// Validated descriptors.
    pub catalogs: Vec<CatalogDescriptor>,
    /// Registry generation represented by this immutable route snapshot.
    pub generation: u64,
}

impl StaticConfig {
    /// Validate raw TOML router settings and catalog entries.
    pub fn new(
        settings: RouterSettings,
        catalogs: Vec<CatalogConfig>,
    ) -> Result<Self, RouterError> {
        if settings.mode != "static" && settings.mode != "registry" {
            return Err(RouterError::InvalidConfig(
                "router.mode must be 'static' or 'registry'".into(),
            ));
        }
        if settings.max_open_catalogs == 0 || settings.catalog_idle_timeout == 0 {
            return Err(RouterError::InvalidConfig(
                "router cache limits must be greater than zero".into(),
            ));
        }
        if catalogs.is_empty() {
            return Err(RouterError::InvalidConfig(
                "at least one catalog is required".into(),
            ));
        }
        let catalogs = catalogs
            .into_iter()
            .map(CatalogDescriptor::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let mut ids = BTreeMap::new();
        let mut aliases = BTreeMap::new();
        let mut locations = Vec::new();
        for catalog in &catalogs {
            if ids.insert(catalog.id.clone(), ()).is_some() {
                return Err(RouterError::InvalidConfig(format!(
                    "duplicate catalog id {}",
                    catalog.id
                )));
            }
            for alias in &catalog.aliases {
                if aliases.insert(alias.clone(), catalog.id.clone()).is_some() {
                    return Err(RouterError::InvalidConfig(format!(
                        "duplicate catalog alias '{alias}'"
                    )));
                }
            }
            for location in [&catalog.catalog, &catalog.data] {
                if locations
                    .iter()
                    .any(|other: &CatalogLocation| location.overlaps(other))
                {
                    return Err(RouterError::InvalidConfig(format!(
                        "catalog location {} overlaps another configured location",
                        location.display_uri()
                    )));
                }
                locations.push(location.clone());
            }
        }
        let default_catalog = settings
            .default_catalog
            .clone()
            .unwrap_or_else(|| catalogs[0].aliases[0].as_str().to_string());
        let default_catalog = CatalogAlias::new(default_catalog)?;
        if !aliases.contains_key(&default_catalog) {
            return Err(RouterError::InvalidConfig(format!(
                "default catalog alias '{default_catalog}' is not configured"
            )));
        }
        Ok(Self {
            settings: RouterSettings {
                default_catalog: Some(default_catalog.to_string()),
                ..settings
            },
            catalogs,
            generation: 0,
        })
    }

    /// Build a validated route configuration from already validated descriptors.
    pub fn from_descriptors(
        settings: RouterSettings,
        descriptors: Vec<CatalogDescriptor>,
    ) -> Result<Self, RouterError> {
        Self::from_descriptors_at_generation(settings, descriptors, 0)
    }

    /// Build a validated route configuration at a specific registry generation.
    pub fn from_descriptors_at_generation(
        settings: RouterSettings,
        descriptors: Vec<CatalogDescriptor>,
        generation: u64,
    ) -> Result<Self, RouterError> {
        let catalogs = descriptors
            .into_iter()
            .map(|descriptor| CatalogConfig {
                id: descriptor.id.to_string(),
                aliases: descriptor.aliases.iter().map(ToString::to_string).collect(),
                catalog: descriptor.catalog.display_uri(),
                data: descriptor.data.display_uri(),
                mode: descriptor.mode,
                credential_provider: descriptor.credential_provider,
                policy_reference: descriptor.policy_reference,
                limits: descriptor.limits,
            })
            .collect();
        let mut config = Self::new(settings, catalogs)?;
        config.generation = generation;
        Ok(config)
    }
}

/// A resolved alias and its stable catalog identity.
#[derive(Clone, Debug)]
pub struct CatalogRoute {
    /// Stable catalog ID.
    pub id: CatalogId,
    /// Alias used for this connection.
    pub alias: CatalogAlias,
    /// Immutable descriptor snapshot.
    pub descriptor: CatalogDescriptor,
    /// Registry or route-table generation selected for this connection.
    pub generation: u64,
}

/// Evidence that a routed catalog has acquired the writer epoch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriterReadiness {
    /// Stable catalog identity.
    pub catalog_id: CatalogId,
    /// Route generation used for this readiness check.
    pub route_generation: u64,
    /// Durable catalog writer epoch acquired by the node.
    pub writer_epoch: u64,
}

impl WriterReadiness {
    /// Whether the writer epoch proves this node is write-ready.
    pub fn is_ready(&self) -> bool {
        self.writer_epoch > 0
    }
}

struct RouteTable {
    generation: u64,
    settings: RouterSettings,
    by_alias: BTreeMap<CatalogAlias, CatalogId>,
    by_id: BTreeMap<CatalogId, CatalogDescriptor>,
}

impl RouteTable {
    fn from_config(config: StaticConfig) -> Self {
        let generation = config.generation;
        let mut by_alias = BTreeMap::new();
        let mut by_id = BTreeMap::new();
        for catalog in config.catalogs {
            for alias in &catalog.aliases {
                by_alias.insert(alias.clone(), catalog.id.clone());
            }
            by_id.insert(catalog.id.clone(), catalog);
        }
        Self {
            generation,
            settings: config.settings,
            by_alias,
            by_id,
        }
    }

    fn resolve_alias(&self, alias: Option<&str>) -> Result<CatalogRoute, RouterError> {
        let alias = alias
            .filter(|value| !value.is_empty())
            .or(self.settings.default_catalog.as_deref())
            .ok_or_else(|| RouterError::UnknownCatalog("no default catalog configured".into()))?;
        let alias = CatalogAlias::new(alias)?;
        let id = self
            .by_alias
            .get(&alias)
            .ok_or_else(|| RouterError::UnknownCatalog(alias.to_string()))?;
        let descriptor = self
            .by_id
            .get(id)
            .cloned()
            .ok_or_else(|| RouterError::UnknownCatalog(alias.to_string()))?;
        Ok(CatalogRoute {
            id: id.clone(),
            alias,
            descriptor,
            generation: self.generation,
        })
    }

    fn resolve_id(&self, id: &CatalogId) -> Result<CatalogRoute, RouterError> {
        let descriptor = self
            .by_id
            .get(id)
            .cloned()
            .ok_or_else(|| RouterError::UnknownCatalog(id.to_string()))?;
        let alias = descriptor.aliases[0].clone();
        Ok(CatalogRoute {
            id: id.clone(),
            alias,
            descriptor,
            generation: self.generation,
        })
    }
}

struct CachedHandle {
    handle: Arc<Mutex<CatalogStore>>,
    last_used: Instant,
}

/// Options used to construct object stores for routed catalogs.
#[derive(Clone, Debug, Default)]
pub struct RouterOpenOptions {
    /// Optional S3-compatible endpoint.
    pub s3_endpoint: Option<String>,
    /// Use path-style S3 requests.
    pub s3_path_style: bool,
    /// Optional encryption key shared by configured catalogs.
    pub encryption: Option<EncryptionConfig>,
    /// Force every descriptor to open without a writer epoch.
    pub force_read_only: bool,
}

/// Errors raised while validating or opening routed catalogs.
#[derive(Debug, Error)]
pub enum RouterError {
    /// Invalid static configuration.
    #[error("invalid router configuration: {0}")]
    InvalidConfig(String),
    /// An alias or stable ID was not present in the current route table.
    #[error("catalog not found: {0}")]
    UnknownCatalog(String),
    /// Opening a catalog failed.
    #[error("catalog open failed: {0}")]
    Open(String),
    /// The configured handle cache cannot evict an active catalog.
    #[error("open catalog limit exhausted")]
    Capacity,
    /// The selected route is read-only.
    #[error("catalog is read-only")]
    ReadOnly,
    /// The route table changed after a connection pinned its route.
    #[error("stale route generation: expected {expected}, found {actual}")]
    StaleRouteGeneration { expected: u64, actual: u64 },
}

/// Runtime router with immutable snapshots and a bounded on-demand handle cache.
pub struct CatalogRouter {
    routes: std::sync::RwLock<Arc<RouteTable>>,
    handles: Mutex<HashMap<CatalogId, CachedHandle>>,
    opening: Mutex<HashMap<CatalogId, Arc<Notify>>>,
    options: RouterOpenOptions,
}

impl CatalogRouter {
    /// Build a router from validated static configuration.
    pub fn new(config: StaticConfig, options: RouterOpenOptions) -> Arc<Self> {
        Arc::new(Self {
            routes: std::sync::RwLock::new(Arc::new(RouteTable::from_config(config))),
            handles: Mutex::new(HashMap::new()),
            opening: Mutex::new(HashMap::new()),
            options,
        })
    }

    /// Resolve an alias without opening a catalog.
    pub fn resolve(&self, alias: Option<&str>) -> Result<CatalogRoute, RouterError> {
        self.routes
            .read()
            .expect("router route lock poisoned")
            .resolve_alias(alias)
    }

    /// Resolve a stable ID without opening a catalog.
    pub fn resolve_id(&self, id: &CatalogId) -> Result<CatalogRoute, RouterError> {
        self.routes
            .read()
            .expect("router route lock poisoned")
            .resolve_id(id)
    }

    /// Return the current route-table generation.
    pub fn generation(&self) -> u64 {
        self.routes
            .read()
            .expect("router route lock poisoned")
            .generation
    }

    /// Open a catalog only when its route generation is still current.
    pub async fn open_id_at_generation(
        &self,
        id: &CatalogId,
        generation: u64,
    ) -> Result<Arc<Mutex<CatalogStore>>, RouterError> {
        let route = self.resolve_id(id)?;
        if route.generation != generation {
            return Err(RouterError::StaleRouteGeneration {
                expected: generation,
                actual: route.generation,
            });
        }
        let handle = self.open_route(route).await?;
        let actual = self.generation();
        if actual != generation {
            return Err(RouterError::StaleRouteGeneration {
                expected: generation,
                actual,
            });
        }
        Ok(handle)
    }

    /// Open a routed catalog as a writer and require epoch acquisition.
    pub async fn open_writer(
        &self,
        id: &CatalogId,
    ) -> Result<Arc<Mutex<CatalogStore>>, RouterError> {
        let route = self.resolve_id(id)?;
        if route.descriptor.mode == CatalogMode::ReadOnly || self.options.force_read_only {
            return Err(RouterError::ReadOnly);
        }
        let handle = self.open_id_at_generation(id, route.generation).await?;
        if !handle.lock().await.is_writer() {
            return Err(RouterError::ReadOnly);
        }
        Ok(handle)
    }

    /// Open a writer and return the epoch-backed readiness evidence.
    pub async fn writer_readiness(&self, id: &CatalogId) -> Result<WriterReadiness, RouterError> {
        let route = self.resolve_id(id)?;
        if route.descriptor.mode == CatalogMode::ReadOnly || self.options.force_read_only {
            return Err(RouterError::ReadOnly);
        }
        let generation = route.generation;
        let handle = self.open_id_at_generation(id, generation).await?;
        let writer_epoch = handle.lock().await.writer_epoch();
        if writer_epoch == 0 {
            return Err(RouterError::ReadOnly);
        }
        Ok(WriterReadiness {
            catalog_id: id.clone(),
            route_generation: generation,
            writer_epoch,
        })
    }

    /// Return the configured mode for a stable catalog ID.
    pub fn mode_for_id(&self, id: &CatalogId) -> Option<CatalogMode> {
        self.routes
            .read()
            .expect("router route lock poisoned")
            .by_id
            .get(id)
            .map(|descriptor| descriptor.mode)
    }

    /// Return per-catalog limits for admission control.
    pub fn limits_for_id(&self, id: &CatalogId) -> Option<CatalogLimits> {
        self.routes
            .read()
            .expect("router route lock poisoned")
            .by_id
            .get(id)
            .map(|descriptor| descriptor.limits.clone())
    }

    /// Return a snapshot of all configured descriptors.
    pub fn descriptors(&self) -> Vec<CatalogDescriptor> {
        self.routes
            .read()
            .expect("router route lock poisoned")
            .by_id
            .values()
            .cloned()
            .collect()
    }

    /// Return the configured default alias.
    pub fn default_catalog(&self) -> String {
        self.routes
            .read()
            .expect("router route lock poisoned")
            .settings
            .default_catalog
            .clone()
            .expect("validated router has a default catalog")
    }

    /// Atomically replace the route table after validating a new configuration.
    pub fn reload(&self, config: StaticConfig) -> Result<(), RouterError> {
        let mut table = RouteTable::from_config(config);
        table.generation = self
            .routes
            .read()
            .expect("router route lock poisoned")
            .generation
            .saturating_add(1);
        let table = Arc::new(table);
        *self.routes.write().expect("router route lock poisoned") = table;
        Ok(())
    }

    /// Number of currently cached handles.
    pub async fn open_handle_count(&self) -> usize {
        self.handles.lock().await.len()
    }

    /// Install an already-open handle, used to seed the default server catalog.
    pub async fn install_handle(
        &self,
        id: CatalogId,
        handle: Arc<Mutex<CatalogStore>>,
    ) -> Result<(), RouterError> {
        self.resolve_id(&id)?;
        self.evict_idle().await;
        let mut handles = self.handles.lock().await;
        let max = self
            .routes
            .read()
            .expect("router route lock poisoned")
            .settings
            .max_open_catalogs;
        if handles.len() >= max && !handles.contains_key(&id) {
            return Err(RouterError::Capacity);
        }
        handles.insert(
            id,
            CachedHandle {
                handle,
                last_used: Instant::now(),
            },
        );
        Ok(())
    }

    /// Evict handles idle for the configured duration when they are not in use.
    pub async fn evict_idle(&self) {
        let timeout = Duration::from_secs(
            self.routes
                .read()
                .expect("router route lock poisoned")
                .settings
                .catalog_idle_timeout,
        );
        let now = Instant::now();
        let mut handles = self.handles.lock().await;
        let evicted: Vec<_> = handles
            .iter()
            .filter(|(_, cached)| {
                Arc::strong_count(&cached.handle) == 1
                    && now.duration_since(cached.last_used) >= timeout
            })
            .map(|(id, _)| id.clone())
            .collect();
        let evicted: Vec<_> = evicted
            .into_iter()
            .filter_map(|id| handles.remove(&id).map(|cached| cached.handle))
            .collect();
        drop(handles);
        for handle in evicted {
            if let Ok(mutex) = Arc::try_unwrap(handle) {
                let _ = mutex.into_inner().close().await;
            }
        }
    }

    /// Open the default catalog, primarily for server startup.
    pub async fn open_default(&self) -> Result<Arc<Mutex<CatalogStore>>, RouterError> {
        let alias = self.default_catalog();
        self.open_alias(Some(&alias)).await
    }

    /// Open the catalog selected by an alias, sharing concurrent first opens.
    pub async fn open_alias(
        &self,
        alias: Option<&str>,
    ) -> Result<Arc<Mutex<CatalogStore>>, RouterError> {
        let route = self.resolve(alias)?;
        self.open_route(route).await
    }

    /// Open the catalog selected by a stable ID, preserving session identity across alias reloads.
    pub async fn open_id(&self, id: &CatalogId) -> Result<Arc<Mutex<CatalogStore>>, RouterError> {
        let route = self.resolve_id(id)?;
        self.open_route(route).await
    }

    async fn open_route(
        &self,
        route: CatalogRoute,
    ) -> Result<Arc<Mutex<CatalogStore>>, RouterError> {
        loop {
            {
                let mut handles = self.handles.lock().await;
                if let Some(cached) = handles.get_mut(&route.id) {
                    cached.last_used = Instant::now();
                    return Ok(cached.handle.clone());
                }
            }
            let waiter = {
                let mut opening = self.opening.lock().await;
                if let Some(waiter) = opening.get(&route.id) {
                    Some(waiter.clone())
                } else {
                    opening.insert(route.id.clone(), Arc::new(Notify::new()));
                    None
                }
            };
            if let Some(waiter) = waiter {
                waiter.notified().await;
                continue;
            }

            let result = self.open_store(&route.descriptor).await;
            let waiter = self
                .opening
                .lock()
                .await
                .remove(&route.id)
                .expect("single-flight opener missing");
            if let Ok(handle) = &result {
                self.evict_idle().await;
                let mut handles = self.handles.lock().await;
                let max = self
                    .routes
                    .read()
                    .expect("router route lock poisoned")
                    .settings
                    .max_open_catalogs;
                if handles.len() >= max {
                    drop(handles);
                    waiter.notify_waiters();
                    return Err(RouterError::Capacity);
                }
                handles.insert(
                    route.id.clone(),
                    CachedHandle {
                        handle: handle.clone(),
                        last_used: Instant::now(),
                    },
                );
            }
            waiter.notify_waiters();
            return result;
        }
    }

    async fn open_store(
        &self,
        descriptor: &CatalogDescriptor,
    ) -> Result<Arc<Mutex<CatalogStore>>, RouterError> {
        let (path, object_store) = descriptor.catalog.object_store_path(&self.options)?;
        let opts = OpenOptions {
            object_store,
            path,
            encryption: self.options.encryption.clone(),
        };
        let store = if self.options.force_read_only || descriptor.mode == CatalogMode::ReadOnly {
            CatalogStore::open_without_epoch(opts)
                .await
                .map_err(|error| RouterError::Open(error.to_string()))?
        } else {
            CatalogStore::open(opts)
                .await
                .map_err(|error| RouterError::Open(error.to_string()))?
        };
        Ok(Arc::new(Mutex::new(store)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, alias: &str, catalog: &str, data: &str) -> CatalogConfig {
        CatalogConfig {
            id: id.into(),
            aliases: vec![alias.into()],
            catalog: catalog.into(),
            data: data.into(),
            mode: CatalogMode::ReadWrite,
            credential_provider: "env".into(),
            policy_reference: None,
            limits: CatalogLimits::default(),
        }
    }

    #[test]
    fn rejects_overlapping_prefixes_and_credentials() {
        let settings = RouterSettings::default();
        let error = StaticConfig::new(
            settings.clone(),
            vec![
                entry(
                    "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9",
                    "a",
                    "s3://bucket/a",
                    "s3://data/a",
                ),
                entry(
                    "018f4f4d-d520-7d91-b9f0-7018b7b50d13",
                    "b",
                    "s3://bucket/a/b",
                    "s3://data/b",
                ),
            ],
        )
        .unwrap_err();
        assert!(error.to_string().contains("overlaps"));
        assert!(CatalogLocation::parse("s3://user:pass@bucket/a").is_err());
    }

    #[test]
    fn accepts_local_paths_with_uri_delimiters() {
        assert!(CatalogLocation::parse(r"\\?\C:\tmp\rocklake").is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn local_display_path_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let location = CatalogLocation::parse(dir.path().to_str().unwrap()).unwrap();
        let reparsed = CatalogLocation::parse(&location.display_uri()).unwrap();
        assert_eq!(reparsed.local_path(), location.local_path());
    }

    #[test]
    fn route_aliases_are_not_storage_keys() {
        let config = StaticConfig::new(
            RouterSettings {
                default_catalog: Some("main".into()),
                ..RouterSettings::default()
            },
            vec![entry(
                "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9",
                "main",
                "file:///tmp/rocklake-router-catalog",
                "file:///tmp/rocklake-router-data",
            )],
        )
        .unwrap();
        let router = CatalogRouter::new(config, RouterOpenOptions::default());
        let route = router.resolve(Some("main")).unwrap();
        assert_eq!(route.id.as_str(), "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9");
        assert_eq!(route.alias.as_str(), "main");
    }

    #[tokio::test]
    async fn concurrent_first_open_is_single_flight() {
        let catalog_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();
        let catalog_path = catalog_dir.path().to_string_lossy().replace('\\', "/");
        let data_path = data_dir.path().to_string_lossy().replace('\\', "/");
        let config = StaticConfig::new(
            RouterSettings {
                max_open_catalogs: 1,
                ..RouterSettings::default()
            },
            vec![entry(
                "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9",
                "main",
                &catalog_path,
                &data_path,
            )],
        )
        .unwrap();
        let router = CatalogRouter::new(config, RouterOpenOptions::default());
        let (left, right) = tokio::join!(
            router.open_alias(Some("main")),
            router.open_alias(Some("main"))
        );
        assert!(left.is_ok());
        assert!(right.is_ok());
        assert_eq!(router.open_handle_count().await, 1);
    }

    #[tokio::test]
    async fn writer_readiness_requires_an_epoch_and_generation() {
        let catalog_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();
        let id = "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9";
        let config = StaticConfig::new(
            RouterSettings::default(),
            vec![entry(
                id,
                "main",
                catalog_dir.path().to_str().unwrap(),
                data_dir.path().to_str().unwrap(),
            )],
        )
        .unwrap();
        let router = CatalogRouter::new(config.clone(), RouterOpenOptions::default());
        let id: CatalogId = id.parse().unwrap();
        let readiness = router.writer_readiness(&id).await.unwrap();
        assert!(readiness.is_ready());
        assert_eq!(readiness.route_generation, 0);

        router.reload(config).unwrap();
        assert!(matches!(
            router.open_id_at_generation(&id, 0).await,
            Err(RouterError::StaleRouteGeneration { .. })
        ));
    }

    #[tokio::test]
    async fn read_only_routes_never_report_writer_readiness() {
        let config = StaticConfig::new(
            RouterSettings::default(),
            vec![CatalogConfig {
                id: "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9".into(),
                aliases: vec!["main".into()],
                catalog: "/tmp/rocklake-readonly-router-catalog".into(),
                data: "/tmp/rocklake-readonly-router-data".into(),
                mode: CatalogMode::ReadOnly,
                credential_provider: "env".into(),
                policy_reference: None,
                limits: CatalogLimits::default(),
            }],
        )
        .unwrap();
        let router = CatalogRouter::new(config, RouterOpenOptions::default());
        let id: CatalogId = "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9".parse().unwrap();
        assert!(matches!(
            router.writer_readiness(&id).await,
            Err(RouterError::ReadOnly)
        ));
    }
}
