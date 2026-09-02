//! Read-only catalog access path (RFC-01, v0.47.0).
//!
//! `ReadOnlyCatalog` opens SlateDB **without** acquiring or incrementing the
//! writer-epoch CAS key.  This means many reader instances can open the same
//! catalog concurrently with zero coordination overhead — ideal for stateless,
//! horizontally-scaled reader fleets.
//!
//! # Guarantees
//!
//! * No RockLake catalog metadata writes on open or refresh.
//! * `reader()` creates a `CatalogReader` bound to the most recently refreshed
//!   snapshot; it never sees data past a snapshot that was committed *after*
//!   the last `refresh()` call (snapshot isolation).
//! * Concurrent `ReadOnlyCatalog` instances opened against the same catalog
//!   prefix produce **zero** CAS transaction conflicts in the SlateDB write log.
//!
//! # Example
//!
//! ```no_run
//! # tokio::runtime::Runtime::new().expect("runtime").block_on(async {
//! use std::sync::Arc;
//! use object_store::local::LocalFileSystem;
//! use object_store::path::Path as ObjectPath;
//! use rocklake_catalog::{OpenOptions, ReadOnlyCatalog};
//!
//! let dir = tempfile::tempdir().expect("tempdir");
//! let store = Arc::new(LocalFileSystem::new_with_prefix(dir.path()).expect("store"));
//! let mut cat = ReadOnlyCatalog::open(OpenOptions {
//!     object_store: store,
//!     path: ObjectPath::from(""),
//!     encryption: None,
//! }).await.expect("open");
//! let snapshot_id = cat.refresh().await.expect("refresh");
//! let reader = cat.reader();
//! # });
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use slatedb::Db;

use rocklake_core::keys;
use rocklake_core::mvcc::SnapshotId;
use rocklake_core::tags::COUNTER_NEXT_SNAPSHOT_ID;
use rocklake_core::values;

use crate::encryption::AesGcmTransformer;
use crate::error::{CatalogError, CatalogResult};
use crate::reader::CatalogReader;
use crate::store::OpenOptions;

/// A read-only catalog handle.
///
/// Does **not** hold a writer epoch — multiple instances may be opened against
/// the same S3/GCS prefix without contention.
pub struct ReadOnlyCatalog {
    db: Db,
    /// Snapshot ID of the latest committed snapshot at the time of the last
    /// `refresh()` call (or `open()`).
    current_snapshot_id: Arc<AtomicU64>,
    /// Cached retain-from floor (read from SlateDB on open / refresh).
    retain_from: Arc<AtomicU64>,
    /// Object store held for callers (e.g. data-file reads).
    object_store: Arc<dyn object_store::ObjectStore>,
}

impl ReadOnlyCatalog {
    /// Open a read-only catalog.  No writer epoch is acquired or incremented.
    ///
    /// The catalog must already be initialized by a writer.
    pub async fn open(opts: OpenOptions) -> CatalogResult<Self> {
        let object_store_ref = Arc::clone(&opts.object_store);

        let db = if let Some(ref enc) = opts.encryption {
            let transformer = Arc::new(AesGcmTransformer::new(enc));
            Db::builder(opts.path, opts.object_store)
                .with_block_transformer(transformer)
                .build()
                .await?
        } else {
            Db::open(opts.path, opts.object_store).await?
        };

        crate::init::verify_format_version(&db).await?;
        crate::init::verify_migrations_complete(&db).await?;
        crate::init::load_counters_from_db(&db).await?;

        let current_snapshot_id = Self::read_latest_snapshot_id(&db).await?;

        // Read the retain-from floor.
        let retain_from_initial = Self::read_retain_from(&db).await?;

        Ok(Self {
            db,
            current_snapshot_id: Arc::new(AtomicU64::new(current_snapshot_id.as_u64())),
            retain_from: Arc::new(AtomicU64::new(retain_from_initial)),
            object_store: object_store_ref,
        })
    }

    /// Return a reader bound to the snapshot captured at the last `refresh()`.
    ///
    /// Returns `CatalogError::SnapshotOutOfRetention` if the current snapshot
    /// has been GC-retired.
    pub fn reader(&self) -> CatalogResult<CatalogReader> {
        self.read_at(self.current_snapshot_id())
    }

    /// Return a reader bound to a specific snapshot ID.
    ///
    /// Returns `CatalogError::SnapshotOutOfRetention` if `dl_snapshot_id`
    /// falls below the current retain-from floor, or `CatalogError::SnapshotNotFound`
    /// if `dl_snapshot_id` exceeds the latest observed committed snapshot.
    pub fn read_at(&self, dl_snapshot_id: impl Into<SnapshotId>) -> CatalogResult<CatalogReader> {
        let dl_snapshot_id = dl_snapshot_id.into();
        let retain_from = self.retain_from.load(Ordering::Acquire);
        if retain_from > 0 && dl_snapshot_id.as_u64() < retain_from {
            return Err(CatalogError::SnapshotOutOfRetention {
                requested: dl_snapshot_id.as_u64(),
                retain_from,
            });
        }
        let latest = self.current_snapshot_id().as_u64();
        if dl_snapshot_id.as_u64() > latest {
            return Err(CatalogError::SnapshotNotFound {
                requested: dl_snapshot_id.as_u64(),
                latest_committed: latest,
            });
        }
        Ok(CatalogReader::new(self.db.clone(), dl_snapshot_id))
    }

    /// Advance to the latest committed snapshot without writer coordination.
    ///
    /// Re-reads the `next_snapshot_id` counter and the `retain_from` key from
    /// SlateDB.  Returns the newly observed snapshot ID.
    pub async fn refresh(&mut self) -> CatalogResult<SnapshotId> {
        crate::init::verify_format_version(&self.db).await?;
        crate::init::verify_migrations_complete(&self.db).await?;
        crate::init::load_counters_from_db(&self.db).await?;

        // Refresh snapshot ID.
        let current_snapshot_id = Self::read_latest_snapshot_id(&self.db).await?;

        // Refresh retain-from floor.
        let retain_from = Self::read_retain_from(&self.db).await?;
        self.current_snapshot_id
            .store(current_snapshot_id.as_u64(), Ordering::Release);
        self.retain_from.store(retain_from, Ordering::Release);

        Ok(current_snapshot_id)
    }

    /// Return the snapshot ID observed at the last `refresh()` (or `open()`).
    pub fn current_snapshot_id(&self) -> SnapshotId {
        SnapshotId::new(self.current_snapshot_id.load(Ordering::Acquire))
    }

    /// Return an atomic handle to the current snapshot ID.
    pub fn current_snapshot_id_atomic(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.current_snapshot_id)
    }

    /// Return the object store backing this catalog.
    pub fn object_store(&self) -> Arc<dyn object_store::ObjectStore> {
        Arc::clone(&self.object_store)
    }

    /// Close the underlying SlateDB handle.
    pub async fn close(self) -> CatalogResult<()> {
        crate::fault_injection::trigger(
            crate::fault_injection::WriteFaultPoint::BeforeCatalogClose,
        )
        .await?;
        self.db.close().await?;
        Ok(())
    }

    /// Creates a `ReadOnlyCatalog` from an existing [`slatedb::Db`] handle.
    ///
    /// **Test-only.** In tests it is sometimes necessary to share a single
    /// `Db` handle between a [`CatalogStore`] writer and a `ReadOnlyCatalog`
    /// reader to avoid the SlateDB WAL-based fencing that occurs when two
    /// independent `Db::open()` calls target the same object-store path.
    ///
    /// The caller must invoke [`refresh()`](Self::refresh) at least once
    /// after construction to populate `current_snapshot_id`.
    ///
    /// This method is intentionally `pub` (not `pub(crate)`) so that
    /// integration tests in `tests/` can access it; it is hidden from
    /// generated documentation and must not be used in production code.
    #[doc(hidden)]
    pub fn from_db_for_test(db: Db, object_store: Arc<dyn object_store::ObjectStore>) -> Self {
        Self {
            db,
            current_snapshot_id: Arc::new(AtomicU64::new(0)),
            retain_from: Arc::new(AtomicU64::new(0)),
            object_store,
        }
    }

    // ── internal ───────────────────────────────────────────────────────────

    async fn read_latest_snapshot_id(db: &Db) -> CatalogResult<SnapshotId> {
        let key = keys::key_counter(COUNTER_NEXT_SNAPSHOT_ID);
        let data = db.get(&key).await?.ok_or(CatalogError::NotInitialized)?;
        let next = values::decode_counter(&data)?;
        if next == 0 {
            return Err(CatalogError::Corruption(
                "next snapshot counter must be greater than zero".to_string(),
            ));
        }
        // next_snapshot_id is always 1 ahead of the committed snapshot.
        Ok(SnapshotId::new(next - 1))
    }

    async fn read_retain_from(db: &Db) -> CatalogResult<u64> {
        let key = keys::key_system(rocklake_core::tags::SYSTEM_RETAIN_FROM);
        let data = db.get(&key).await?.ok_or(CatalogError::NotInitialized)?;
        Ok(values::decode_counter(&data)?)
    }
}
