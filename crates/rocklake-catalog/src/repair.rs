//! Catalog repair tooling.
//!
//! Conservative repair rules:
//! - **Repairable:** orphaned dynamic inlined keys, stale counters, dangling rows
//!   outside retention window, missing optional stats rows.
//! - **Unrecoverable:** magic mismatch, Protobuf decode failure for retained row,
//!   missing `ducklake_snapshot` or `ducklake_metadata`, missing Parquet files for
//!   retained snapshots — refuse mutation, direct operator to restore.

#![allow(missing_docs)]

use rocklake_core::keys;
use rocklake_core::rows::*;
use rocklake_core::tags::*;
use rocklake_core::values;
use slatedb::{Db, WriteBatch};

use crate::error::{CatalogError, CatalogResult};

/// A proposed repair action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepairAction {
    /// Fix a stale counter value.
    FixCounter {
        name: String,
        current: u64,
        correct: u64,
    },
    /// Remove orphaned inlined row not referenced by any table.
    RemoveOrphanedInlinedRow { key_hex: String },
    /// Remove dangling stats row for non-existent table/column.
    RemoveDanglingStats { key_hex: String },
    /// Rebuild the table ID to schema ID index.
    RebuildTableIndex { table_id: u64, schema_id: u64 },
    /// Rebuild the data-file-by-snapshot index from its primary value.
    RebuildDataFileIndex {
        table_id: u64,
        data_file_id: u64,
        begin_snapshot: u64,
        value: Vec<u8>,
    },
}

/// Result of a repair planning operation.
#[derive(Debug, Clone)]
pub struct RepairPlan {
    pub actions: Vec<RepairAction>,
    pub unrecoverable_errors: Vec<String>,
}

impl RepairPlan {
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty() && self.unrecoverable_errors.is_empty()
    }

    pub fn has_unrecoverable(&self) -> bool {
        !self.unrecoverable_errors.is_empty()
    }
}

/// Result of a repair apply operation.
#[derive(Debug, Clone)]
pub struct RepairResult {
    pub actions_applied: u64,
    pub actions_failed: u64,
}

/// Plan repairs without applying them.
pub async fn repair_plan(db: &Db) -> CatalogResult<RepairPlan> {
    let mut actions = Vec::new();
    let mut unrecoverable_errors = Vec::new();

    // Check counters are consistent with actual data
    check_counter_consistency(db, &mut actions, &mut unrecoverable_errors).await?;

    // Check for orphaned inlined rows
    check_orphaned_inlined_rows(db, &mut actions).await?;

    // Check for dangling stats
    check_dangling_stats(db, &mut actions).await?;

    // Check the copies used by hot read paths
    check_index_consistency(db, &mut actions).await?;

    // Check for critical data integrity
    check_critical_integrity(db, &mut unrecoverable_errors).await?;

    actions.sort_by_key(|action| format!("{action:?}"));
    actions.dedup();

    Ok(RepairPlan {
        actions,
        unrecoverable_errors,
    })
}

/// Apply a repair plan.
#[tracing::instrument(skip(db, plan))]
pub async fn repair_apply(db: &Db, plan: &RepairPlan) -> CatalogResult<RepairResult> {
    if plan.has_unrecoverable() {
        return Err(CatalogError::RepairRefused(
            "unrecoverable errors present; restore from backup".to_string(),
        ));
    }

    let mut batch = WriteBatch::new();
    for action in &plan.actions {
        append_action(&mut batch, action)?;
    }
    if !plan.actions.is_empty() {
        db.write(batch).await?;
    }

    Ok(RepairResult {
        actions_applied: plan.actions.len() as u64,
        actions_failed: 0,
    })
}

fn append_action(batch: &mut WriteBatch, action: &RepairAction) -> CatalogResult<()> {
    match action {
        RepairAction::FixCounter { name, correct, .. } => {
            if let RepairAction::FixCounter { current, .. } = action {
                if correct < current {
                    return Err(CatalogError::RepairRefused(format!(
                        "refusing to lower counter {name} from {current} to {correct}"
                    )));
                }
            }
            let counter_id = match name.as_str() {
                "next_snapshot_id" => COUNTER_NEXT_SNAPSHOT_ID,
                "next_catalog_id" => COUNTER_NEXT_CATALOG_ID,
                "next_file_id" => COUNTER_NEXT_FILE_ID,
                _ => return Err(CatalogError::NotFound(format!("counter {name}"))),
            };
            let key = keys::key_counter(counter_id);
            batch.put(key, values::encode_counter(*correct));
        }
        RepairAction::RemoveOrphanedInlinedRow { key_hex } => {
            batch.delete(decode_hex(key_hex)?);
        }
        RepairAction::RemoveDanglingStats { key_hex } => {
            batch.delete(decode_hex(key_hex)?);
        }
        RepairAction::RebuildTableIndex {
            table_id,
            schema_id,
        } => {
            batch.put(
                keys::key_table_by_id(*table_id),
                values::encode_counter(*schema_id),
            );
        }
        RepairAction::RebuildDataFileIndex {
            table_id,
            data_file_id,
            begin_snapshot,
            value,
        } => {
            batch.put(
                keys::key_data_file_by_snapshot(*table_id, *begin_snapshot, *data_file_id),
                value,
            );
        }
    }
    Ok(())
}

async fn check_counter_consistency(
    db: &Db,
    actions: &mut Vec<RepairAction>,
    _unrecoverable: &mut Vec<String>,
) -> CatalogResult<()> {
    // Find max snapshot_id in use
    let mut max_snapshot = 0u64;
    let prefix = keys::prefix_for_tag(TAG_SNAPSHOT);
    let mut iter = db.scan_prefix(&prefix).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let row: SnapshotRow = values::decode_value(&kv.value)?;
        if row.snapshot_id > max_snapshot {
            max_snapshot = row.snapshot_id;
        }
    }

    let counter_key = keys::key_counter(COUNTER_NEXT_SNAPSHOT_ID);
    if let Some(data) = db.get(&counter_key).await? {
        let current = values::decode_counter(&data)?;
        let correct = max_snapshot.saturating_add(1);
        if current < correct {
            actions.push(RepairAction::FixCounter {
                name: "next_snapshot_id".to_string(),
                current,
                correct,
            });
        }
    }

    // Find max catalog_id in use
    let mut max_catalog_id = 0u64;
    for tag in [
        TAG_SCHEMA,
        TAG_TABLE,
        TAG_COLUMN,
        TAG_VIEW,
        TAG_MACRO,
        TAG_MACRO_IMPL,
        TAG_COLUMN_MAPPING,
        TAG_PARTITION_INFO,
        TAG_SORT_INFO,
    ] {
        let prefix = keys::prefix_for_tag(tag);
        let mut iter = db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            // Extract ID from value (first field is usually the ID)
            match tag {
                TAG_SCHEMA => {
                    let row: SchemaRow = values::decode_value(&kv.value)?;
                    if row.schema_id > max_catalog_id {
                        max_catalog_id = row.schema_id;
                    }
                }
                TAG_TABLE => {
                    let row: TableRow = values::decode_value(&kv.value)?;
                    if row.table_id > max_catalog_id {
                        max_catalog_id = row.table_id;
                    }
                }
                TAG_COLUMN => {
                    let row: ColumnRow = values::decode_value(&kv.value)?;
                    if row.column_id > max_catalog_id {
                        max_catalog_id = row.column_id;
                    }
                }
                TAG_VIEW => {
                    let row: ViewRow = values::decode_value(&kv.value)?;
                    max_catalog_id = max_catalog_id.max(row.view_id);
                }
                TAG_MACRO => {
                    let row: MacroRow = values::decode_value(&kv.value)?;
                    max_catalog_id = max_catalog_id.max(row.macro_id);
                }
                TAG_MACRO_IMPL => {
                    let row: MacroImplRow = values::decode_value(&kv.value)?;
                    max_catalog_id = max_catalog_id.max(row.impl_id);
                }
                TAG_COLUMN_MAPPING => {
                    let row: ColumnMappingRow = values::decode_value(&kv.value)?;
                    max_catalog_id = max_catalog_id.max(row.mapping_id);
                }
                TAG_PARTITION_INFO => {
                    let row: PartitionInfoRow = values::decode_value(&kv.value)?;
                    max_catalog_id = max_catalog_id.max(row.partition_id);
                }
                TAG_SORT_INFO => {
                    let row: SortInfoRow = values::decode_value(&kv.value)?;
                    max_catalog_id = max_catalog_id.max(row.sort_id);
                }
                _ => {}
            }
        }
    }

    let counter_key = keys::key_counter(COUNTER_NEXT_CATALOG_ID);
    if let Some(data) = db.get(&counter_key).await? {
        let current = values::decode_counter(&data)?;
        let correct = max_catalog_id.saturating_add(1);
        if current < correct {
            actions.push(RepairAction::FixCounter {
                name: "next_catalog_id".to_string(),
                current,
                correct,
            });
        }
    }

    // Find max file_id in use
    let mut max_file_id = 0u64;
    for tag in [
        TAG_DATA_FILE,
        TAG_DELETE_FILE,
        TAG_FILES_SCHEDULED_FOR_DELETION,
    ] {
        let prefix = keys::prefix_for_tag(tag);
        let mut iter = db.scan_prefix(&prefix).await?;
        while let Some(kv) = iter
            .next()
            .await
            .map_err(|e| CatalogError::SlateDb(e.to_string()))?
        {
            let id = match tag {
                TAG_DATA_FILE => values::decode_value::<DataFileRow>(&kv.value)?.data_file_id,
                TAG_DELETE_FILE => values::decode_value::<DeleteFileRow>(&kv.value)?.delete_file_id,
                TAG_FILES_SCHEDULED_FOR_DELETION => {
                    values::decode_value::<FilesScheduledForDeletionRow>(&kv.value)?.data_file_id
                }
                _ => 0,
            };
            max_file_id = max_file_id.max(id);
        }
    }

    let counter_key = keys::key_counter(COUNTER_NEXT_FILE_ID);
    if let Some(data) = db.get(&counter_key).await? {
        let current = values::decode_counter(&data)?;
        let correct = max_file_id.saturating_add(1);
        if current < correct {
            actions.push(RepairAction::FixCounter {
                name: "next_file_id".to_string(),
                current,
                correct,
            });
        }
    }

    Ok(())
}

async fn check_orphaned_inlined_rows(
    db: &Db,
    actions: &mut Vec<RepairAction>,
) -> CatalogResult<()> {
    // Collect valid table IDs
    let mut valid_tables = std::collections::HashSet::new();
    let prefix = keys::prefix_for_tag(TAG_TABLE);
    let mut iter = db.scan_prefix(&prefix).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let row: TableRow = values::decode_value(&kv.value)?;
        valid_tables.insert(row.table_id);
    }

    // Check inlined inserts
    let prefix = vec![TAG_INLINED_ROWS, INLINED_SUBTYPE_INSERT];
    let mut iter = db.scan_prefix(&prefix).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let row: InlinedInsertRow = values::decode_value(&kv.value)?;
        if !valid_tables.contains(&row.table_id) {
            actions.push(RepairAction::RemoveOrphanedInlinedRow {
                key_hex: bytes_to_hex(&kv.key),
            });
        }
    }

    Ok(())
}

async fn check_dangling_stats(db: &Db, actions: &mut Vec<RepairAction>) -> CatalogResult<()> {
    // Collect valid table IDs
    let mut valid_tables = std::collections::HashSet::new();
    let prefix = keys::prefix_for_tag(TAG_TABLE);
    let mut iter = db.scan_prefix(&prefix).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let row: TableRow = values::decode_value(&kv.value)?;
        valid_tables.insert(row.table_id);
    }

    // Check table stats
    let prefix = keys::prefix_for_tag(TAG_TABLE_STATS);
    let mut iter = db.scan_prefix(&prefix).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let row: TableStatsRow = values::decode_value(&kv.value)?;
        if !valid_tables.contains(&row.table_id) {
            actions.push(RepairAction::RemoveDanglingStats {
                key_hex: bytes_to_hex(&kv.key),
            });
        }
    }

    Ok(())
}

async fn check_index_consistency(db: &Db, actions: &mut Vec<RepairAction>) -> CatalogResult<()> {
    let mut iter = db.scan_prefix(&keys::prefix_for_tag(TAG_TABLE)).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let row: TableRow = values::decode_value(&kv.value)?;
        let key = keys::key_table_by_id(row.table_id);
        let expected = values::encode_counter(row.schema_id);
        if db.get(&key).await?.as_deref() != Some(expected.as_slice()) {
            actions.push(RepairAction::RebuildTableIndex {
                table_id: row.table_id,
                schema_id: row.schema_id,
            });
        }
    }

    let mut iter = db.scan_prefix(&keys::prefix_for_tag(TAG_DATA_FILE)).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let row: DataFileRow = values::decode_value(&kv.value)?;
        let begin_snapshot = row.begin_snapshot.unwrap_or(0);
        let index_key =
            keys::key_data_file_by_snapshot(row.table_id, begin_snapshot, row.data_file_id);
        if db.get(&index_key).await?.as_deref() != Some(kv.value.as_ref()) {
            actions.push(RepairAction::RebuildDataFileIndex {
                table_id: row.table_id,
                data_file_id: row.data_file_id,
                begin_snapshot,
                value: kv.value.to_vec(),
            });
        }
    }
    Ok(())
}

async fn check_critical_integrity(db: &Db, unrecoverable: &mut Vec<String>) -> CatalogResult<()> {
    // Check catalog format version exists
    let key = keys::key_system(SYSTEM_CATALOG_FORMAT_VERSION);
    if db.get(&key).await?.is_none() {
        unrecoverable.push("missing catalog-format-version key".to_string());
    }

    // Check at least one snapshot exists
    let prefix = keys::prefix_for_tag(TAG_SNAPSHOT);
    let mut iter = db.scan_prefix(&prefix).await?;
    let first = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?;
    if first.is_none() {
        unrecoverable.push("no snapshots found in catalog".to_string());
    }

    Ok(())
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn decode_hex(hex: &str) -> CatalogResult<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return Err(CatalogError::RepairRefused(
            "repair key has an odd number of hex digits".to_string(),
        ));
    }
    (0..hex.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&hex[index..index + 2], 16).map_err(|_| {
                CatalogError::RepairRefused(format!(
                    "repair key contains invalid hex at byte {index}"
                ))
            })
        })
        .collect()
}
