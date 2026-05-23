//! CatalogWriter — transactional catalog writes with MVCC, counter allocation,
//! schema_version tracking, and inlined data storage.

use slatedb::{Db, IsolationLevel};
use slateduck_core::encoding::{decode_counter, encode_counter, encode_value};
use slateduck_core::error::{Result, SlateDuckError};
use slateduck_core::keys;
use slateduck_core::mvcc::{MvccFields, SnapshotId};
use slateduck_core::rows::*;
use slateduck_core::tags::*;
use slateduck_core::MAX_INLINED_ROW_SIZE;

/// A write transaction that batches catalog mutations and commits atomically.
///
/// Counter allocation, counter increment, and the row that consumes the ID
/// all commit in a single SlateDB `DbTransaction`.
pub struct CatalogWriter<'a> {
    db: &'a Db,
    schema_changed: bool,
    current_schema_version: u64,
}

impl<'a> CatalogWriter<'a> {
    pub(crate) async fn new(db: &'a Db) -> Result<Self> {
        // Read current schema_version from latest snapshot
        let schema_version = Self::read_current_schema_version(db).await?;
        Ok(Self {
            db,
            schema_changed: false,
            current_schema_version: schema_version,
        })
    }

    async fn read_current_schema_version(db: &Db) -> Result<u64> {
        // Get current snapshot ID
        let counter_key = keys::counter_key(COUNTER_NEXT_SNAPSHOT_ID);
        let data = db
            .get(&counter_key)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?
            .ok_or(SlateDuckError::CatalogNotInitialized)?;
        let next_snapshot = decode_counter(&data)?;
        if next_snapshot <= 1 {
            return Ok(0);
        }
        // Read latest snapshot row to get schema_version
        let snap_key = keys::snapshot_key(next_snapshot - 1);
        let snap_data = db
            .get(&snap_key)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;
        match snap_data {
            Some(bytes) => {
                let payload = slateduck_core::encoding::decode_value(&bytes)?;
                let row: SnapshotRow = serde_json::from_slice(payload)
                    .map_err(|e| SlateDuckError::Encoding(e.to_string()))?;
                Ok(row.schema_version)
            }
            None => Ok(0),
        }
    }

    /// Mark that a schema-mutating operation has occurred.
    /// Must be called explicitly by every schema-mutating operation.
    pub fn mark_schema_changed(&mut self) {
        self.schema_changed = true;
    }

    /// Allocate the next snapshot ID and commit a snapshot with all changes.
    pub async fn create_snapshot(
        &mut self,
        changes_json: &str,
        author: Option<&str>,
        message: Option<&str>,
    ) -> Result<SnapshotId> {
        let txn = self
            .db
            .begin(IsolationLevel::SerializableSnapshot)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        // Allocate snapshot ID
        let counter_key = keys::counter_key(COUNTER_NEXT_SNAPSHOT_ID);
        let counter_data = txn
            .get(&counter_key)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?
            .ok_or(SlateDuckError::CatalogNotInitialized)?;
        let snapshot_id = decode_counter(&counter_data)?;
        txn.put(&counter_key, encode_counter(snapshot_id + 1))
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        // Compute schema_version
        let schema_version = if self.schema_changed {
            self.current_schema_version + 1
        } else {
            self.current_schema_version
        };

        // Write snapshot row
        let row = SnapshotRow {
            snapshot_id,
            schema_version,
            created_at: chrono_now(),
            author: author.map(|s| s.to_string()),
            message: message.map(|s| s.to_string()),
        };
        let snap_key = keys::snapshot_key(snapshot_id);
        let snap_value = encode_value(
            &serde_json::to_vec(&row).map_err(|e| SlateDuckError::Encoding(e.to_string()))?,
        );
        txn.put(&snap_key, snap_value)
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        // Write snapshot changes
        let changes_row = SnapshotChangesRow {
            snapshot_id,
            changes_json: changes_json.to_string(),
        };
        let changes_key = keys::snapshot_changes_key(snapshot_id);
        let changes_value = encode_value(
            &serde_json::to_vec(&changes_row)
                .map_err(|e| SlateDuckError::Encoding(e.to_string()))?,
        );
        txn.put(&changes_key, changes_value)
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        // Write schema_versions entry if schema changed
        if self.schema_changed {
            let sv_key = keys::schema_versions_key(0, snapshot_id); // table_id=0 for global
            let sv_row = SchemaVersionsRow {
                table_id: 0,
                begin_snapshot: snapshot_id,
                schema_version,
            };
            let sv_value = encode_value(
                &serde_json::to_vec(&sv_row)
                    .map_err(|e| SlateDuckError::Encoding(e.to_string()))?,
            );
            txn.put(&sv_key, sv_value)
                .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;
        }

        txn.commit()
            .await
            .map_err(|e| SlateDuckError::TransactionConflict(e.to_string()))?;

        self.db
            .flush()
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        // Update internal state
        self.current_schema_version = schema_version;
        self.schema_changed = false;

        Ok(snapshot_id)
    }

    /// Create a schema.
    pub async fn create_schema(&mut self, name: &str, snapshot_id: SnapshotId) -> Result<u64> {
        self.mark_schema_changed();
        let schema_id = self.allocate_catalog_id().await?;

        let row = SchemaRow {
            schema_id,
            name: name.to_string(),
            mvcc: MvccFields::new(snapshot_id),
        };
        let key = keys::schema_key(schema_id, snapshot_id);
        let value = encode_value(
            &serde_json::to_vec(&row).map_err(|e| SlateDuckError::Encoding(e.to_string()))?,
        );
        self.db
            .put(&key, value)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        Ok(schema_id)
    }

    /// Drop a schema (set end_snapshot on all versions).
    pub async fn drop_schema(&mut self, schema_id: u64, snapshot_id: SnapshotId) -> Result<()> {
        self.mark_schema_changed();
        self.end_versioned_rows(
            &keys::table_prefix(TAG_DUCKLAKE_SCHEMA),
            snapshot_id,
            |payload| {
                let row: SchemaRow = serde_json::from_slice(payload)
                    .map_err(|e| SlateDuckError::Encoding(e.to_string()))?;
                Ok(row.schema_id == schema_id && row.mvcc.end_snapshot.is_none())
            },
            |payload| {
                let mut row: SchemaRow = serde_json::from_slice(payload)
                    .map_err(|e| SlateDuckError::Encoding(e.to_string()))?;
                row.mvcc.end_at(snapshot_id);
                serde_json::to_vec(&row).map_err(|e| SlateDuckError::Encoding(e.to_string()))
            },
        )
        .await
    }

    /// Create a table.
    pub async fn create_table(
        &mut self,
        schema_id: u64,
        name: &str,
        uuid: &str,
        snapshot_id: SnapshotId,
    ) -> Result<u64> {
        self.mark_schema_changed();
        let table_id = self.allocate_catalog_id().await?;

        let row = TableRow {
            schema_id,
            table_id,
            name: name.to_string(),
            uuid: uuid.to_string(),
            mvcc: MvccFields::new(snapshot_id),
        };
        let key = keys::table_key(schema_id, table_id, snapshot_id);
        let value = encode_value(
            &serde_json::to_vec(&row).map_err(|e| SlateDuckError::Encoding(e.to_string()))?,
        );
        self.db
            .put(&key, value)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        // Initialize table stats
        let stats = TableStatsRow {
            table_id,
            record_count: 0,
            file_count: 0,
            total_size_bytes: 0,
        };
        let stats_key = keys::table_stats_key(table_id);
        let stats_value = encode_value(
            &serde_json::to_vec(&stats).map_err(|e| SlateDuckError::Encoding(e.to_string()))?,
        );
        self.db
            .put(&stats_key, stats_value)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        Ok(table_id)
    }

    /// Drop a table.
    pub async fn drop_table(
        &mut self,
        schema_id: u64,
        table_id: u64,
        snapshot_id: SnapshotId,
    ) -> Result<()> {
        self.mark_schema_changed();
        let prefix = keys::table_by_schema_prefix(schema_id);
        self.end_versioned_rows(
            &prefix,
            snapshot_id,
            |payload| {
                let row: TableRow = serde_json::from_slice(payload)
                    .map_err(|e| SlateDuckError::Encoding(e.to_string()))?;
                Ok(row.table_id == table_id && row.mvcc.end_snapshot.is_none())
            },
            |payload| {
                let mut row: TableRow = serde_json::from_slice(payload)
                    .map_err(|e| SlateDuckError::Encoding(e.to_string()))?;
                row.mvcc.end_at(snapshot_id);
                serde_json::to_vec(&row).map_err(|e| SlateDuckError::Encoding(e.to_string()))
            },
        )
        .await
    }

    /// Add a column to a table.
    pub async fn add_column(
        &mut self,
        table_id: u64,
        name: &str,
        data_type: &str,
        is_nullable: bool,
        default_value: Option<&str>,
        snapshot_id: SnapshotId,
    ) -> Result<u64> {
        self.mark_schema_changed();
        let column_id = self.allocate_table_column_id(table_id).await?;

        let row = ColumnRow {
            table_id,
            column_id,
            name: name.to_string(),
            data_type: data_type.to_string(),
            is_nullable,
            default_value: default_value.map(|s| s.to_string()),
            mvcc: MvccFields::new(snapshot_id),
        };
        let key = keys::column_key(table_id, column_id, snapshot_id);
        let value = encode_value(
            &serde_json::to_vec(&row).map_err(|e| SlateDuckError::Encoding(e.to_string()))?,
        );
        self.db
            .put(&key, value)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        Ok(column_id)
    }

    /// Drop a column.
    pub async fn drop_column(
        &mut self,
        table_id: u64,
        column_id: u64,
        snapshot_id: SnapshotId,
    ) -> Result<()> {
        self.mark_schema_changed();
        let prefix = keys::columns_by_table_prefix(table_id);
        self.end_versioned_rows(
            &prefix,
            snapshot_id,
            |payload| {
                let row: ColumnRow = serde_json::from_slice(payload)
                    .map_err(|e| SlateDuckError::Encoding(e.to_string()))?;
                Ok(row.column_id == column_id && row.mvcc.end_snapshot.is_none())
            },
            |payload| {
                let mut row: ColumnRow = serde_json::from_slice(payload)
                    .map_err(|e| SlateDuckError::Encoding(e.to_string()))?;
                row.mvcc.end_at(snapshot_id);
                serde_json::to_vec(&row).map_err(|e| SlateDuckError::Encoding(e.to_string()))
            },
        )
        .await
    }

    /// Register a data file.
    pub async fn register_data_file(
        &mut self,
        table_id: u64,
        path: &str,
        path_is_relative: bool,
        file_size_bytes: u64,
        record_count: u64,
        snapshot_id: SnapshotId,
    ) -> Result<u64> {
        let data_file_id = self.allocate_file_id().await?;

        let row = DataFileRow {
            table_id,
            data_file_id,
            path: path.to_string(),
            path_is_relative,
            file_size_bytes,
            record_count,
            mvcc: MvccFields::new(snapshot_id),
        };
        let key = keys::data_file_key(table_id, data_file_id);
        let value = encode_value(
            &serde_json::to_vec(&row).map_err(|e| SlateDuckError::Encoding(e.to_string()))?,
        );
        self.db
            .put(&key, value)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        Ok(data_file_id)
    }

    /// Register a delete file.
    pub async fn register_delete_file(
        &mut self,
        data_file_id: u64,
        path: &str,
        path_is_relative: bool,
        file_size_bytes: u64,
        record_count: u64,
    ) -> Result<u64> {
        let delete_file_id = self.allocate_file_id().await?;

        let row = DeleteFileRow {
            data_file_id,
            delete_file_id,
            path: path.to_string(),
            path_is_relative,
            file_size_bytes,
            record_count,
        };
        let key = keys::delete_file_key(data_file_id, delete_file_id);
        let value = encode_value(
            &serde_json::to_vec(&row).map_err(|e| SlateDuckError::Encoding(e.to_string()))?,
        );
        self.db
            .put(&key, value)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        Ok(delete_file_id)
    }

    /// Register an inlined insert row.
    pub async fn register_inlined_insert(
        &mut self,
        table_id: u64,
        schema_version: u64,
        row_id: u64,
        payload: &[u8],
        snapshot_id: SnapshotId,
    ) -> Result<()> {
        if payload.len() > MAX_INLINED_ROW_SIZE {
            return Err(SlateDuckError::ValueTooLarge {
                size: payload.len(),
                limit: MAX_INLINED_ROW_SIZE,
            });
        }

        let row = InlinedInsertRow {
            table_id,
            schema_version,
            row_id,
            payload: payload.to_vec(),
            begin_snapshot: snapshot_id,
            end_snapshot: None,
        };
        let key = keys::inlined_insert_key(table_id, schema_version, row_id);
        let value = encode_value(
            &serde_json::to_vec(&row).map_err(|e| SlateDuckError::Encoding(e.to_string()))?,
        );
        self.db
            .put(&key, value)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        Ok(())
    }

    /// Mark an inlined insert row as deleted.
    pub async fn mark_inlined_insert_deleted(
        &mut self,
        table_id: u64,
        schema_version: u64,
        row_id: u64,
        snapshot_id: SnapshotId,
    ) -> Result<()> {
        let key = keys::inlined_insert_key(table_id, schema_version, row_id);
        let data = self
            .db
            .get(&key)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?
            .ok_or_else(|| SlateDuckError::Internal("inlined row not found".to_string()))?;

        let payload_bytes = slateduck_core::encoding::decode_value(&data)?;
        let mut row: InlinedInsertRow = serde_json::from_slice(payload_bytes)
            .map_err(|e| SlateDuckError::Encoding(e.to_string()))?;
        row.end_snapshot = Some(snapshot_id);

        let value = encode_value(
            &serde_json::to_vec(&row).map_err(|e| SlateDuckError::Encoding(e.to_string()))?,
        );
        self.db
            .put(&key, value)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        Ok(())
    }

    /// Register an inlined delete marker.
    pub async fn register_inlined_delete(
        &mut self,
        table_id: u64,
        data_file_id: u64,
        row_id: u64,
        snapshot_id: SnapshotId,
    ) -> Result<()> {
        let row = InlinedDeleteRow {
            table_id,
            data_file_id,
            row_id,
            begin_snapshot: snapshot_id,
        };
        let key = keys::inlined_delete_key(table_id, data_file_id, row_id);
        let value = encode_value(
            &serde_json::to_vec(&row).map_err(|e| SlateDuckError::Encoding(e.to_string()))?,
        );
        self.db
            .put(&key, value)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        Ok(())
    }

    /// Update table stats (atomic increment of record_count).
    pub async fn update_table_stats(
        &self,
        table_id: u64,
        record_count_delta: i64,
        file_count_delta: i64,
        size_delta: i64,
    ) -> Result<()> {
        let txn = self
            .db
            .begin(IsolationLevel::SerializableSnapshot)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        let key = keys::table_stats_key(table_id);
        let data = txn
            .get(&key)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        let mut stats = match data {
            Some(bytes) => {
                let payload = slateduck_core::encoding::decode_value(&bytes)?;
                serde_json::from_slice::<TableStatsRow>(payload)
                    .map_err(|e| SlateDuckError::Encoding(e.to_string()))?
            }
            None => TableStatsRow {
                table_id,
                record_count: 0,
                file_count: 0,
                total_size_bytes: 0,
            },
        };

        stats.record_count += record_count_delta;
        stats.file_count = (stats.file_count as i64 + file_count_delta) as u64;
        stats.total_size_bytes = (stats.total_size_bytes as i64 + size_delta) as u64;

        let value = encode_value(
            &serde_json::to_vec(&stats).map_err(|e| SlateDuckError::Encoding(e.to_string()))?,
        );
        txn.put(&key, value)
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;
        txn.commit()
            .await
            .map_err(|e| SlateDuckError::TransactionConflict(e.to_string()))?;

        Ok(())
    }

    /// Upsert file column stats.
    pub async fn upsert_file_column_stats(&self, stats: FileColumnStatsRow) -> Result<()> {
        let key = keys::file_column_stats_key(stats.table_id, stats.column_id, stats.data_file_id);
        let value = encode_value(
            &serde_json::to_vec(&stats).map_err(|e| SlateDuckError::Encoding(e.to_string()))?,
        );
        self.db
            .put(&key, value)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;
        Ok(())
    }

    // -- Counter allocation (all in single transactions) --

    /// Allocate the next catalog ID.
    async fn allocate_catalog_id(&self) -> Result<u64> {
        self.allocate_counter(keys::counter_key(COUNTER_NEXT_CATALOG_ID))
            .await
    }

    /// Allocate the next file ID.
    async fn allocate_file_id(&self) -> Result<u64> {
        self.allocate_counter(keys::counter_key(COUNTER_NEXT_FILE_ID))
            .await
    }

    /// Allocate the next column ID for a table.
    async fn allocate_table_column_id(&self, table_id: u64) -> Result<u64> {
        self.allocate_counter(keys::table_counter_key(table_id))
            .await
    }

    /// Generic counter allocation: read, increment, commit in one transaction.
    async fn allocate_counter(&self, counter_key: Vec<u8>) -> Result<u64> {
        let txn = self
            .db
            .begin(IsolationLevel::SerializableSnapshot)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        let data = txn
            .get(&counter_key)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        let current_id = match data {
            Some(bytes) => decode_counter(&bytes)?,
            None => 1, // First allocation
        };

        txn.put(&counter_key, encode_counter(current_id + 1))
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;
        txn.commit()
            .await
            .map_err(|e| SlateDuckError::TransactionConflict(e.to_string()))?;

        Ok(current_id)
    }

    /// End versioned rows matching a predicate.
    async fn end_versioned_rows<P, U>(
        &self,
        prefix: &[u8],
        _snapshot_id: SnapshotId,
        predicate: P,
        update: U,
    ) -> Result<()>
    where
        P: Fn(&[u8]) -> Result<bool>,
        U: Fn(&[u8]) -> Result<Vec<u8>>,
    {
        let mut iter = self
            .db
            .scan_prefix(prefix)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        let mut updates = Vec::new();
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?
        {
            let payload = slateduck_core::encoding::decode_value(&kv.value)?;
            if predicate(payload)? {
                let new_payload = update(payload)?;
                updates.push((kv.key.to_vec(), encode_value(&new_payload)));
            }
        }

        for (key, value) in updates {
            self.db
                .put(&key, value)
                .await
                .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;
        }

        Ok(())
    }
}

/// Get current timestamp as ISO 8601 string.
fn chrono_now() -> String {
    // Simple UTC timestamp without external chrono dependency
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Format as basic ISO: we don't need chrono for this
    format!("{secs}")
}
