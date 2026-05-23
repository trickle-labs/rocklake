//! CatalogStore — top-level catalog handle.

use object_store::path::Path as ObjectPath;
use slatedb::{Db, IsolationLevel};
use slateduck_core::encoding::{decode_u32, encode_counter, encode_u32, encode_value};
use slateduck_core::error::{Result, SlateDuckError};
use slateduck_core::keys;
use slateduck_core::mvcc::SnapshotId;
use slateduck_core::tags::*;
use std::sync::Arc;

use crate::reader::CatalogReader;
use crate::writer::CatalogWriter;

/// Options for opening a CatalogStore.
#[derive(Debug, Clone)]
pub struct OpenOptions {
    /// Object store root path for the catalog.
    pub path: String,
    /// Object store implementation.
    pub object_store: Arc<dyn object_store::ObjectStore>,
    /// Default retention days for GC (0 = retain forever).
    pub retention_days: u32,
}

/// Top-level catalog store backed by SlateDB.
pub struct CatalogStore {
    db: Db,
    #[allow(dead_code)]
    retention_days: u32,
}

impl CatalogStore {
    /// Open or create a catalog store.
    ///
    /// Uses `DbTransaction` with `SerializableSnapshot` isolation to ensure
    /// safe concurrent initialization: two processes opening simultaneously
    /// converge on exactly one coherent initial metadata set.
    pub async fn open(opts: OpenOptions) -> Result<Self> {
        let path = ObjectPath::from(opts.path.as_str());
        let db = Db::builder(path, opts.object_store)
            .build()
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        // Check/perform initialization
        Self::ensure_initialized(&db).await?;

        Ok(Self {
            db,
            retention_days: opts.retention_days,
        })
    }

    /// Ensure the catalog is initialized. Uses a serializable transaction
    /// to implement insert-if-absent for the format version key.
    async fn ensure_initialized(db: &Db) -> Result<()> {
        let format_key = keys::system_key(SYSTEM_CATALOG_FORMAT_VERSION);

        let txn = db
            .begin(IsolationLevel::SerializableSnapshot)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        let existing = txn
            .get(&format_key)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        match existing {
            Some(data) => {
                // Already initialized — verify format version
                let version = decode_u32(&data)?;
                if version != CATALOG_FORMAT_VERSION {
                    return Err(SlateDuckError::FormatVersionMismatch {
                        expected: CATALOG_FORMAT_VERSION,
                        actual: version,
                    });
                }
                // Transaction not needed — drop without commit
                Ok(())
            }
            None => {
                // First time — initialize all counters and metadata
                let format_value = encode_u32(CATALOG_FORMAT_VERSION);
                txn.put(&format_key, format_value)
                    .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

                // Initialize counters
                let snapshot_counter_key = keys::counter_key(COUNTER_NEXT_SNAPSHOT_ID);
                txn.put(&snapshot_counter_key, encode_counter(1))
                    .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

                let catalog_counter_key = keys::counter_key(COUNTER_NEXT_CATALOG_ID);
                txn.put(&catalog_counter_key, encode_counter(1))
                    .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

                let file_counter_key = keys::counter_key(COUNTER_NEXT_FILE_ID);
                txn.put(&file_counter_key, encode_counter(1))
                    .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

                // Initialize retain-from to 0 (retain everything)
                let retain_key = keys::system_key(SYSTEM_RETAIN_FROM);
                txn.put(&retain_key, encode_counter(0))
                    .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

                // Initialize writer epoch
                let epoch_key = keys::system_key(SYSTEM_WRITER_EPOCH);
                txn.put(&epoch_key, encode_counter(1))
                    .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

                // Set default metadata
                let data_path_key = keys::metadata_key(0x00, 0, "data_path");
                txn.put(&data_path_key, encode_value(b""))
                    .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

                txn.commit()
                    .await
                    .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

                db.flush()
                    .await
                    .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

                Ok(())
            }
        }
    }

    /// Get a reader at the specified DuckLake snapshot ID.
    pub async fn read_at(&self, dl_snapshot_id: SnapshotId) -> CatalogReader<'_> {
        CatalogReader::new(&self.db, dl_snapshot_id)
    }

    /// Begin a write transaction.
    pub async fn begin_write(&self) -> Result<CatalogWriter<'_>> {
        CatalogWriter::new(&self.db).await
    }

    /// Get the current (latest) snapshot ID.
    pub async fn current_snapshot_id(&self) -> Result<SnapshotId> {
        let key = keys::counter_key(COUNTER_NEXT_SNAPSHOT_ID);
        let data = self
            .db
            .get(&key)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?
            .ok_or(SlateDuckError::CatalogNotInitialized)?;
        let next_id = slateduck_core::encoding::decode_counter(&data)?;
        // Current snapshot is next_id - 1, but if next_id is 1, no snapshots yet
        Ok(next_id.saturating_sub(1))
    }

    /// Get the oldest retained snapshot ID for GC.
    pub async fn retain_from(&self) -> Result<SnapshotId> {
        let key = keys::system_key(SYSTEM_RETAIN_FROM);
        let data = self
            .db
            .get(&key)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?
            .ok_or(SlateDuckError::CatalogNotInitialized)?;
        slateduck_core::encoding::decode_counter(&data)
    }

    /// Pin a snapshot to prevent GC from advancing past it.
    pub async fn pin_snapshot(&self, snapshot_id: SnapshotId) -> Result<()> {
        let key = keys::system_key(SYSTEM_RETAIN_FROM);
        let current = self.retain_from().await?;
        if snapshot_id < current {
            // Can't pin something already GC'd — but we allow pinning at current or later
            return Err(SlateDuckError::Internal(format!(
                "cannot pin snapshot {snapshot_id}: already past retain-from {current}"
            )));
        }
        // Pin means we write the snapshot_id as the new retain-from
        // (the caller is asserting: "don't GC past this")
        self.db
            .put(&key, slateduck_core::encoding::encode_counter(snapshot_id))
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;
        Ok(())
    }

    /// Close the catalog store.
    pub async fn close(self) -> Result<()> {
        self.db
            .close()
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;
        Ok(())
    }
}
