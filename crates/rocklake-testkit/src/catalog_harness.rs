//! CatalogHarness: lightweight catalog write/read helper for integration tests.
//!
//! The harness keeps the `CatalogStore` behind a shared mutex so tests can
//! hand out owned writers and readers without cloning setup code or reopening
//! the backing object store repeatedly.

use std::sync::Arc;

use object_store::path::Path as ObjectPath;
use tokio::sync::Mutex;

use rocklake_catalog::{CatalogError, CatalogStore, CommitResult, OpenOptions};
use rocklake_core::mvcc::SnapshotId;
use rocklake_core::rows::InlinedInsertRow;

#[cfg(feature = "minio-tests")]
use crate::MinioHarness;

/// Lightweight catalog harness for Tier 2+ integration tests.
pub struct CatalogHarness {
    pub store: Arc<Mutex<CatalogStore>>,
    _dir: Option<tempfile::TempDir>,
    opts: OpenOptions,
}

impl CatalogHarness {
    /// Create a harness backed by the local filesystem.
    pub async fn local() -> Result<Self, CatalogError> {
        let dir = tempfile::TempDir::new()
            .map_err(|e| CatalogError::Internal(format!("tempdir failed: {e}")))?;
        let object_store: Arc<dyn object_store::ObjectStore> = Arc::new(
            object_store::local::LocalFileSystem::new_with_prefix(dir.path())
                .map_err(|e| CatalogError::Internal(format!("local fs init failed: {e}")))?,
        );
        Self::with_object_store(object_store, "test-catalog", Some(dir)).await
    }

    /// Backwards-compatible alias for callers that still use the old name.
    pub async fn in_memory() -> Result<Self, CatalogError> {
        Self::local().await
    }

    /// Create a harness backed by a specific object store.
    pub async fn with_object_store(
        object_store: Arc<dyn object_store::ObjectStore>,
        path: &str,
        dir: Option<tempfile::TempDir>,
    ) -> Result<Self, CatalogError> {
        let opts = OpenOptions {
            object_store,
            path: ObjectPath::from(path),
            encryption: None,
        };
        let store = CatalogStore::open(opts.clone()).await?;
        Ok(Self {
            store: Arc::new(Mutex::new(store)),
            _dir: dir,
            opts,
        })
    }

    /// Create a harness backed by a MinIO test container.
    #[cfg(feature = "minio-tests")]
    pub async fn on_minio(harness: &MinioHarness, prefix: &str) -> Result<Self, CatalogError> {
        Self::with_object_store(harness.object_store(), prefix, None).await
    }

    /// Reopen the catalog (simulates process restart).
    pub async fn reopen(&self) -> Result<(), CatalogError> {
        let store = CatalogStore::open(self.opts.clone()).await?;
        *self.store.lock().await = store;
        Ok(())
    }

    /// Begin a write session and return the owned writer.
    pub async fn writer(&self) -> rocklake_catalog::CatalogWriter {
        self.store.lock().await.begin_write()
    }

    /// Commit a writer result back into the catalog store.
    pub async fn commit_writer(&self, result: CommitResult) {
        self.store.lock().await.commit_writer(result);
    }

    /// Read the latest snapshot.
    pub async fn reader_latest(&self) -> rocklake_catalog::CatalogReader {
        self.store.lock().await.read_latest()
    }

    /// Read at a specific snapshot.
    pub async fn reader_at(
        &self,
        snapshot: SnapshotId,
    ) -> Result<rocklake_catalog::CatalogReader, CatalogError> {
        self.store.lock().await.read_at(snapshot)
    }

    /// Read back all inline inserts for a given table.
    pub async fn read_inlined_inserts(
        &self,
        table_id: u64,
    ) -> Result<Vec<InlinedInsertRow>, CatalogError> {
        let reader = self.reader_latest().await;
        reader.list_inlined_inserts(table_id).await
    }

    /// Assert the catalog can be reopened without error (durability check).
    pub async fn assert_durable(&self) -> Result<(), CatalogError> {
        let _reopened = CatalogStore::open(self.opts.clone()).await?;
        Ok(())
    }
}
