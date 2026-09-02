//! RockLake Client: idiomatic async Rust API for the RockLake catalog.
//!
//! `rocklake-client` wraps `rocklake-catalog`'s `CatalogStore` with an
//! ergonomic, async-first interface that is independent of both DuckDB and
//! the C ABI.  It is the recommended entry point for:
//!
//! - Rust microservices
//! - DataFusion integrations
//! - The sync blocking wrapper used by C extension threads and Python
//!
//! # Quick start
//!
//! ```no_run
//! # tokio::runtime::Runtime::new().expect("runtime").block_on(async {
//! use rocklake_client::{CatalogClient, CatalogClientBuilder, SnapshotRef};
//!
//! let client = CatalogClientBuilder::new("file:///tmp/my-catalog")
//!     .build()
//!     .await
//!     .expect("build");
//!
//! let schemas = client
//!     .list_schemas(SnapshotRef::Latest)
//!     .await
//!     .expect("list_schemas");
//! println!("schemas: {schemas:?}");
//!
//! client.close().await.expect("close");
//! # });
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures::stream::BoxStream;
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use thiserror::Error;

use rocklake_catalog::{CatalogError, CatalogStore, OpenOptions};
pub use rocklake_core::mvcc::SnapshotId;

// Re-export so call sites can match on the new structured variants.
pub use rocklake_catalog::error::{is_transient, with_transient_retry};

// ─── Error type ────────────────────────────────────────────────────────────

/// Errors returned by `rocklake-client`.
#[derive(Debug, Error)]
pub enum ClientError {
    /// Underlying catalog operation failed.
    #[error("catalog error: {0}")]
    Catalog(#[from] CatalogError),

    /// Bad configuration (e.g., unparseable URI).
    #[error("configuration error: {0}")]
    Config(String),
}

/// Shorthand result type.
pub type ClientResult<T> = Result<T, ClientError>;

/// Select the latest committed snapshot or an exact snapshot ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotRef {
    /// The latest committed snapshot when the operation starts.
    Latest,
    /// An exact snapshot ID.
    At(SnapshotId),
}

impl From<u64> for SnapshotRef {
    fn from(snapshot_id: u64) -> Self {
        Self::At(SnapshotId::new(snapshot_id))
    }
}

impl From<SnapshotId> for SnapshotRef {
    fn from(snapshot_id: SnapshotId) -> Self {
        Self::At(snapshot_id)
    }
}

// ─── Schema / Table / DataFile value types ─────────────────────────────────

/// A catalog schema returned by [`CatalogClient::list_schemas`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    /// Opaque schema identifier.
    pub schema_id: u64,
    /// Human-readable schema name.
    pub schema_name: String,
}

/// A catalog table returned by [`CatalogClient::list_tables`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// Opaque table identifier.
    pub table_id: u64,
    /// Schema this table belongs to.
    pub schema_id: u64,
    /// Human-readable table name.
    pub table_name: String,
}

/// A data file returned by [`CatalogClient::list_data_files`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFile {
    /// Opaque data file identifier.
    pub data_file_id: u64,
    /// Table this file belongs to.
    pub table_id: u64,
    /// Object-store path or URL.
    pub path: String,
    /// File format (`"parquet"`, `"csv"`, …).
    pub file_format: String,
    /// Number of rows in the file.
    pub row_count: u64,
    /// Total size in bytes.
    pub file_size_bytes: u64,
    /// Snapshot at which this file was first visible.
    pub snapshot_id: u64,
}

/// A bounded page of data files and an opaque continuation token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFilePage {
    pub files: Vec<DataFile>,
    pub continuation_token: Option<String>,
}

/// A column descriptor returned by [`CatalogClient::get_table`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// Opaque column identifier.
    pub column_id: u64,
    /// Table this column belongs to.
    pub table_id: u64,
    /// Column name.
    pub column_name: String,
    /// SQL data type string.
    pub data_type: String,
    /// Zero-based ordinal position.
    pub column_index: u64,
    /// Whether `NULL` values are permitted.
    pub is_nullable: bool,
}

// ─── CatalogClientBuilder ──────────────────────────────────────────────────

/// Builder for [`CatalogClient`].
///
/// # Examples
///
/// ```no_run
/// # tokio::runtime::Runtime::new().expect("runtime").block_on(async {
/// use rocklake_client::CatalogClientBuilder;
///
/// let client = CatalogClientBuilder::new("file:///tmp/demo")
///     .build()
///     .await
///     .expect("build");
/// client.close().await.expect("close");
/// # });
/// ```
pub struct CatalogClientBuilder {
    uri: String,
}

impl CatalogClientBuilder {
    /// Create a new builder with the given catalog URI.
    ///
    /// Supported URI schemes:
    /// - `file:///path/to/catalog` — local filesystem
    /// - Raw local path (no scheme) — also treated as local filesystem
    pub fn new(uri: impl Into<String>) -> Self {
        Self { uri: uri.into() }
    }

    /// Build and open a [`CatalogClient`].
    ///
    /// Connects to the catalog storage backend and returns a ready-to-use
    /// client.  Fails if the URI scheme is unsupported or if the underlying
    /// store cannot be opened.
    pub async fn build(self) -> ClientResult<CatalogClient> {
        let path = catalog_path(&self.uri)?;
        let object_store = build_object_store(&self.uri)?;

        let opts = OpenOptions {
            object_store,
            path,
            encryption: None,
        };

        let store = CatalogStore::open(opts).await?;
        Ok(CatalogClient {
            store: Arc::new(tokio::sync::RwLock::new(Some(store))),
        })
    }

    /// Build and open a **read-only** [`ReadOnlyClient`].
    ///
    /// Unlike [`build()`](Self::build), this does **not** acquire or increment
    /// the writer epoch.  Many simultaneous `build_readonly()` calls against the
    /// same catalog path produce zero CAS transaction conflicts.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # tokio::runtime::Runtime::new().expect("runtime").block_on(async {
    /// use rocklake_client::CatalogClientBuilder;
    ///
    /// let client = CatalogClientBuilder::new("file:///tmp/demo")
    ///     .build_readonly()
    ///     .await
    ///     .expect("build_readonly");
    /// let snapshot_id = client.current_snapshot_id();
    /// client.close().await.expect("close");
    /// # });
    /// ```
    pub async fn build_readonly(self) -> ClientResult<ReadOnlyClient> {
        let path = catalog_path(&self.uri)?;
        let object_store = build_object_store(&self.uri)?;

        let opts = OpenOptions {
            object_store,
            path,
            encryption: None,
        };

        let cat = rocklake_catalog::ReadOnlyCatalog::open(opts).await?;
        let current_snapshot_id = cat.current_snapshot_id_atomic();
        Ok(ReadOnlyClient {
            inner: tokio::sync::Mutex::new(cat),
            current_snapshot_id,
        })
    }
}

// ─── Object-store URI resolution ───────────────────────────────────────────

/// Build an `ObjectStore` from a URI string, supporting:
/// - `file:///path` or raw local path → `LocalFileSystem`
/// - `s3://bucket/prefix` → `AmazonS3Builder` (reads `AWS_*` env vars)
/// - `gs://bucket/prefix` → `GoogleCloudStorageBuilder` (reads `GOOGLE_APPLICATION_CREDENTIALS`)
/// - `az://container/prefix` or `abfs://container/prefix` → `MicrosoftAzureBuilder` (reads `AZURE_*`)
fn build_object_store(uri: &str) -> ClientResult<Arc<dyn object_store::ObjectStore>> {
    if let Some(without_scheme) = uri.strip_prefix("file://") {
        let store = LocalFileSystem::new_with_prefix(without_scheme).map_err(|e| {
            ClientError::Config(format!("cannot open local store at {without_scheme}: {e}"))
        })?;
        return Ok(Arc::new(store));
    }

    if let Some(without_scheme) = uri.strip_prefix("s3://") {
        let (bucket, _prefix) = split_bucket_prefix(without_scheme);
        let store = object_store::aws::AmazonS3Builder::from_env()
            .with_bucket_name(bucket)
            .build()
            .map_err(|e| ClientError::Config(format!("cannot open S3 store for {uri}: {e}")))?;
        return Ok(Arc::new(store));
    }

    if let Some(without_scheme) = uri.strip_prefix("gs://") {
        let (bucket, _prefix) = split_bucket_prefix(without_scheme);
        let store = object_store::gcp::GoogleCloudStorageBuilder::from_env()
            .with_bucket_name(bucket)
            .build()
            .map_err(|e| ClientError::Config(format!("cannot open GCS store for {uri}: {e}")))?;
        return Ok(Arc::new(store));
    }

    if let Some(without_scheme) = uri
        .strip_prefix("az://")
        .or_else(|| uri.strip_prefix("azure://"))
        .or_else(|| uri.strip_prefix("abfs://"))
        .or_else(|| uri.strip_prefix("abfss://"))
    {
        let (container, _prefix) = split_bucket_prefix(without_scheme);
        let store = object_store::azure::MicrosoftAzureBuilder::from_env()
            .with_container_name(container)
            .build()
            .map_err(|e| ClientError::Config(format!("cannot open Azure store for {uri}: {e}")))?;
        return Ok(Arc::new(store));
    }

    // Unknown scheme or raw path → treat as local filesystem
    if uri.contains("://") {
        return Err(ClientError::Config(format!(
            "unsupported URI scheme in '{uri}': supported schemes are file://, s3://, gs://, az://, azure://, abfs://, abfss://"
        )));
    }

    let store = LocalFileSystem::new_with_prefix(uri)
        .map_err(|e| ClientError::Config(format!("cannot open local store at {uri}: {e}")))?;
    Ok(Arc::new(store))
}

fn split_bucket_prefix(without_scheme: &str) -> (&str, &str) {
    match without_scheme.find('/') {
        Some(idx) => (&without_scheme[..idx], &without_scheme[idx + 1..]),
        None => (without_scheme, ""),
    }
}

fn catalog_path(uri: &str) -> ClientResult<ObjectPath> {
    let prefix = ["s3://", "gs://", "az://", "azure://", "abfs://", "abfss://"]
        .iter()
        .find_map(|scheme| {
            uri.strip_prefix(scheme)
                .map(split_bucket_prefix)
                .map(|(_, p)| p)
        });
    match prefix {
        Some(prefix) => {
            let prefix = prefix.trim_matches('/');
            rocklake_core::path::validate_object_prefix(prefix)
                .map_err(|e| ClientError::Config(e.to_string()))?;
            Ok(ObjectPath::from(if prefix.is_empty() {
                "catalog".to_string()
            } else {
                format!("{prefix}/catalog")
            }))
        }
        None if uri.contains("://") && !uri.starts_with("file://") => Err(ClientError::Config(
            format!("unsupported URI scheme in '{uri}'"),
        )),
        None => Ok(ObjectPath::from("catalog")),
    }
}

// ─── CatalogClient ─────────────────────────────────────────────────────────

/// Async-first client for RockLake catalog operations.
///
/// All methods are `async` and cancel-safe.  To use from synchronous code
/// (e.g., C extension threads) use [`CatalogClientSync`].
///
/// # Thread safety
///
/// `CatalogClient` is `Send + Sync` and may be shared across tasks via
/// `Arc<CatalogClient>`.  Internally the `CatalogStore` is protected by a
/// `tokio::sync::RwLock` — concurrent read operations (e.g., `list_schemas`,
/// `list_tables`, `list_data_files`) acquire a read lock and do **not**
/// serialise against each other.  Only `close()` acquires a write lock.
pub struct CatalogClient {
    store: Arc<tokio::sync::RwLock<Option<CatalogStore>>>,
}

impl std::fmt::Debug for CatalogClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CatalogClient").finish_non_exhaustive()
    }
}

impl CatalogClient {
    /// Acquire a read guard, returning an error if the catalog has been closed.
    async fn read(&self) -> ClientResult<tokio::sync::RwLockReadGuard<'_, Option<CatalogStore>>> {
        let guard = self.store.read().await;
        if guard.is_none() {
            return Err(ClientError::Config("catalog has been closed".to_owned()));
        }
        Ok(guard)
    }

    fn reader(
        store: &CatalogStore,
        snapshot: SnapshotRef,
    ) -> rocklake_catalog::CatalogResult<rocklake_catalog::CatalogReader> {
        match snapshot {
            SnapshotRef::Latest => Ok(store.read_latest()),
            SnapshotRef::At(snapshot_id) => store.read_at(snapshot_id),
        }
    }

    /// Return the current (latest committed) snapshot ID.
    ///
    /// Returns `0` when the catalog has no snapshots yet.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # tokio::runtime::Runtime::new().expect("runtime").block_on(async {
    /// use rocklake_client::{CatalogClientBuilder, SnapshotRef};
    /// let client = CatalogClientBuilder::new("file:///tmp/demo").build().await.expect("build");
    /// let snap = client.snapshot_id().await.expect("snapshot_id");
    /// assert_eq!(snap, 0); // fresh catalog
    /// client.close().await.expect("close");
    /// # });
    /// ```
    pub async fn snapshot_id(&self) -> ClientResult<u64> {
        let reader = {
            let guard = self.read().await?;
            let store = guard.as_ref().expect("checked by read()");
            store.read_latest()
        };
        let snap = reader.get_snapshot().await?;
        Ok(snap.map(|s| s.snapshot_id).unwrap_or(0))
    }

    /// List all schemas visible at the selected snapshot.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # tokio::runtime::Runtime::new().expect("runtime").block_on(async {
    /// use rocklake_client::{CatalogClientBuilder, SnapshotRef};
    /// let client = CatalogClientBuilder::new("file:///tmp/demo").build().await.expect("build");
    /// let schemas = client.list_schemas(SnapshotRef::Latest).await.expect("list_schemas");
    /// assert!(schemas.is_empty());
    /// client.close().await.expect("close");
    /// # });
    /// ```
    pub async fn list_schemas(
        &self,
        snapshot: impl Into<SnapshotRef>,
    ) -> ClientResult<Vec<Schema>> {
        let reader = {
            let guard = self.read().await?;
            let store = guard.as_ref().expect("checked by read()");
            Self::reader(store, snapshot.into())?
        };
        let rows = reader.list_schemas().await?;
        Ok(rows
            .into_iter()
            .map(|r| Schema {
                schema_id: r.schema_id,
                schema_name: r.schema_name,
            })
            .collect())
    }

    /// List all tables in `schema_id` visible at the selected snapshot.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # tokio::runtime::Runtime::new().expect("runtime").block_on(async {
    /// use rocklake_client::{CatalogClientBuilder, SnapshotRef};
    /// let client = CatalogClientBuilder::new("file:///tmp/demo").build().await.expect("build");
    /// let tables = client.list_tables(1, SnapshotRef::Latest).await.expect("list_tables");
    /// assert!(tables.is_empty());
    /// client.close().await.expect("close");
    /// # });
    /// ```
    pub async fn list_tables(
        &self,
        schema_id: u64,
        snapshot: impl Into<SnapshotRef>,
    ) -> ClientResult<Vec<Table>> {
        let reader = {
            let guard = self.read().await?;
            let store = guard.as_ref().expect("checked by read()");
            Self::reader(store, snapshot.into())?
        };
        let rows = reader.list_tables(schema_id).await?;
        Ok(rows
            .into_iter()
            .map(|r| Table {
                table_id: r.table_id,
                schema_id: r.schema_id,
                table_name: r.table_name,
            })
            .collect())
    }

    /// Describe the columns of `table_id` at the selected snapshot.
    ///
    /// Returns `None` when no table with that ID exists at the given snapshot.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # tokio::runtime::Runtime::new().expect("runtime").block_on(async {
    /// use rocklake_client::{CatalogClientBuilder, SnapshotRef};
    /// let client = CatalogClientBuilder::new("file:///tmp/demo").build().await.expect("build");
    /// let cols = client.get_table(999, SnapshotRef::Latest).await.expect("get_table");
    /// assert!(cols.is_none());
    /// client.close().await.expect("close");
    /// # });
    /// ```
    pub async fn get_table(
        &self,
        table_id: u64,
        snapshot: impl Into<SnapshotRef>,
    ) -> ClientResult<Option<Vec<Column>>> {
        let reader = {
            let guard = self.read().await?;
            let store = guard.as_ref().expect("checked by read()");
            Self::reader(store, snapshot.into())?
        };
        let result = reader.describe_table(table_id).await?;
        Ok(result.map(|(_table, cols)| {
            cols.into_iter()
                .map(|c| Column {
                    column_id: c.column_id,
                    table_id: c.table_id,
                    column_name: c.column_name,
                    data_type: c.data_type,
                    column_index: c.column_index,
                    is_nullable: c.is_nullable,
                })
                .collect()
        }))
    }

    /// List data files for `table_id` visible at the selected snapshot.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # tokio::runtime::Runtime::new().expect("runtime").block_on(async {
    /// use rocklake_client::{CatalogClientBuilder, SnapshotRef};
    /// let client = CatalogClientBuilder::new("file:///tmp/demo").build().await.expect("build");
    /// let files = client
    ///     .list_data_files(1, SnapshotRef::Latest)
    ///     .await
    ///     .expect("list_data_files");
    /// assert!(files.is_empty());
    /// client.close().await.expect("close");
    /// # });
    /// ```
    pub async fn list_data_files(
        &self,
        table_id: u64,
        snapshot: impl Into<SnapshotRef>,
    ) -> ClientResult<Vec<DataFile>> {
        let reader = {
            let guard = self.read().await?;
            let store = guard.as_ref().expect("checked by read()");
            Self::reader(store, snapshot.into())?
        };
        let rows = reader.list_data_files(table_id).await?;
        Ok(rows.into_iter().map(DataFile::from_row).collect())
    }

    /// List one bounded page of data files at the selected snapshot.
    pub async fn list_data_files_paged(
        &self,
        table_id: u64,
        snapshot: impl Into<SnapshotRef>,
        page_size: usize,
        continuation_token: Option<&str>,
    ) -> ClientResult<DataFilePage> {
        let reader = {
            let guard = self.read().await?;
            let store = guard.as_ref().expect("checked by read()");
            Self::reader(store, snapshot.into())?
        };
        let page = reader
            .list_data_files_paged(table_id, page_size, continuation_token)
            .await?;
        Ok(DataFilePage {
            files: page.files.into_iter().map(DataFile::from_row).collect(),
            continuation_token: page.continuation_token,
        })
    }

    /// Stream data files incrementally at the selected snapshot.
    pub async fn stream_data_files(
        &self,
        table_id: u64,
        snapshot: impl Into<SnapshotRef>,
    ) -> ClientResult<BoxStream<'static, ClientResult<DataFile>>> {
        let reader = {
            let guard = self.read().await?;
            let store = guard.as_ref().expect("checked by read()");
            Self::reader(store, snapshot.into())?
        };
        let stream = reader.stream_data_files(table_id).await?;
        Ok(Box::pin(futures::StreamExt::map(stream, |row| {
            row.map(DataFile::from_row).map_err(ClientError::from)
        })))
    }

    /// Close the catalog and release all resources.
    ///
    /// After this call all methods return an error.  It is not an error to
    /// call `close()` more than once.
    pub async fn close(self) -> ClientResult<()> {
        let mut guard = self.store.write().await;
        if let Some(store) = guard.take() {
            store.close().await?;
        }
        Ok(())
    }
}

impl DataFile {
    fn from_row(f: rocklake_core::rows::DataFileRow) -> Self {
        Self {
            data_file_id: f.data_file_id,
            table_id: f.table_id,
            path: f.path,
            file_format: f.file_format,
            row_count: f.record_count,
            file_size_bytes: f.file_size_bytes,
            snapshot_id: f.begin_snapshot.unwrap_or(0),
        }
    }
}

// ─── ReadOnlyClient ────────────────────────────────────────────────────────

/// Read-only client for the RockLake catalog.
///
/// Opened via [`CatalogClientBuilder::build_readonly()`].  Does **not**
/// acquire a writer epoch — many instances may be opened concurrently
/// against the same catalog prefix with zero coordination overhead.
///
/// # Examples
///
/// ```no_run
/// # tokio::runtime::Runtime::new().expect("runtime").block_on(async {
/// use rocklake_client::CatalogClientBuilder;
///
/// let mut client = CatalogClientBuilder::new("file:///tmp/demo")
///     .build_readonly()
///     .await
///     .expect("build_readonly");
/// let snapshot_id = client.current_snapshot_id();
/// let new_snap = client.refresh().await.expect("refresh");
/// client.close().await.expect("close");
/// # });
/// ```
pub struct ReadOnlyClient {
    inner: tokio::sync::Mutex<rocklake_catalog::ReadOnlyCatalog>,
    current_snapshot_id: Arc<AtomicU64>,
}

impl std::fmt::Debug for ReadOnlyClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadOnlyClient").finish_non_exhaustive()
    }
}

impl ReadOnlyClient {
    fn resolve_reader(
        cat: &rocklake_catalog::ReadOnlyCatalog,
        snapshot: SnapshotRef,
    ) -> rocklake_catalog::CatalogResult<rocklake_catalog::CatalogReader> {
        match snapshot {
            SnapshotRef::Latest => cat.reader(),
            SnapshotRef::At(snapshot_id) => cat.read_at(snapshot_id),
        }
    }

    /// Return the snapshot ID observed at the last `refresh()` (or `open()`).
    pub fn current_snapshot_id(&self) -> Option<SnapshotId> {
        Some(SnapshotId::new(
            self.current_snapshot_id.load(Ordering::Acquire),
        ))
    }

    /// Return the latest committed snapshot ID.
    pub async fn snapshot_id(&self) -> ClientResult<u64> {
        Ok(self.current_snapshot_id.load(Ordering::Acquire))
    }

    /// Advance to the latest committed snapshot without writer coordination.
    ///
    /// Re-reads the snapshot counter and the GC retain-from key from SlateDB.
    /// Returns the newly observed snapshot ID.
    pub async fn refresh(&self) -> ClientResult<SnapshotId> {
        let mut guard = self.inner.lock().await;
        let snap = guard.refresh().await?;
        self.current_snapshot_id
            .store(snap.as_u64(), Ordering::Release);
        Ok(snap)
    }

    /// List schemas visible at the selected snapshot.
    pub async fn list_schemas(
        &self,
        snapshot: impl Into<SnapshotRef>,
    ) -> ClientResult<Vec<Schema>> {
        let reader = {
            let guard = self.inner.lock().await;
            Self::resolve_reader(&guard, snapshot.into())?
        };
        let rows = reader.list_schemas().await?;
        Ok(rows
            .into_iter()
            .map(|r| Schema {
                schema_id: r.schema_id,
                schema_name: r.schema_name,
            })
            .collect())
    }

    /// List tables in a schema, visible at the selected snapshot.
    pub async fn list_tables(
        &self,
        schema_id: u64,
        snapshot: impl Into<SnapshotRef>,
    ) -> ClientResult<Vec<Table>> {
        let reader = {
            let guard = self.inner.lock().await;
            Self::resolve_reader(&guard, snapshot.into())?
        };
        let rows = reader.list_tables(schema_id).await?;
        Ok(rows
            .into_iter()
            .map(|r| Table {
                table_id: r.table_id,
                schema_id: r.schema_id,
                table_name: r.table_name,
            })
            .collect())
    }

    /// Describe the columns of `table_id` at the selected snapshot.
    ///
    /// Returns `None` when no table with that ID exists at the given snapshot.
    pub async fn get_table(
        &self,
        table_id: u64,
        snapshot: impl Into<SnapshotRef>,
    ) -> ClientResult<Option<Vec<Column>>> {
        let reader = {
            let guard = self.inner.lock().await;
            Self::resolve_reader(&guard, snapshot.into())?
        };
        let result = reader.describe_table(table_id).await?;
        Ok(result.map(|(_table, cols)| {
            cols.into_iter()
                .map(|c| Column {
                    column_id: c.column_id,
                    table_id: c.table_id,
                    column_name: c.column_name,
                    data_type: c.data_type,
                    column_index: c.column_index,
                    is_nullable: c.is_nullable,
                })
                .collect()
        }))
    }

    /// List data files for `table_id` visible at the selected snapshot.
    pub async fn list_data_files(
        &self,
        table_id: u64,
        snapshot: impl Into<SnapshotRef>,
    ) -> ClientResult<Vec<DataFile>> {
        let reader = {
            let guard = self.inner.lock().await;
            Self::resolve_reader(&guard, snapshot.into())?
        };
        let rows = reader.list_data_files(table_id).await?;
        Ok(rows.into_iter().map(DataFile::from_row).collect())
    }

    /// List one bounded page of data files at the selected snapshot.
    pub async fn list_data_files_paged(
        &self,
        table_id: u64,
        snapshot: impl Into<SnapshotRef>,
        page_size: usize,
        continuation_token: Option<&str>,
    ) -> ClientResult<DataFilePage> {
        let reader = {
            let guard = self.inner.lock().await;
            Self::resolve_reader(&guard, snapshot.into())?
        };
        let page = reader
            .list_data_files_paged(table_id, page_size, continuation_token)
            .await?;
        Ok(DataFilePage {
            files: page.files.into_iter().map(DataFile::from_row).collect(),
            continuation_token: page.continuation_token,
        })
    }

    /// Stream data files incrementally at the selected snapshot.
    pub async fn stream_data_files(
        &self,
        table_id: u64,
        snapshot: impl Into<SnapshotRef>,
    ) -> ClientResult<BoxStream<'static, ClientResult<DataFile>>> {
        let reader = {
            let guard = self.inner.lock().await;
            Self::resolve_reader(&guard, snapshot.into())?
        };
        let stream = reader.stream_data_files(table_id).await?;
        Ok(Box::pin(futures::StreamExt::map(stream, |row| {
            row.map(DataFile::from_row).map_err(ClientError::from)
        })))
    }

    /// Close the catalog and release all resources.
    pub async fn close(self) -> ClientResult<()> {
        let guard = self.inner.into_inner();
        guard.close().await.map_err(ClientError::from)
    }
}

/// Unified asynchronous read model implemented by writer-backed and read-only clients.
#[async_trait::async_trait]
pub trait CatalogReaderOps {
    /// Return the latest committed snapshot ID.
    async fn snapshot_id(&self) -> ClientResult<u64>;

    /// List schemas visible at the selected snapshot.
    async fn list_schemas(&self, snapshot: SnapshotRef) -> ClientResult<Vec<Schema>>;

    /// List tables in `schema_id` visible at the selected snapshot.
    async fn list_tables(&self, schema_id: u64, snapshot: SnapshotRef) -> ClientResult<Vec<Table>>;

    /// Describe the columns of `table_id` at the selected snapshot.
    async fn get_table(
        &self,
        table_id: u64,
        snapshot: SnapshotRef,
    ) -> ClientResult<Option<Vec<Column>>>;

    /// List data files for `table_id` visible at the selected snapshot.
    async fn list_data_files(
        &self,
        table_id: u64,
        snapshot: SnapshotRef,
    ) -> ClientResult<Vec<DataFile>>;

    /// List one bounded page of data files at the selected snapshot.
    async fn list_data_files_paged(
        &self,
        table_id: u64,
        snapshot: SnapshotRef,
        page_size: usize,
        continuation_token: Option<&str>,
    ) -> ClientResult<DataFilePage>;

    /// Stream data files incrementally at the selected snapshot.
    async fn stream_data_files(
        &self,
        table_id: u64,
        snapshot: SnapshotRef,
    ) -> ClientResult<BoxStream<'static, ClientResult<DataFile>>>;
}

#[async_trait::async_trait]
impl CatalogReaderOps for CatalogClient {
    async fn snapshot_id(&self) -> ClientResult<u64> {
        self.snapshot_id().await
    }

    async fn list_schemas(&self, snapshot: SnapshotRef) -> ClientResult<Vec<Schema>> {
        self.list_schemas(snapshot).await
    }

    async fn list_tables(&self, schema_id: u64, snapshot: SnapshotRef) -> ClientResult<Vec<Table>> {
        self.list_tables(schema_id, snapshot).await
    }

    async fn get_table(
        &self,
        table_id: u64,
        snapshot: SnapshotRef,
    ) -> ClientResult<Option<Vec<Column>>> {
        self.get_table(table_id, snapshot).await
    }

    async fn list_data_files(
        &self,
        table_id: u64,
        snapshot: SnapshotRef,
    ) -> ClientResult<Vec<DataFile>> {
        self.list_data_files(table_id, snapshot).await
    }

    async fn list_data_files_paged(
        &self,
        table_id: u64,
        snapshot: SnapshotRef,
        page_size: usize,
        continuation_token: Option<&str>,
    ) -> ClientResult<DataFilePage> {
        self.list_data_files_paged(table_id, snapshot, page_size, continuation_token)
            .await
    }

    async fn stream_data_files(
        &self,
        table_id: u64,
        snapshot: SnapshotRef,
    ) -> ClientResult<BoxStream<'static, ClientResult<DataFile>>> {
        self.stream_data_files(table_id, snapshot).await
    }
}

#[async_trait::async_trait]
impl CatalogReaderOps for ReadOnlyClient {
    async fn snapshot_id(&self) -> ClientResult<u64> {
        self.snapshot_id().await
    }

    async fn list_schemas(&self, snapshot: SnapshotRef) -> ClientResult<Vec<Schema>> {
        self.list_schemas(snapshot).await
    }

    async fn list_tables(&self, schema_id: u64, snapshot: SnapshotRef) -> ClientResult<Vec<Table>> {
        self.list_tables(schema_id, snapshot).await
    }

    async fn get_table(
        &self,
        table_id: u64,
        snapshot: SnapshotRef,
    ) -> ClientResult<Option<Vec<Column>>> {
        self.get_table(table_id, snapshot).await
    }

    async fn list_data_files(
        &self,
        table_id: u64,
        snapshot: SnapshotRef,
    ) -> ClientResult<Vec<DataFile>> {
        self.list_data_files(table_id, snapshot).await
    }

    async fn list_data_files_paged(
        &self,
        table_id: u64,
        snapshot: SnapshotRef,
        page_size: usize,
        continuation_token: Option<&str>,
    ) -> ClientResult<DataFilePage> {
        self.list_data_files_paged(table_id, snapshot, page_size, continuation_token)
            .await
    }

    async fn stream_data_files(
        &self,
        table_id: u64,
        snapshot: SnapshotRef,
    ) -> ClientResult<BoxStream<'static, ClientResult<DataFile>>> {
        self.stream_data_files(table_id, snapshot).await
    }
}

// ─── CatalogClientSync ─────────────────────────────────────────────────────

/// Synchronous (blocking) wrapper around [`CatalogClient`].
///
/// Use this from contexts that cannot use `async` Rust:
/// - C extension threads
/// - Python GIL-holding code (before releasing the GIL)
/// - FFI callers outside a Tokio runtime
///
/// # Examples
///
/// ```no_run
/// use rocklake_client::CatalogClientSync;
///
/// let client = CatalogClientSync::open("file:///tmp/demo").expect("open");
/// let snap = client.snapshot_id().expect("snapshot_id");
/// assert_eq!(snap, 0);
/// client.close().expect("close");
/// ```
pub struct CatalogClientSync {
    runtime: tokio::runtime::Runtime,
    inner: CatalogClient,
}

impl CatalogClientSync {
    /// Open a catalog synchronously.
    pub fn open(uri: impl Into<String>) -> ClientResult<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| ClientError::Config(format!("failed to create Tokio runtime: {e}")))?;
        let inner = runtime.block_on(CatalogClientBuilder::new(uri).build())?;
        Ok(Self { runtime, inner })
    }

    /// Open a catalog in read-only mode (no writer epoch acquired).
    ///
    /// Use this for stateless reader replicas and analytics sidecars. Multiple
    /// simultaneous calls produce zero CAS write conflicts.
    pub fn open_readonly(uri: impl Into<String>) -> ClientResult<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| ClientError::Config(format!("failed to create Tokio runtime: {e}")))?;
        // build_readonly returns a ReadOnlyClient; wrap it in a CatalogClient
        // via the underlying open_without_epoch path so we keep the same sync API.
        let uri_str = uri.into();
        let path = catalog_path(&uri_str)?;
        let os = build_object_store(&uri_str)?;
        let opts = OpenOptions {
            object_store: os,
            path,
            encryption: None,
        };
        let object_store =
            runtime.block_on(rocklake_catalog::CatalogStore::open_without_epoch(opts))?;
        let inner = CatalogClient {
            store: std::sync::Arc::new(tokio::sync::RwLock::new(Some(object_store))),
        };
        Ok(Self { runtime, inner })
    }

    /// Return the current snapshot ID.
    pub fn snapshot_id(&self) -> ClientResult<u64> {
        self.runtime.block_on(self.inner.snapshot_id())
    }

    /// List schemas at the selected snapshot.
    pub fn list_schemas(&self, snapshot: impl Into<SnapshotRef>) -> ClientResult<Vec<Schema>> {
        self.runtime.block_on(self.inner.list_schemas(snapshot))
    }

    /// List tables in `schema_id` at the selected snapshot.
    pub fn list_tables(
        &self,
        schema_id: u64,
        snapshot: impl Into<SnapshotRef>,
    ) -> ClientResult<Vec<Table>> {
        self.runtime
            .block_on(self.inner.list_tables(schema_id, snapshot))
    }

    /// Describe columns of `table_id` at the selected snapshot.
    pub fn get_table(
        &self,
        table_id: u64,
        snapshot: impl Into<SnapshotRef>,
    ) -> ClientResult<Option<Vec<Column>>> {
        self.runtime
            .block_on(self.inner.get_table(table_id, snapshot))
    }

    /// List data files for `table_id` at the selected snapshot.
    pub fn list_data_files(
        &self,
        table_id: u64,
        snapshot: impl Into<SnapshotRef>,
    ) -> ClientResult<Vec<DataFile>> {
        self.runtime
            .block_on(self.inner.list_data_files(table_id, snapshot))
    }

    /// List one bounded page of data files at the selected snapshot.
    pub fn list_data_files_paged(
        &self,
        table_id: u64,
        snapshot: impl Into<SnapshotRef>,
        page_size: usize,
        continuation_token: Option<&str>,
    ) -> ClientResult<DataFilePage> {
        self.runtime.block_on(self.inner.list_data_files_paged(
            table_id,
            snapshot,
            page_size,
            continuation_token,
        ))
    }

    /// Close the catalog.
    pub fn close(self) -> ClientResult<()> {
        self.runtime.block_on(self.inner.close())
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn builder_open_close() {
        let dir = tempfile::TempDir::new().unwrap();
        let uri = format!("file://{}", dir.path().to_str().unwrap());
        let client = CatalogClientBuilder::new(&uri).build().await.unwrap();
        let snap = client.snapshot_id().await.unwrap();
        assert_eq!(snap, 0, "fresh catalog has no snapshots");
        client.close().await.unwrap();
    }

    #[tokio::test]
    async fn list_schemas_empty() {
        let dir = tempfile::TempDir::new().unwrap();
        let uri = format!("file://{}", dir.path().to_str().unwrap());
        let client = CatalogClientBuilder::new(uri).build().await.unwrap();
        let schemas = client.list_schemas(SnapshotRef::Latest).await.unwrap();
        assert!(schemas.is_empty(), "fresh catalog has no schemas");
        client.close().await.unwrap();
    }

    #[tokio::test]
    async fn list_tables_empty() {
        let dir = tempfile::TempDir::new().unwrap();
        let uri = format!("file://{}", dir.path().to_str().unwrap());
        let client = CatalogClientBuilder::new(uri).build().await.unwrap();
        let tables = client.list_tables(1, SnapshotRef::Latest).await.unwrap();
        assert!(tables.is_empty(), "fresh catalog has no tables");
        client.close().await.unwrap();
    }

    #[tokio::test]
    async fn get_table_not_found() {
        let dir = tempfile::TempDir::new().unwrap();
        let uri = format!("file://{}", dir.path().to_str().unwrap());
        let client = CatalogClientBuilder::new(uri).build().await.unwrap();
        let cols = client.get_table(999, SnapshotRef::Latest).await.unwrap();
        assert!(cols.is_none(), "non-existent table returns None");
        client.close().await.unwrap();
    }

    #[tokio::test]
    async fn list_data_files_empty() {
        let dir = tempfile::TempDir::new().unwrap();
        let uri = format!("file://{}", dir.path().to_str().unwrap());
        let client = CatalogClientBuilder::new(uri).build().await.unwrap();
        let files = client
            .list_data_files(1, SnapshotRef::Latest)
            .await
            .unwrap();
        assert!(files.is_empty(), "fresh catalog has no data files");
        client.close().await.unwrap();
    }

    #[test]
    fn sync_client_open_close() {
        let dir = tempfile::TempDir::new().unwrap();
        let uri = format!("file://{}", dir.path().to_str().unwrap());
        let client = CatalogClientSync::open(&uri).unwrap();
        assert_eq!(client.snapshot_id().unwrap(), 0);
        client.close().unwrap();
    }

    #[test]
    fn sync_client_list_schemas_empty() {
        let dir = tempfile::TempDir::new().unwrap();
        let uri = format!("file://{}", dir.path().to_str().unwrap());
        let client = CatalogClientSync::open(uri).unwrap();
        let schemas = client
            .list_schemas(SnapshotRef::At(SnapshotId::new(0)))
            .unwrap();
        assert!(schemas.is_empty());
        client.close().unwrap();
    }

    #[test]
    fn sync_raw_path_without_file_scheme() {
        let dir = tempfile::TempDir::new().unwrap();
        let client = CatalogClientSync::open(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(client.snapshot_id().unwrap(), 0);
        client.close().unwrap();
    }

    // ─── Multi-URI builder scheme tests ────────────────────────────────────

    #[test]
    fn builder_file_scheme_opens_local() {
        let dir = tempfile::TempDir::new().unwrap();
        let uri = format!("file://{}", dir.path().to_str().unwrap());
        let client = CatalogClientSync::open(&uri).unwrap();
        assert_eq!(client.snapshot_id().unwrap(), 0);
        client.close().unwrap();
    }

    #[test]
    fn builder_unknown_scheme_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let err = rt
            .block_on(CatalogClientBuilder::new("ftp://example.com/catalog").build())
            .unwrap_err();
        assert!(
            matches!(err, ClientError::Config(_)),
            "unknown scheme should return Config error, got: {err}"
        );
    }

    #[test]
    fn cloud_catalog_path_preserves_nested_prefix() {
        assert_eq!(
            catalog_path("s3://bucket/tenant/warehouse").unwrap(),
            ObjectPath::from("tenant/warehouse/catalog")
        );
        assert_eq!(
            catalog_path("abfss://container/tenant/warehouse").unwrap(),
            ObjectPath::from("tenant/warehouse/catalog")
        );
        assert!(catalog_path("s3://bucket/tenant/../outside").is_err());
    }

    #[tokio::test]
    async fn concurrent_readers_do_not_serialize() {
        // v0.46.0: verify that 8 concurrent list_schemas calls all complete
        // without deadlocking (RwLock allows concurrent reads).
        let dir = tempfile::TempDir::new().unwrap();
        let uri = format!("file://{}", dir.path().to_str().unwrap());
        let client = std::sync::Arc::new(CatalogClientBuilder::new(uri).build().await.unwrap());

        let mut tasks = Vec::new();
        for _ in 0..8 {
            let c = client.clone();
            tasks.push(tokio::spawn(async move {
                c.list_schemas(SnapshotRef::Latest).await.unwrap()
            }));
        }

        for task in tasks {
            let schemas = task.await.unwrap();
            assert!(schemas.is_empty());
        }

        // Unwrap the Arc to close.
        std::sync::Arc::try_unwrap(client)
            .expect("single owner after tasks complete")
            .close()
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn readonly_client_shares_read_model() {
        let dir = tempfile::TempDir::new().unwrap();
        let uri = format!("file://{}", dir.path().to_str().unwrap());

        // First initialize with writer client
        let writer = CatalogClientBuilder::new(&uri).build().await.unwrap();
        writer.close().await.unwrap();

        // Open with readonly client
        let reader = CatalogClientBuilder::new(&uri)
            .build_readonly()
            .await
            .unwrap();
        assert_eq!(reader.snapshot_id().await.unwrap(), 0);
        assert_eq!(reader.current_snapshot_id().unwrap().as_u64(), 0);

        let schemas = reader.list_schemas(SnapshotRef::Latest).await.unwrap();
        assert!(schemas.is_empty());

        let tables = reader.list_tables(1, SnapshotRef::Latest).await.unwrap();
        assert!(tables.is_empty());

        let table = reader.get_table(1, SnapshotRef::Latest).await.unwrap();
        assert!(table.is_none());

        let files = reader
            .list_data_files(1, SnapshotRef::Latest)
            .await
            .unwrap();
        assert!(files.is_empty());

        let paged = reader
            .list_data_files_paged(1, SnapshotRef::Latest, 10, None)
            .await
            .unwrap();
        assert!(paged.files.is_empty());

        // Trait usage
        async fn read_via_trait<T: CatalogReaderOps>(client: &T) {
            let _ = client.list_schemas(SnapshotRef::Latest).await.unwrap();
        }
        read_via_trait(&reader).await;

        reader.close().await.unwrap();
    }
}
