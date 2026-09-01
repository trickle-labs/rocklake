//! CatalogReader: read catalog state at a specific DuckLake snapshot.

use base64::Engine as _;
use futures::stream::{self, BoxStream};
use futures::StreamExt;
use rocklake_core::keys;
use rocklake_core::mvcc::{self, SnapshotId};
use rocklake_core::rows::*;
use rocklake_core::tags::*;
use rocklake_core::types::DuckLakeType;
use rocklake_core::values;
use serde::{Deserialize, Serialize};
use slatedb::Db;

use crate::error::{CatalogError, CatalogResult};

// ─── v0.10: Snapshot Diff ──────────────────────────────────────────────────

/// Structured diff between two DuckLake snapshots.
///
/// Contains the sets of catalog facts that were added or retired in the
/// transition from `from_snapshot` to `to_snapshot`.  This is the primary
/// primitive for CDC export: every committed snapshot is a natural change
/// stream.
#[derive(Debug, Clone)]
pub struct SnapshotDiff {
    /// The base snapshot ("before" state).
    pub from_snapshot: SnapshotId,
    /// The target snapshot ("after" state).
    pub to_snapshot: SnapshotId,
    /// Schema rows first written at `to_snapshot`.
    pub added_schemas: Vec<SchemaRow>,
    /// Schema rows retired at `to_snapshot`.
    pub retired_schemas: Vec<SchemaRow>,
    /// Table rows first written at `to_snapshot`.
    pub added_tables: Vec<TableRow>,
    /// Table rows retired at `to_snapshot`.
    pub retired_tables: Vec<TableRow>,
    /// Column rows first written at `to_snapshot`.
    pub added_columns: Vec<ColumnRow>,
    /// Column rows retired at `to_snapshot`.
    pub retired_columns: Vec<ColumnRow>,
    /// Data files registered in the `(from_snapshot, to_snapshot]` window.
    pub added_data_files: Vec<DataFileRow>,
    /// Data files logically deleted/replaced in the `(from_snapshot, to_snapshot]` window.
    pub retired_data_files: Vec<DataFileRow>,
}

impl SnapshotDiff {
    /// Returns true if there are no changes between the two snapshots.
    pub fn is_empty(&self) -> bool {
        self.added_schemas.is_empty()
            && self.retired_schemas.is_empty()
            && self.added_tables.is_empty()
            && self.retired_tables.is_empty()
            && self.added_columns.is_empty()
            && self.retired_columns.is_empty()
            && self.added_data_files.is_empty()
            && self.retired_data_files.is_empty()
    }

    /// Total number of changed facts.
    pub fn change_count(&self) -> usize {
        self.added_schemas.len()
            + self.retired_schemas.len()
            + self.added_tables.len()
            + self.retired_tables.len()
            + self.added_columns.len()
            + self.retired_columns.len()
            + self.added_data_files.len()
            + self.retired_data_files.len()
    }
}

/// Reads catalog state at a specific DuckLake snapshot ID.
#[derive(Clone)]
pub struct CatalogReader {
    db: Db,
    dl_snapshot_id: SnapshotId,
}

/// Maximum number of rows returned by one paginated data-file read.
pub const MAX_DATA_FILE_PAGE_SIZE: usize = 1_024;

/// A bounded page of data files and the cursor for the next page.
#[derive(Debug, Clone)]
pub struct DataFilePage {
    /// Rows visible at the reader's snapshot.
    pub files: Vec<DataFileRow>,
    /// Opaque cursor, or `None` when this is the last page.
    pub continuation_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DataFileCursor {
    version: u8,
    table_id: u64,
    snapshot_id: u64,
    page_size: usize,
    begin_snapshot: u64,
    data_file_id: u64,
}

fn decode_data_file_cursor(
    token: &str,
    table_id: u64,
    snapshot_id: u64,
    page_size: usize,
) -> CatalogResult<(u64, u64)> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(token)
        .map_err(|e| CatalogError::InvalidInput(format!("invalid continuation token: {e}")))?;
    let cursor: DataFileCursor = serde_json::from_slice(&bytes)
        .map_err(|e| CatalogError::InvalidInput(format!("invalid continuation token: {e}")))?;
    if cursor.version != 1
        || cursor.table_id != table_id
        || cursor.snapshot_id != snapshot_id
        || cursor.page_size != page_size
    {
        return Err(CatalogError::InvalidInput(
            "continuation token does not match this request".to_string(),
        ));
    }
    Ok((cursor.begin_snapshot, cursor.data_file_id))
}

fn encode_data_file_cursor(cursor: DataFileCursor) -> CatalogResult<String> {
    let bytes = serde_json::to_vec(&cursor)
        .map_err(|e| CatalogError::Internal(format!("encode continuation token: {e}")))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

impl CatalogReader {
    pub(crate) fn new(db: Db, dl_snapshot_id: SnapshotId) -> Self {
        Self { db, dl_snapshot_id }
    }

    /// Return the DuckLake snapshot ID this reader is bound to.
    pub fn snapshot_id(&self) -> SnapshotId {
        self.dl_snapshot_id
    }

    /// Read the `ducklake_snapshot` row for this snapshot, if it exists.
    pub async fn get_snapshot(&self) -> CatalogResult<Option<SnapshotRow>> {
        let key = keys::key_snapshot(self.dl_snapshot_id.as_u64());
        match self.db.get(&key).await? {
            None => Ok(None),
            Some(data) => Ok(Some(values::decode_value::<SnapshotRow>(&data)?)),
        }
    }

    /// List all `ducklake_snapshot_changes` rows across all snapshots.
    ///
    /// Returns one `SnapshotChangesRow` per snapshot that has a recorded
    /// changes entry.  The caller (response builder) is responsible for
    /// aggregating them into spec output rows.
    pub async fn list_all_snapshot_changes(&self) -> CatalogResult<Vec<SnapshotChangesRow>> {
        let prefix = keys::prefix_for_tag(TAG_SNAPSHOT_CHANGES);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: SnapshotChangesRow = values::decode_value(&kv.value)?;
            if row.snapshot_id <= self.dl_snapshot_id.as_u64() {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    /// List all schemas visible at this snapshot.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// use std::sync::Arc;
    /// use object_store::local::LocalFileSystem;
    /// use object_store::path::Path as ObjectPath;
    /// use rocklake_catalog::{CatalogStore, OpenOptions};
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// let store = Arc::new(LocalFileSystem::new_with_prefix(dir.path()).unwrap());
    /// let catalog = CatalogStore::open(OpenOptions { object_store: store, path: ObjectPath::from(""), encryption: None }).await.unwrap();
    /// let reader = catalog.read_at(rocklake_core::mvcc::SnapshotId::new(0)).unwrap();
    /// let schemas = reader.list_schemas().await.unwrap();
    /// assert!(schemas.is_empty());
    /// # });
    /// ```
    pub async fn list_schemas(&self) -> CatalogResult<Vec<SchemaRow>> {
        let prefix = keys::prefix_for_tag(TAG_SCHEMA);
        let mut schemas = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: SchemaRow = values::decode_value(&kv.value)?;
            if mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id) {
                schemas.push(row);
            }
        }
        Ok(schemas)
    }

    /// List all tables in a schema visible at this snapshot.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// use std::sync::Arc;
    /// use object_store::local::LocalFileSystem;
    /// use object_store::path::Path as ObjectPath;
    /// use rocklake_catalog::{CatalogStore, OpenOptions};
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// let store = Arc::new(LocalFileSystem::new_with_prefix(dir.path()).unwrap());
    /// let catalog = CatalogStore::open(OpenOptions { object_store: store, path: ObjectPath::from(""), encryption: None }).await.unwrap();
    /// let reader = catalog.read_at(rocklake_core::mvcc::SnapshotId::new(0)).unwrap();
    /// let tables = reader.list_tables(1).await.unwrap();
    /// assert!(tables.is_empty());
    /// # });
    /// ```
    pub async fn list_tables(&self, schema_id: u64) -> CatalogResult<Vec<TableRow>> {
        let prefix = keys::prefix_tables_for_schema(schema_id);
        let mut tables = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: TableRow = values::decode_value(&kv.value)?;
            if mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id) {
                tables.push(row);
            }
        }
        Ok(tables)
    }

    /// Return the table row and its columns visible at this snapshot, or `None` if not found.
    pub async fn describe_table(
        &self,
        table_id: u64,
    ) -> CatalogResult<Option<(TableRow, Vec<ColumnRow>)>> {
        // O(1) secondary-index lookup: TAG_TABLE_BY_ID → schema_id.
        let idx_key = keys::key_table_by_id(table_id);
        let schema_id_opt = match self.db.get(&idx_key).await? {
            Some(data) => Some(values::decode_counter(&data)?),
            None => None,
        };

        let table_row: Option<TableRow> = if let Some(schema_id) = schema_id_opt {
            // Use the narrow schema+table prefix — O(log n) in practice.
            let prefix = keys::prefix_tables_for_schema_table(schema_id, table_id);
            let mut best: Option<TableRow> = None;
            let mut iter = self.db.scan_prefix(&prefix).await?;
            while let Some(kv) = iter
                .next()
                .await
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?
            {
                let row: TableRow = values::decode_value(&kv.value)?;
                if mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id) {
                    match &best {
                        None => best = Some(row),
                        Some(existing) if row.begin_snapshot > existing.begin_snapshot => {
                            best = Some(row);
                        }
                        _ => {}
                    }
                }
            }
            best
        } else {
            // Fallback: full scan for catalogs predating the secondary index.
            let prefix = keys::prefix_for_tag(TAG_TABLE);
            let mut best: Option<TableRow> = None;
            let mut iter = self.db.scan_prefix(&prefix).await?;
            while let Some(kv) = iter
                .next()
                .await
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?
            {
                let row: TableRow = values::decode_value(&kv.value)?;
                if row.table_id == table_id
                    && mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id)
                {
                    match &best {
                        None => best = Some(row),
                        Some(existing) if row.begin_snapshot > existing.begin_snapshot => {
                            best = Some(row);
                        }
                        _ => {}
                    }
                }
            }
            best
        };

        let table = match table_row {
            None => return Ok(None),
            Some(t) => t,
        };

        let col_prefix = keys::prefix_columns_for_table(table_id);
        let mut columns = Vec::new();
        let mut iter = self.db.scan_prefix(&col_prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: ColumnRow = values::decode_value(&kv.value)?;
            if mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id) {
                columns.push(row);
            }
        }

        columns.sort_by(|a, b| {
            a.column_id
                .cmp(&b.column_id)
                .then(b.begin_snapshot.cmp(&a.begin_snapshot))
        });
        columns.dedup_by_key(|c| c.column_id);
        // v0.26: sort into a column tree — top-level columns by column_index,
        // then child columns (parent_column IS NOT NULL) following their parent.
        sort_columns_tree(&mut columns);

        Ok(Some((table, columns)))
    }

    /// List all data files for a table visible at the current snapshot.
    pub async fn list_data_files(&self, table_id: u64) -> CatalogResult<Vec<DataFileRow>> {
        // Use the secondary index TAG_DATA_FILE_BY_SNAPSHOT (0x21) for an
        // O(log N) range scan bounded by read_snapshot instead of scanning all
        // data files for the table and filtering in memory.
        let prefix = keys::prefix_data_files_by_snapshot_for_table(table_id);
        let upper =
            keys::prefix_data_files_by_snapshot_upper(table_id, self.dl_snapshot_id.as_u64());

        let mut files = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            // Stop once we exceed the upper bound (snapshot_id > read_snapshot).
            if let Some(ref upper_key) = upper {
                if kv.key.as_ref() >= upper_key.as_slice() {
                    break;
                }
            }
            let row: DataFileRow = values::decode_value(&kv.value)?;
            // v0.24: filter out rows retired at or before the requested snapshot.
            // The index key already encodes begin_snapshot in the key range, so only
            // end_snapshot filtering is needed here.
            if let Some(end) = row.end_snapshot {
                if end <= self.dl_snapshot_id.as_u64() {
                    continue;
                }
            }
            files.push(row);
        }

        // v0.24: order results by file_order (spec requirement).
        files.sort_by_key(|f| f.file_order.unwrap_or(f.data_file_id));
        Ok(files)
    }

    /// Return one bounded page of data files visible at this snapshot.
    ///
    /// The continuation token is valid only for the same table, snapshot, and
    /// page size. Results are ordered by the data-file snapshot index.
    pub async fn list_data_files_paged(
        &self,
        table_id: u64,
        page_size: usize,
        continuation_token: Option<&str>,
    ) -> CatalogResult<DataFilePage> {
        if page_size == 0 || page_size > MAX_DATA_FILE_PAGE_SIZE {
            return Err(CatalogError::InvalidInput(format!(
                "page size must be between 1 and {MAX_DATA_FILE_PAGE_SIZE}"
            )));
        }
        let cursor = continuation_token
            .map(|token| {
                decode_data_file_cursor(token, table_id, self.dl_snapshot_id.as_u64(), page_size)
            })
            .transpose()?;
        let upper =
            keys::prefix_data_files_by_snapshot_upper(table_id, self.dl_snapshot_id.as_u64());
        let start = cursor.map(|(begin_snapshot, data_file_id)| {
            keys::key_data_file_by_snapshot(table_id, begin_snapshot, data_file_id)
        });
        let mut iter = match (start, upper.clone()) {
            (Some(start), Some(upper)) => {
                self.db
                    .scan::<Vec<u8>, _>((
                        std::ops::Bound::Excluded(start),
                        std::ops::Bound::Excluded(upper),
                    ))
                    .await?
            }
            (Some(start), None) => {
                self.db
                    .scan::<Vec<u8>, _>((
                        std::ops::Bound::Excluded(start),
                        std::ops::Bound::Unbounded,
                    ))
                    .await?
            }
            (None, Some(upper)) => {
                self.db
                    .scan::<Vec<u8>, _>((
                        std::ops::Bound::Included(keys::prefix_data_files_by_snapshot_for_table(
                            table_id,
                        )),
                        std::ops::Bound::Excluded(upper),
                    ))
                    .await?
            }
            (None, None) => {
                self.db
                    .scan::<Vec<u8>, _>((
                        std::ops::Bound::Included(keys::prefix_data_files_by_snapshot_for_table(
                            table_id,
                        )),
                        std::ops::Bound::Unbounded,
                    ))
                    .await?
            }
        };
        let mut files = Vec::with_capacity(page_size);
        let mut last = None;
        let mut page_full = false;
        let mut has_more = false;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let key = kv.key.as_ref();
            if key.len() < 25 {
                return Err(CatalogError::Corruption(
                    "data-file snapshot index key is shorter than 25 bytes".to_string(),
                ));
            }
            let begin_snapshot = keys::decode_u64(&key[9..17])?;
            let data_file_id = keys::decode_u64(&key[17..25])?;
            let row: DataFileRow = values::decode_value(&kv.value)?;
            if row
                .end_snapshot
                .is_some_and(|end| end <= self.dl_snapshot_id.as_u64())
            {
                continue;
            }
            if page_full {
                has_more = true;
                break;
            }
            last = Some((begin_snapshot, data_file_id));
            files.push(row);
            if files.len() == page_size {
                page_full = true;
            }
        }
        let continuation_token = if let Some((begin_snapshot, data_file_id)) = last {
            if has_more {
                Some(encode_data_file_cursor(DataFileCursor {
                    version: 1,
                    table_id,
                    snapshot_id: self.dl_snapshot_id.as_u64(),
                    page_size,
                    begin_snapshot,
                    data_file_id,
                })?)
            } else {
                None
            }
        } else {
            None
        };
        Ok(DataFilePage {
            files,
            continuation_token,
        })
    }

    /// Stream data files with at most one decoded row buffered.
    // ponytail: pull-based iteration provides backpressure and cancellation;
    // a producer channel would add memory without a measured throughput need.
    pub async fn stream_data_files(
        &self,
        table_id: u64,
    ) -> CatalogResult<BoxStream<'static, CatalogResult<DataFileRow>>> {
        let prefix = keys::prefix_data_files_by_snapshot_for_table(table_id);
        let upper =
            keys::prefix_data_files_by_snapshot_upper(table_id, self.dl_snapshot_id.as_u64());
        let snapshot_id = self.dl_snapshot_id.as_u64();
        let iter = self.db.scan_prefix(&prefix).await?;
        Ok(
            stream::try_unfold((iter, upper), move |(mut iter, upper)| async move {
                loop {
                    let Some(kv) = iter
                        .next()
                        .await
                        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
                    else {
                        return Ok(None);
                    };
                    if upper
                        .as_ref()
                        .is_some_and(|key| kv.key.as_ref() >= key.as_slice())
                    {
                        return Ok(None);
                    }
                    let row: DataFileRow = values::decode_value(&kv.value)?;
                    if row.end_snapshot.is_some_and(|end| end <= snapshot_id) {
                        continue;
                    }
                    return Ok(Some((row, (iter, upper))));
                }
            })
            .boxed(),
        )
    }

    /// List delete files visible at the current snapshot.
    ///
    /// v0.24: implements spec MVCC visibility: `begin_snapshot ≤ snapshot_id`
    /// and (`end_snapshot IS NULL` or `end_snapshot > snapshot_id`).
    pub async fn list_delete_files(&self, table_id: u64) -> CatalogResult<Vec<DeleteFileRow>> {
        use rocklake_core::tags::TAG_DELETE_FILE;
        let prefix = keys::prefix_for_tag(TAG_DELETE_FILE);
        let snap = self.dl_snapshot_id.as_u64();
        let mut files = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: DeleteFileRow = values::decode_value(&kv.value)?;
            // Filter by table_id if populated; resolve owning data file if table_id is missing.
            let matches_table = match row.table_id {
                Some(tid) => tid == table_id,
                None => {
                    let df_key = keys::key_data_file(table_id, row.data_file_id);
                    self.db.get(&df_key).await?.is_some()
                }
            };
            if !matches_table {
                continue;
            }
            // MVCC visibility using begin_snapshot / end_snapshot if present,
            // falling back to legacy snapshot_id.
            let begin = row.begin_snapshot.unwrap_or(row.snapshot_id);
            if begin > snap {
                continue;
            }
            if let Some(end) = row.end_snapshot {
                if end <= snap {
                    continue;
                }
            }
            files.push(row);
        }
        Ok(files)
    }

    /// Return aggregate table stats for the given table, if recorded.
    pub async fn get_table_stats(&self, table_id: u64) -> CatalogResult<Option<TableStatsRow>> {
        let key = keys::key_table_stats(table_id);
        match self.db.get(&key).await? {
            Some(data) => Ok(Some(values::decode_value(&data)?)),
            None => Ok(None),
        }
    }

    /// List all aggregate table stats rows for tables visible at this snapshot.
    pub async fn list_all_table_stats(&self) -> CatalogResult<Vec<TableStatsRow>> {
        let prefix = keys::prefix_for_tag(TAG_TABLE_STATS);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: TableStatsRow = values::decode_value(&kv.value)?;
            rows.push(row);
        }
        Ok(rows)
    }

    /// Return data file IDs that survive a statistics-based predicate prune for a column.
    pub async fn prune_files(
        &self,
        table_id: u64,
        column_id: u64,
        predicate_value: &str,
        col_type: &DuckLakeType,
    ) -> CatalogResult<Vec<u64>> {
        use rocklake_core::types::{prune_file, type_aware_compare, PruneResult};

        let visible_files = self.list_data_files(table_id).await?;
        let mut kept_file_ids = Vec::new();

        for file in visible_files {
            let key = keys::key_file_column_stats(table_id, column_id, file.data_file_id);
            let mut should_keep = true;

            if let Some(data) = self.db.get(&key).await? {
                if let Ok(row) = values::decode_value::<FileColumnStatsRow>(&data) {
                    let has_stats =
                        row.min_value.is_some() || row.max_value.is_some() || row.contains_nan;
                    if has_stats {
                        match prune_file(
                            predicate_value,
                            row.min_value.as_deref(),
                            row.max_value.as_deref(),
                            row.contains_nan,
                            col_type,
                        ) {
                            Ok(PruneResult::Prune) => {
                                should_keep = false;
                            }
                            Ok(PruneResult::Keep) => {
                                should_keep = true;
                            }
                            Err(_) => {
                                // malformed, NaN, or unsupported -> conservatively keep
                                should_keep = true;
                            }
                        }
                    }
                }
            }

            // Check partial_max pruning only if still a candidate to prune
            if should_keep {
                if let Some(ref partial_max) = file.partial_max {
                    if !partial_max.is_empty() {
                        if let Ok(std::cmp::Ordering::Greater) =
                            type_aware_compare(predicate_value, partial_max, col_type)
                        {
                            should_keep = false;
                        }
                    }
                }
            }

            if should_keep {
                kept_file_ids.push(file.data_file_id);
            }
        }

        Ok(kept_file_ids)
    }

    /// List file-level column statistics for a table column for files visible at this snapshot.
    pub async fn list_file_column_stats(
        &self,
        table_id: u64,
        column_id: u64,
    ) -> CatalogResult<Vec<FileColumnStatsRow>> {
        let visible_files = self.list_data_files(table_id).await?;
        let visible_file_ids: std::collections::HashSet<u64> =
            visible_files.into_iter().map(|f| f.data_file_id).collect();

        let mut prefix = Vec::with_capacity(17);
        prefix.push(TAG_FILE_COLUMN_STATS);
        prefix.extend_from_slice(&keys::encode_u64(table_id));
        prefix.extend_from_slice(&keys::encode_u64(column_id));

        let mut stats = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: FileColumnStatsRow = values::decode_value(&kv.value)?;
            if visible_file_ids.contains(&row.data_file_id) {
                stats.push(row);
            }
        }
        Ok(stats)
    }

    /// Look up a single metadata entry by scope, scope ID, and key.
    pub async fn get_metadata(
        &self,
        scope: rocklake_core::keys::MetadataScope,
        scope_id: u64,
        key: &str,
    ) -> CatalogResult<Option<MetadataRow>> {
        let k = keys::key_metadata(scope, scope_id, key);
        match self.db.get(&k).await? {
            None => Ok(None),
            Some(data) => Ok(Some(values::decode_value::<MetadataRow>(&data)?)),
        }
    }

    /// List inlined-insert rows for a table visible at the current snapshot.
    pub async fn list_inlined_inserts(
        &self,
        table_id: u64,
    ) -> CatalogResult<Vec<InlinedInsertRow>> {
        let prefix = keys::prefix_inlined_inserts_for_table(table_id);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: InlinedInsertRow = values::decode_value(&kv.value)?;
            if mvcc::is_inlined_insert_visible(
                row.begin_snapshot,
                row.end_snapshot,
                self.dl_snapshot_id,
            ) {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    /// List registered inlined data tables, optionally scoped to a table id.
    pub async fn list_inlined_data_tables(
        &self,
        table_id: Option<u64>,
    ) -> CatalogResult<Vec<InlinedDataTablesRow>> {
        let prefix = keys::prefix_for_tag(TAG_INLINED_DATA_TABLES);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: InlinedDataTablesRow = values::decode_value(&kv.value)?;
            if table_id.map(|id| id == row.table_id).unwrap_or(true) {
                rows.push(row);
            }
        }
        rows.sort_by_key(|row| (row.table_id, row.schema_version));
        Ok(rows)
    }

    /// List inlined-delete rows for a table visible at the current snapshot.
    pub async fn list_inlined_deletes(
        &self,
        table_id: u64,
    ) -> CatalogResult<Vec<InlinedDeleteRow>> {
        let prefix = keys::prefix_inlined_deletes_for_table(table_id);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: InlinedDeleteRow = values::decode_value(&kv.value)?;
            if mvcc::is_inlined_delete_visible_at(
                row.begin_snapshot,
                row.end_snapshot,
                self.dl_snapshot_id,
            ) {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    // ─── Phase 6: Views ────────────────────────────────────────────────────

    /// List all views in a schema visible at this snapshot.
    pub async fn list_views(&self, schema_id: u64) -> CatalogResult<Vec<ViewRow>> {
        let prefix = keys::prefix_views_for_schema(schema_id);
        let mut views = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: ViewRow = values::decode_value(&kv.value)?;
            if mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id) {
                views.push(row);
            }
        }
        Ok(views)
    }

    /// v0.25: List all visible views across all schemas.
    pub async fn list_all_views(&self) -> CatalogResult<Vec<ViewRow>> {
        let prefix = keys::prefix_for_tag(rocklake_core::tags::TAG_VIEW);
        let mut views = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: ViewRow = values::decode_value(&kv.value)?;
            if mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id) {
                views.push(row);
            }
        }
        Ok(views)
    }

    // ─── Phase 6: Macros ────────────────────────────────────────────────────

    /// List all macros in a schema visible at this snapshot.
    pub async fn list_macros(&self, schema_id: u64) -> CatalogResult<Vec<MacroRow>> {
        let prefix = keys::prefix_macros_for_schema(schema_id);
        let mut macros = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: MacroRow = values::decode_value(&kv.value)?;
            if mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id) {
                macros.push(row);
            }
        }
        Ok(macros)
    }

    /// v0.25: List all visible macros across all schemas.
    pub async fn list_all_macros(&self) -> CatalogResult<Vec<MacroRow>> {
        let prefix = keys::prefix_for_tag(rocklake_core::tags::TAG_MACRO);
        let mut macros = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: MacroRow = values::decode_value(&kv.value)?;
            if mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id) {
                macros.push(row);
            }
        }
        Ok(macros)
    }

    /// v0.25: List all metadata entries (all scopes) for the SQL facade.
    pub async fn list_all_metadata(&self) -> CatalogResult<Vec<MetadataRow>> {
        let prefix = keys::prefix_all_metadata();
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: MetadataRow = values::decode_value(&kv.value)?;
            rows.push(row);
        }
        Ok(rows)
    }

    /// List all implementations of a macro.
    pub async fn list_macro_impls(&self, macro_id: u64) -> CatalogResult<Vec<MacroImplRow>> {
        let prefix = keys::prefix_macro_impls(macro_id);
        let mut impls = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: MacroImplRow = values::decode_value(&kv.value)?;
            impls.push(row);
        }
        Ok(impls)
    }

    /// List all parameter rows for a macro implementation.
    pub async fn list_macro_parameters(
        &self,
        macro_id: u64,
        impl_id: u64,
    ) -> CatalogResult<Vec<MacroParametersRow>> {
        let prefix = keys::prefix_macro_params(macro_id, impl_id);
        let mut params = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: MacroParametersRow = values::decode_value(&kv.value)?;
            params.push(row);
        }
        Ok(params)
    }

    // ─── Phase 6: Tags ──────────────────────────────────────────────────────

    /// List all tags for an object visible at this snapshot.
    pub async fn list_tags(&self, object_id: u64) -> CatalogResult<Vec<TagRow>> {
        let prefix = keys::prefix_tags_for_object(object_id);
        let mut tags = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: TagRow = values::decode_value(&kv.value)?;
            if mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id) {
                tags.push(row);
            }
        }
        Ok(tags)
    }

    /// List all column-level tags for a table column visible at this snapshot.
    pub async fn list_column_tags(
        &self,
        table_id: u64,
        column_id: u64,
    ) -> CatalogResult<Vec<ColumnTagRow>> {
        let prefix = keys::prefix_column_tags(table_id, column_id);
        let mut tags = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: ColumnTagRow = values::decode_value(&kv.value)?;
            if mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id) {
                tags.push(row);
            }
        }
        Ok(tags)
    }

    // ─── Phase 6: File Variant Stats ────────────────────────────────────────

    // ─── Phase 6: File Variant Stats ────────────────────────────────────────

    /// List file-level variant statistics for a column for files visible at this snapshot.
    pub async fn list_file_variant_stats(
        &self,
        table_id: u64,
        column_id: u64,
    ) -> CatalogResult<Vec<FileVariantStatsRow>> {
        let visible_files = self.list_data_files(table_id).await?;
        let visible_file_ids: std::collections::HashSet<u64> =
            visible_files.into_iter().map(|f| f.data_file_id).collect();

        let mut buf = Vec::with_capacity(17);
        buf.push(TAG_FILE_VARIANT_STATS);
        buf.extend_from_slice(&keys::encode_u64(table_id));
        buf.extend_from_slice(&keys::encode_u64(column_id));

        let mut stats = Vec::new();
        let mut iter = self.db.scan_prefix(&buf).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: FileVariantStatsRow = values::decode_value(&kv.value)?;
            if visible_file_ids.contains(&row.data_file_id) {
                stats.push(row);
            }
        }
        Ok(stats)
    }

    // ─── Phase 6: Files Scheduled for Deletion ──────────────────────────────

    /// List all data files scheduled for deletion (GC candidates).
    pub async fn list_files_scheduled_for_deletion(
        &self,
    ) -> CatalogResult<Vec<FilesScheduledForDeletionRow>> {
        let prefix = keys::prefix_for_tag(TAG_FILES_SCHEDULED_FOR_DELETION);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: FilesScheduledForDeletionRow = values::decode_value(&kv.value)?;
            rows.push(row);
        }
        Ok(rows)
    }

    // ─── v0.27: All-tags / all-column-tags / sort-info ──────────────────────

    /// List all `ducklake_tag` rows visible at this snapshot (all objects).
    pub async fn list_all_tags(&self) -> CatalogResult<Vec<TagRow>> {
        let prefix = keys::prefix_for_tag(TAG_TAG);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: TagRow = values::decode_value(&kv.value)?;
            if mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id) {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    /// List all `ducklake_column_tag` rows visible at this snapshot (all tables).
    pub async fn list_all_column_tags(&self) -> CatalogResult<Vec<ColumnTagRow>> {
        let prefix = keys::prefix_for_tag(TAG_COLUMN_TAG);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: ColumnTagRow = values::decode_value(&kv.value)?;
            if mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id) {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    /// List all `ducklake_sort_info` rows visible at this snapshot (all tables).
    pub async fn list_all_sort_info(&self) -> CatalogResult<Vec<SortInfoRow>> {
        let prefix = keys::prefix_for_tag(TAG_SORT_INFO);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: SortInfoRow = values::decode_value(&kv.value)?;
            if mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id) {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    /// List all `ducklake_schema_versions` rows visible at this snapshot (all tables).
    pub async fn list_all_schema_versions(&self) -> CatalogResult<Vec<SchemaVersionsRow>> {
        let prefix = keys::prefix_for_tag(TAG_SCHEMA_VERSIONS);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: SchemaVersionsRow = values::decode_value(&kv.value)?;
            if row.begin_snapshot <= self.dl_snapshot_id.as_u64() {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    /// List all `ducklake_table_column_stats` rows visible at this snapshot.
    pub async fn list_all_table_column_stats(&self) -> CatalogResult<Vec<TableColumnStatsRow>> {
        let prefix = keys::prefix_for_tag(TAG_TABLE_COLUMN_STATS);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: TableColumnStatsRow = values::decode_value(&kv.value)?;
            rows.push(row);
        }
        Ok(rows)
    }

    /// List all `ducklake_column_mapping` rows visible at this snapshot.
    pub async fn list_column_mappings(&self) -> CatalogResult<Vec<ColumnMappingRow>> {
        let prefix = keys::prefix_for_tag(TAG_COLUMN_MAPPING);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: ColumnMappingRow = values::decode_value(&kv.value)?;
            if self.describe_table(row.table_id).await?.is_some() {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    /// List all `ducklake_name_mapping` rows visible at this snapshot.
    pub async fn list_name_mappings(&self) -> CatalogResult<Vec<NameMappingRow>> {
        let visible_columns: std::collections::HashSet<u64> = {
            let schemas = self.list_schemas().await?;
            let mut cols = std::collections::HashSet::new();
            for s in schemas {
                for t in self.list_tables(s.schema_id).await? {
                    if let Some((_, table_cols)) = self.describe_table(t.table_id).await? {
                        for c in table_cols {
                            cols.insert(c.column_id);
                        }
                    }
                }
            }
            cols
        };

        let prefix = keys::prefix_for_tag(TAG_NAME_MAPPING);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: NameMappingRow = values::decode_value(&kv.value)?;
            if visible_columns.is_empty() || visible_columns.contains(&row.column_id) {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    /// List all `ducklake_partition_info` rows visible at this snapshot for a table.
    pub async fn list_partition_info(&self, table_id: u64) -> CatalogResult<Vec<PartitionInfoRow>> {
        let prefix = keys::prefix_for_tag(TAG_PARTITION_INFO);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: PartitionInfoRow = values::decode_value(&kv.value)?;
            if row.table_id == table_id
                && mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id)
            {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    /// List all `ducklake_partition_column` rows visible at this snapshot for a partition.
    pub async fn list_partition_columns(
        &self,
        partition_id: u64,
    ) -> CatalogResult<Vec<PartitionColumnRow>> {
        let prefix = keys::prefix_for_tag(TAG_PARTITION_COLUMN);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: PartitionColumnRow = values::decode_value(&kv.value)?;
            if row.partition_id == partition_id {
                rows.push(row);
            }
        }
        // Sort by partition_key_index
        rows.sort_by_key(|r| r.partition_key_index);
        Ok(rows)
    }

    /// List all `ducklake_partition_column` rows visible at this snapshot.
    pub async fn list_all_partition_columns(&self) -> CatalogResult<Vec<PartitionColumnRow>> {
        let prefix = keys::prefix_for_tag(TAG_PARTITION_COLUMN);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: PartitionColumnRow = values::decode_value(&kv.value)?;
            let visible = if let Some(tid) = row.table_id {
                self.describe_table(tid).await?.is_some()
            } else {
                true
            };
            if visible {
                rows.push(row);
            }
        }
        rows.sort_by_key(|r| (r.partition_id, r.partition_key_index));
        Ok(rows)
    }

    /// List all `ducklake_sort_expression` rows visible at this snapshot for a table.
    pub async fn list_sort_expressions(
        &self,
        table_id: u64,
    ) -> CatalogResult<Vec<SortExpressionRow>> {
        let prefix = keys::prefix_for_tag(TAG_SORT_EXPRESSION);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: SortExpressionRow = values::decode_value(&kv.value)?;
            if row.table_id.unwrap_or(0) == table_id {
                rows.push(row);
            }
        }
        // Sort by sort_id
        rows.sort_by_key(|r| r.sort_id);
        Ok(rows)
    }

    /// List all `ducklake_sort_expression` rows visible at this snapshot.
    pub async fn list_all_sort_expressions(&self) -> CatalogResult<Vec<SortExpressionRow>> {
        let prefix = keys::prefix_for_tag(TAG_SORT_EXPRESSION);
        let mut rows = Vec::new();
        let mut iter = self.db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: SortExpressionRow = values::decode_value(&kv.value)?;
            let visible = if let Some(tid) = row.table_id {
                self.describe_table(tid).await?.is_some()
            } else {
                true
            };
            if visible {
                rows.push(row);
            }
        }
        rows.sort_by_key(|r| (r.table_id.unwrap_or(0), r.sort_id, r.sort_key_index));
        Ok(rows)
    }

    // ─── v0.10: Snapshot Diff (CDC Output Primitive) ────────────────────────

    /// Compute the diff between two snapshots.
    ///
    /// Returns the set of catalog facts that changed between `from_snapshot`
    /// and `to_snapshot` — specifically the rows whose `begin_snapshot` equals
    /// `to_snapshot` (newly added) and rows whose `end_snapshot` equals
    /// `to_snapshot` (retired at that snapshot).
    ///
    /// This is the foundational primitive for CDC output: every committed
    /// snapshot is a natural change stream for rows that carry begin/end
    /// versioning.
    pub async fn snapshot_diff(
        &self,
        from_snapshot: impl Into<SnapshotId>,
        to_snapshot: impl Into<SnapshotId>,
    ) -> CatalogResult<SnapshotDiff> {
        let to_snapshot: SnapshotId = to_snapshot.into();
        let from_snapshot: SnapshotId = from_snapshot.into();
        let to = to_snapshot.as_u64();
        let from = from_snapshot.as_u64();

        if from > to {
            return Err(CatalogError::InvalidInput(format!(
                "from_snapshot ({from}) cannot exceed to_snapshot ({to})"
            )));
        }

        let retain_from = crate::gc::read_retain_from(&self.db).await.unwrap_or(0);
        if retain_from > 0 && from < retain_from {
            return Err(CatalogError::SnapshotOutOfRetention {
                requested: from,
                retain_from,
            });
        }

        let next_snap_key = keys::key_counter(rocklake_core::tags::COUNTER_NEXT_SNAPSHOT_ID);
        let latest_committed = match self.db.get(&next_snap_key).await? {
            Some(data) => rocklake_core::values::decode_counter(&data)
                .unwrap_or(1)
                .saturating_sub(1),
            None => 0,
        };
        if to > latest_committed {
            return Err(CatalogError::SnapshotNotFound {
                requested: to,
                latest_committed,
            });
        }

        let mut added_schemas: Vec<SchemaRow> = Vec::new();
        let mut retired_schemas: Vec<SchemaRow> = Vec::new();
        let mut added_tables: Vec<TableRow> = Vec::new();
        let mut retired_tables: Vec<TableRow> = Vec::new();
        let mut added_columns: Vec<ColumnRow> = Vec::new();
        let mut retired_columns: Vec<ColumnRow> = Vec::new();
        let mut added_data_files: Vec<DataFileRow> = Vec::new();
        let mut retired_data_files: Vec<DataFileRow> = Vec::new();

        // ── schemas ──────────────────────────────────────────────────────────
        {
            let prefix = keys::prefix_for_tag(TAG_SCHEMA);
            let mut iter = self.db.scan_prefix(&prefix).await?;
            while let Some(kv) = iter
                .next()
                .await
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?
            {
                let row: SchemaRow = values::decode_value(&kv.value)?;
                if row.begin_snapshot > from && row.begin_snapshot <= to {
                    added_schemas.push(row.clone());
                }
                if let Some(end) = row.end_snapshot {
                    if end > from && end <= to {
                        retired_schemas.push(row);
                    }
                }
            }
        }

        // ── tables ───────────────────────────────────────────────────────────
        {
            let prefix = keys::prefix_for_tag(TAG_TABLE);
            let mut iter = self.db.scan_prefix(&prefix).await?;
            while let Some(kv) = iter
                .next()
                .await
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?
            {
                let row: TableRow = values::decode_value(&kv.value)?;
                if row.begin_snapshot > from && row.begin_snapshot <= to {
                    added_tables.push(row.clone());
                }
                if let Some(end) = row.end_snapshot {
                    if end > from && end <= to {
                        retired_tables.push(row);
                    }
                }
            }
        }

        // ── columns ──────────────────────────────────────────────────────────
        {
            let prefix = keys::prefix_for_tag(TAG_COLUMN);
            let mut iter = self.db.scan_prefix(&prefix).await?;
            while let Some(kv) = iter
                .next()
                .await
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?
            {
                let row: ColumnRow = values::decode_value(&kv.value)?;
                if row.begin_snapshot > from && row.begin_snapshot <= to {
                    added_columns.push(row.clone());
                }
                if let Some(end) = row.end_snapshot {
                    if end > from && end <= to {
                        retired_columns.push(row);
                    }
                }
            }
        }

        // ── data files ───────────────────────────────────────────────────────
        // v0.24: use begin_snapshot/end_snapshot exclusively; snapshot_id was removed.
        {
            let prefix = keys::prefix_for_tag(TAG_DATA_FILE);
            let mut iter = self.db.scan_prefix(&prefix).await?;
            while let Some(kv) = iter
                .next()
                .await
                .map_err(|e| CatalogError::SlateDb(e.to_string()))?
            {
                let row: DataFileRow = values::decode_value(&kv.value)?;
                let begin = row.begin_snapshot.unwrap_or(0);
                if begin > from && begin <= to {
                    added_data_files.push(row.clone());
                }
                if let Some(end) = row.end_snapshot {
                    if end > from && end <= to {
                        retired_data_files.push(row);
                    }
                }
            }
        }

        Ok(SnapshotDiff {
            from_snapshot,
            to_snapshot,
            added_schemas,
            retired_schemas,
            added_tables,
            retired_tables,
            added_columns,
            retired_columns,
            added_data_files,
            retired_data_files,
        })
    }

    /// v0.26: Look up the column type string for a given (table_id, column_id).
    ///
    /// Returns the `column_type` field from the most-recent visible `ColumnRow`,
    /// or `None` if the column is not found at this snapshot.
    pub async fn get_column_type(
        &self,
        table_id: u64,
        column_id: u64,
    ) -> CatalogResult<Option<String>> {
        let col_prefix = keys::prefix_columns_for_table(table_id);
        let mut best: Option<ColumnRow> = None;
        let mut iter = self.db.scan_prefix(&col_prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let row: ColumnRow = values::decode_value(&kv.value)?;
            if row.column_id == column_id
                && mvcc::is_visible(row.begin_snapshot, row.end_snapshot, self.dl_snapshot_id)
            {
                match &best {
                    None => best = Some(row),
                    Some(existing) if row.begin_snapshot > existing.begin_snapshot => {
                        best = Some(row);
                    }
                    _ => {}
                }
            }
        }
        Ok(best.map(|r| r.data_type))
    }
}

// ─── Column Tree Sort ─────────────────────────────────────────────────────

/// Sort a flat list of column rows into column-tree order.
///
/// Top-level columns (parent_column IS NULL) are ordered by `column_index`.
/// Child columns follow their parent, ordered by `column_index` within the
/// same parent.  This handles arbitrarily nested struct columns.
fn sort_columns_tree(columns: &mut Vec<ColumnRow>) {
    // Separate top-level from nested columns.
    let mut top_level: Vec<ColumnRow> = std::mem::take(columns);

    // Sort everything by column_index first (stable order within each level).
    top_level.sort_by_key(|c| c.column_index);

    // Build a map from column_id to children.
    let mut children: std::collections::HashMap<u64, Vec<ColumnRow>> =
        std::collections::HashMap::new();
    let mut roots: Vec<ColumnRow> = Vec::new();
    for col in top_level {
        if let Some(parent_id) = col.parent_column {
            children.entry(parent_id).or_default().push(col);
        } else {
            roots.push(col);
        }
    }

    // Recursively expand each root into the output list.
    fn expand(
        col: ColumnRow,
        children: &mut std::collections::HashMap<u64, Vec<ColumnRow>>,
        out: &mut Vec<ColumnRow>,
    ) {
        let col_id = col.column_id;
        out.push(col);
        if let Some(mut kids) = children.remove(&col_id) {
            kids.sort_by_key(|c| c.column_index);
            for kid in kids {
                expand(kid, children, out);
            }
        }
    }

    for root in roots {
        expand(root, &mut children, columns);
    }
    // Append any orphaned children that had an unknown parent (shouldn't happen normally).
    for (_, orphans) in children {
        for orphan in orphans {
            columns.push(orphan);
        }
    }
}
