//! CatalogReader — point-in-time reads at a DuckLake snapshot.

use slatedb::Db;
use slateduck_core::encoding::decode_value;
use slateduck_core::error::{Result, SlateDuckError};
use slateduck_core::keys;
use slateduck_core::mvcc::{MvccFields, SnapshotId};
use slateduck_core::rows::*;

/// A reader that provides consistent catalog reads at a specific DuckLake snapshot.
pub struct CatalogReader<'a> {
    db: &'a Db,
    dl_snapshot_id: SnapshotId,
}

impl<'a> CatalogReader<'a> {
    pub(crate) fn new(db: &'a Db, dl_snapshot_id: SnapshotId) -> Self {
        Self { db, dl_snapshot_id }
    }

    /// Get the DuckLake snapshot ID this reader is pinned to.
    pub fn snapshot_id(&self) -> SnapshotId {
        self.dl_snapshot_id
    }

    /// List all schemas visible at this snapshot.
    pub async fn list_schemas(&self) -> Result<Vec<SchemaRow>> {
        let prefix = keys::table_prefix(slateduck_core::tags::TAG_DUCKLAKE_SCHEMA);
        self.scan_versioned(&prefix, |payload| {
            serde_json::from_slice::<SchemaRow>(payload)
                .map_err(|e| SlateDuckError::Encoding(e.to_string()))
        })
        .await
    }

    /// List all tables in a schema visible at this snapshot.
    pub async fn list_tables(&self, schema_id: u64) -> Result<Vec<TableRow>> {
        let prefix = keys::table_by_schema_prefix(schema_id);
        self.scan_versioned(&prefix, |payload| {
            serde_json::from_slice::<TableRow>(payload)
                .map_err(|e| SlateDuckError::Encoding(e.to_string()))
        })
        .await
    }

    /// Describe a table: get its columns visible at this snapshot.
    pub async fn describe_table(&self, table_id: u64) -> Result<Vec<ColumnRow>> {
        let prefix = keys::columns_by_table_prefix(table_id);
        self.scan_versioned(&prefix, |payload| {
            serde_json::from_slice::<ColumnRow>(payload)
                .map_err(|e| SlateDuckError::Encoding(e.to_string()))
        })
        .await
    }

    /// List data files for a table visible at this snapshot.
    pub async fn list_data_files(&self, table_id: u64) -> Result<Vec<DataFileRow>> {
        let prefix = keys::data_files_by_table_prefix(table_id);
        self.scan_versioned(&prefix, |payload| {
            serde_json::from_slice::<DataFileRow>(payload)
                .map_err(|e| SlateDuckError::Encoding(e.to_string()))
        })
        .await
    }

    /// Get file column stats for pruning.
    pub async fn get_file_column_stats(
        &self,
        table_id: u64,
        column_id: u64,
    ) -> Result<Vec<FileColumnStatsRow>> {
        let prefix = keys::file_column_stats_prefix(table_id, column_id);
        self.scan_unversioned(&prefix, |payload| {
            serde_json::from_slice::<FileColumnStatsRow>(payload)
                .map_err(|e| SlateDuckError::Encoding(e.to_string()))
        })
        .await
    }

    /// Get a snapshot row by ID.
    pub async fn get_snapshot(&self, snapshot_id: u64) -> Result<Option<SnapshotRow>> {
        let key = keys::snapshot_key(snapshot_id);
        self.get_single(&key, |payload| {
            serde_json::from_slice::<SnapshotRow>(payload)
                .map_err(|e| SlateDuckError::Encoding(e.to_string()))
        })
        .await
    }

    /// Get table stats.
    pub async fn get_table_stats(&self, table_id: u64) -> Result<Option<TableStatsRow>> {
        let key = keys::table_stats_key(table_id);
        self.get_single(&key, |payload| {
            serde_json::from_slice::<TableStatsRow>(payload)
                .map_err(|e| SlateDuckError::Encoding(e.to_string()))
        })
        .await
    }

    /// List inlined insert rows for a table.
    pub async fn list_inlined_inserts(&self, table_id: u64) -> Result<Vec<InlinedInsertRow>> {
        let prefix = keys::inlined_inserts_by_table_prefix(table_id);
        let mut rows = Vec::new();
        let mut iter = self
            .db
            .scan_prefix(&prefix)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?
        {
            let payload = decode_value(&kv.value)?;
            let row: InlinedInsertRow = serde_json::from_slice(payload)
                .map_err(|e| SlateDuckError::Encoding(e.to_string()))?;
            // MVCC filter for inlined rows
            if row.begin_snapshot <= self.dl_snapshot_id
                && row
                    .end_snapshot
                    .map_or(true, |end| self.dl_snapshot_id < end)
            {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    /// List inlined delete markers for a table.
    pub async fn list_inlined_deletes(&self, table_id: u64) -> Result<Vec<InlinedDeleteRow>> {
        let prefix = keys::inlined_deletes_by_table_prefix(table_id);
        let mut rows = Vec::new();
        let mut iter = self
            .db
            .scan_prefix(&prefix)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?
        {
            let payload = decode_value(&kv.value)?;
            let row: InlinedDeleteRow = serde_json::from_slice(payload)
                .map_err(|e| SlateDuckError::Encoding(e.to_string()))?;
            if row.begin_snapshot <= self.dl_snapshot_id {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    /// Scan and deserialize versioned rows, applying MVCC filter.
    async fn scan_versioned<T, F>(&self, prefix: &[u8], deserialize: F) -> Result<Vec<T>>
    where
        F: Fn(&[u8]) -> Result<T>,
        T: HasMvcc,
    {
        let mut rows = Vec::new();
        let mut iter = self
            .db
            .scan_prefix(prefix)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?
        {
            let payload = decode_value(&kv.value)?;
            let row = deserialize(payload)?;
            if row.mvcc_fields().is_visible_at(self.dl_snapshot_id) {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    /// Scan and deserialize unversioned rows (no MVCC filter).
    async fn scan_unversioned<T, F>(&self, prefix: &[u8], deserialize: F) -> Result<Vec<T>>
    where
        F: Fn(&[u8]) -> Result<T>,
    {
        let mut rows = Vec::new();
        let mut iter = self
            .db
            .scan_prefix(prefix)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?
        {
            let payload = decode_value(&kv.value)?;
            let row = deserialize(payload)?;
            rows.push(row);
        }
        Ok(rows)
    }

    /// Get a single row by exact key.
    async fn get_single<T, F>(&self, key: &[u8], deserialize: F) -> Result<Option<T>>
    where
        F: Fn(&[u8]) -> Result<T>,
    {
        let data = self
            .db
            .get(key)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

        match data {
            Some(bytes) => {
                let payload = decode_value(&bytes)?;
                Ok(Some(deserialize(payload)?))
            }
            None => Ok(None),
        }
    }
}

/// Trait for rows that have MVCC fields.
pub trait HasMvcc {
    fn mvcc_fields(&self) -> &MvccFields;
}

impl HasMvcc for SchemaRow {
    fn mvcc_fields(&self) -> &MvccFields {
        &self.mvcc
    }
}

impl HasMvcc for TableRow {
    fn mvcc_fields(&self) -> &MvccFields {
        &self.mvcc
    }
}

impl HasMvcc for ColumnRow {
    fn mvcc_fields(&self) -> &MvccFields {
        &self.mvcc
    }
}

impl HasMvcc for DataFileRow {
    fn mvcc_fields(&self) -> &MvccFields {
        &self.mvcc
    }
}

impl HasMvcc for ViewRow {
    fn mvcc_fields(&self) -> &MvccFields {
        &self.mvcc
    }
}

impl HasMvcc for MacroRow {
    fn mvcc_fields(&self) -> &MvccFields {
        &self.mvcc
    }
}

impl HasMvcc for PartitionInfoRow {
    fn mvcc_fields(&self) -> &MvccFields {
        &self.mvcc
    }
}

impl HasMvcc for SortInfoRow {
    fn mvcc_fields(&self) -> &MvccFields {
        &self.mvcc
    }
}

impl HasMvcc for TagRow {
    fn mvcc_fields(&self) -> &MvccFields {
        &self.mvcc
    }
}

impl HasMvcc for ColumnTagRow {
    fn mvcc_fields(&self) -> &MvccFields {
        &self.mvcc
    }
}
