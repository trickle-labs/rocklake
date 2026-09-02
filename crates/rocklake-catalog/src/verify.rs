//! Full catalog integrity verification.

#![allow(missing_docs)]

use std::collections::{HashMap, HashSet};
use std::ops::RangeFull;

use prost::Message;
use rocklake_core::keys;
use rocklake_core::rows::*;
use rocklake_core::tags::*;
use rocklake_core::values;
use slatedb::Db;

use crate::error::{CatalogError, CatalogResult};

/// Result of catalog verification.
#[derive(Debug, Default)]
pub struct VerifyResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub tables_checked: u32,
    pub rows_checked: u64,
}

impl VerifyResult {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug, Clone)]
struct VersionedName {
    id: u64,
    owner: u64,
    name: String,
    begin: u64,
    end: Option<u64>,
}

/// Verify every supported catalog category, relationship, index, counter, and
/// MVCC interval. Findings are returned together as one operator report.
pub async fn verify_catalog(db: &Db) -> CatalogResult<VerifyResult> {
    let mut result = VerifyResult::default();
    verify_keys(db, &mut result).await?;
    verify_format_version(db, &mut result).await?;

    let metadata = scan_rows::<MetadataRow>(db, TAG_METADATA, &mut result).await?;
    let snapshots = scan_rows::<SnapshotRow>(db, TAG_SNAPSHOT, &mut result).await?;
    let snapshot_changes =
        scan_rows::<SnapshotChangesRow>(db, TAG_SNAPSHOT_CHANGES, &mut result).await?;
    let latest = snapshots
        .iter()
        .map(|(_, row)| row.snapshot_id)
        .max()
        .unwrap_or(0);
    if snapshots.is_empty() {
        result
            .warnings
            .push("no snapshots found in catalog (empty catalog)".to_string());
    }
    let snapshot_ids: HashSet<u64> = snapshots.iter().map(|(_, row)| row.snapshot_id).collect();
    let mut previous = None;
    for (_, row) in &snapshots {
        if let Some(prev) = previous {
            if row.snapshot_id <= prev {
                result.errors.push(format!(
                    "snapshot ordering violation: {} follows {}",
                    row.snapshot_id, prev
                ));
            }
        }
        previous = Some(row.snapshot_id);
    }

    let schemas = scan_rows::<SchemaRow>(db, TAG_SCHEMA, &mut result).await?;
    let tables = scan_rows::<TableRow>(db, TAG_TABLE, &mut result).await?;
    let columns = scan_rows::<ColumnRow>(db, TAG_COLUMN, &mut result).await?;
    let views = scan_rows::<ViewRow>(db, TAG_VIEW, &mut result).await?;
    let macros = scan_rows::<MacroRow>(db, TAG_MACRO, &mut result).await?;
    let macro_impls = scan_rows::<MacroImplRow>(db, TAG_MACRO_IMPL, &mut result).await?;
    let macro_parameters =
        scan_rows::<MacroParametersRow>(db, TAG_MACRO_PARAMETERS, &mut result).await?;
    let data_files = scan_rows::<DataFileRow>(db, TAG_DATA_FILE, &mut result).await?;
    let delete_files = scan_rows::<DeleteFileRow>(db, TAG_DELETE_FILE, &mut result).await?;
    let scheduled = scan_rows::<FilesScheduledForDeletionRow>(
        db,
        TAG_FILES_SCHEDULED_FOR_DELETION,
        &mut result,
    )
    .await?;
    let inlined_tables =
        scan_rows::<InlinedDataTablesRow>(db, TAG_INLINED_DATA_TABLES, &mut result).await?;
    let column_mappings =
        scan_rows::<ColumnMappingRow>(db, TAG_COLUMN_MAPPING, &mut result).await?;
    let name_mappings = scan_rows::<NameMappingRow>(db, TAG_NAME_MAPPING, &mut result).await?;
    let table_stats = scan_rows::<TableStatsRow>(db, TAG_TABLE_STATS, &mut result).await?;
    let table_column_stats =
        scan_rows::<TableColumnStatsRow>(db, TAG_TABLE_COLUMN_STATS, &mut result).await?;
    let file_column_stats =
        scan_rows::<FileColumnStatsRow>(db, TAG_FILE_COLUMN_STATS, &mut result).await?;
    let file_variant_stats =
        scan_rows::<FileVariantStatsRow>(db, TAG_FILE_VARIANT_STATS, &mut result).await?;
    let partition_info = scan_rows::<PartitionInfoRow>(db, TAG_PARTITION_INFO, &mut result).await?;
    let partition_columns =
        scan_rows::<PartitionColumnRow>(db, TAG_PARTITION_COLUMN, &mut result).await?;
    let file_partition_values =
        scan_rows::<FilePartitionValueRow>(db, TAG_FILE_PARTITION_VALUE, &mut result).await?;
    let sort_info = scan_rows::<SortInfoRow>(db, TAG_SORT_INFO, &mut result).await?;
    let sort_expressions =
        scan_rows::<SortExpressionRow>(db, TAG_SORT_EXPRESSION, &mut result).await?;
    let tags = scan_rows::<TagRow>(db, TAG_TAG, &mut result).await?;
    let column_tags = scan_rows::<ColumnTagRow>(db, TAG_COLUMN_TAG, &mut result).await?;
    let schema_versions =
        scan_rows::<SchemaVersionsRow>(db, TAG_SCHEMA_VERSIONS, &mut result).await?;
    let leases = scan_rows::<SnapshotLeaseRow>(db, TAG_SNAPSHOT_LEASE, &mut result).await?;
    let extensions = scan_rows::<ExtensionSchemaRow>(db, TAG_EXTENSION_SCHEMA, &mut result).await?;
    let _encrypted_secrets =
        scan_rows::<EncryptedSecretRow>(db, TAG_ENCRYPTED_SECRET, &mut result).await?;
    let _encryption_keys =
        scan_rows::<EncryptionKeyRow>(db, TAG_ENCRYPTION_KEY, &mut result).await?;
    let inline_inserts = scan_rows_prefix::<InlinedInsertRow>(
        db,
        &[TAG_INLINED_ROWS, INLINED_SUBTYPE_INSERT],
        &mut result,
    )
    .await?;
    let inline_deletes = scan_rows_prefix::<InlinedDeleteRow>(
        db,
        &[TAG_INLINED_ROWS, INLINED_SUBTYPE_DELETE],
        &mut result,
    )
    .await?;

    let schema_ids: HashSet<u64> = schemas.iter().map(|(_, row)| row.schema_id).collect();
    let table_ids: HashSet<u64> = tables.iter().map(|(_, row)| row.table_id).collect();
    let column_ids: HashSet<u64> = columns.iter().map(|(_, row)| row.column_id).collect();
    let macro_ids: HashSet<u64> = macros.iter().map(|(_, row)| row.macro_id).collect();
    let impl_ids: HashSet<u64> = macro_impls.iter().map(|(_, row)| row.impl_id).collect();
    let file_ids: HashSet<u64> = data_files.iter().map(|(_, row)| row.data_file_id).collect();
    let partition_ids: HashSet<u64> = partition_info
        .iter()
        .map(|(_, row)| row.partition_id)
        .collect();
    let sort_ids: HashSet<u64> = sort_info.iter().map(|(_, row)| row.sort_id).collect();
    let mapping_ids: HashSet<u64> = column_mappings
        .iter()
        .map(|(_, row)| row.mapping_id)
        .collect();
    let object_ids: HashSet<u64> = schema_ids
        .iter()
        .chain(table_ids.iter())
        .chain(column_ids.iter())
        .chain(views.iter().map(|(_, row)| &row.view_id))
        .chain(macro_ids.iter())
        .chain(impl_ids.iter())
        .chain(mapping_ids.iter())
        .chain(partition_ids.iter())
        .chain(sort_ids.iter())
        .copied()
        .collect();

    verify_mvcc(&mut result, latest, &schemas, "schema", |row| {
        (row.schema_id, row.begin_snapshot, row.end_snapshot)
    });
    verify_mvcc(&mut result, latest, &tables, "table", |row| {
        (row.table_id, row.begin_snapshot, row.end_snapshot)
    });
    verify_mvcc(&mut result, latest, &columns, "column", |row| {
        (row.column_id, row.begin_snapshot, row.end_snapshot)
    });
    verify_mvcc(&mut result, latest, &views, "view", |row| {
        (row.view_id, row.begin_snapshot, row.end_snapshot)
    });
    verify_mvcc(&mut result, latest, &macros, "macro", |row| {
        (row.macro_id, row.begin_snapshot, row.end_snapshot)
    });
    verify_mvcc(&mut result, latest, &partition_info, "partition", |row| {
        (row.partition_id, row.begin_snapshot, row.end_snapshot)
    });
    verify_mvcc(&mut result, latest, &sort_info, "sort", |row| {
        (row.sort_id, row.begin_snapshot, row.end_snapshot)
    });
    verify_mvcc(&mut result, latest, &tags, "tag", |row| {
        (row.object_id, row.begin_snapshot, row.end_snapshot)
    });
    verify_mvcc(&mut result, latest, &column_tags, "column tag", |row| {
        (row.column_id, row.begin_snapshot, row.end_snapshot)
    });
    verify_mvcc(
        &mut result,
        latest,
        &inline_inserts,
        "inlined insert",
        |row| (row.row_id, row.begin_snapshot, row.end_snapshot),
    );
    verify_mvcc(
        &mut result,
        latest,
        &inline_deletes,
        "inlined delete",
        |row| (row.row_id, row.begin_snapshot, row.end_snapshot),
    );

    verify_duplicate_names(&mut result, &schemas, &tables, &columns, &views, &macros);
    verify_foreign_keys(
        &mut result,
        &schema_ids,
        &table_ids,
        &column_ids,
        &macro_ids,
        &impl_ids,
        &file_ids,
        &partition_ids,
        &sort_ids,
        &mapping_ids,
        &snapshot_ids,
        &object_ids,
        &tables,
        &columns,
        &views,
        &macros,
        &macro_impls,
        &macro_parameters,
        &data_files,
        &delete_files,
        &scheduled,
        &inlined_tables,
        &column_mappings,
        &name_mappings,
        &table_stats,
        &table_column_stats,
        &file_column_stats,
        &file_variant_stats,
        &partition_info,
        &partition_columns,
        &file_partition_values,
        &sort_info,
        &sort_expressions,
        &tags,
        &column_tags,
        &schema_versions,
        &inline_inserts,
        &inline_deletes,
        &snapshot_changes,
    );
    let _ = metadata;
    for (_, lease) in &leases {
        if latest > 0 && lease.min_snapshot_id > latest {
            result.errors.push(format!(
                "snapshot lease '{}' pins future snapshot {}",
                lease.consumer_id, lease.min_snapshot_id
            ));
        }
    }
    verify_paths(&mut result, &schemas, &tables, &data_files, &delete_files);
    verify_extensions(&mut result, &extensions);
    verify_indexes(
        db,
        &mut result,
        &tables,
        &data_files,
        &delete_files,
        &file_column_stats,
    )
    .await?;
    verify_counters(
        db,
        &mut result,
        latest,
        &snapshots,
        &schemas,
        &tables,
        &columns,
        &views,
        &macros,
        &macro_impls,
        &column_mappings,
        &partition_info,
        &sort_info,
        &data_files,
        &delete_files,
        &scheduled,
        &table_stats,
        &file_column_stats,
        &file_variant_stats,
        &inline_inserts,
        &inline_deletes,
    )
    .await?;
    Ok(result)
}

async fn scan_rows<M: Message + Default>(
    db: &Db,
    tag: u8,
    result: &mut VerifyResult,
) -> CatalogResult<Vec<(Vec<u8>, M)>> {
    scan_rows_prefix(db, &[tag], result).await
}

async fn scan_rows_prefix<M: Message + Default>(
    db: &Db,
    prefix: &[u8],
    result: &mut VerifyResult,
) -> CatalogResult<Vec<(Vec<u8>, M)>> {
    result.tables_checked += 1;
    let mut rows = Vec::new();
    let mut iter = db.scan_prefix(prefix).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        match values::decode_value(&kv.value) {
            Ok(row) => {
                result.rows_checked += 1;
                rows.push((kv.key.to_vec(), row));
            }
            Err(error) => result.errors.push(format!(
                "tag 0x{:02X} key {} has invalid value: {error}",
                prefix[0],
                hex(&kv.key)
            )),
        }
    }
    Ok(rows)
}

async fn verify_keys(db: &Db, result: &mut VerifyResult) -> CatalogResult<()> {
    let mut iter = db.scan::<&[u8], _>(RangeFull).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        let key = kv.key.as_ref();
        let Some(&tag) = key.first() else {
            result.errors.push("empty catalog key".to_string());
            continue;
        };
        if !is_known_tag(tag) && tag != TAG_DATA_FILE_BY_SNAPSHOT {
            result
                .errors
                .push(format!("unknown catalog key tag 0x{tag:02X}"));
            continue;
        }
        let minimum = match tag {
            TAG_METADATA => 12,
            TAG_SNAPSHOT | TAG_SNAPSHOT_CHANGES | TAG_SCHEMA | TAG_TABLE_STATS
            | TAG_ENCRYPTED_SECRET => 9,
            TAG_TABLE | TAG_COLUMN | TAG_VIEW | TAG_MACRO | TAG_PARTITION_INFO | TAG_SORT_INFO
            | TAG_ENCRYPTION_KEY => 17,
            TAG_MACRO_IMPL
            | TAG_MACRO_PARAMETERS
            | TAG_DATA_FILE
            | TAG_DELETE_FILE
            | TAG_FILES_SCHEDULED_FOR_DELETION
            | TAG_INLINED_DATA_TABLES
            | TAG_COLUMN_MAPPING
            | TAG_TABLE_COLUMN_STATS
            | TAG_PARTITION_COLUMN
            | TAG_SORT_EXPRESSION
            | TAG_SCHEMA_VERSIONS => 17,
            TAG_NAME_MAPPING | TAG_FILE_COLUMN_STATS | TAG_FILE_PARTITION_VALUE => 25,
            TAG_FILE_COLUMN_STATS_BY_SNAPSHOT => 33,
            TAG_DELETE_FILE_BY_TABLE | TAG_DATA_FILE_BY_ORDER => 25,
            TAG_FILE_VARIANT_STATS | TAG_COLUMN_TAG => 33,
            TAG_TAG => 25,
            TAG_DATA_FILE_BY_SNAPSHOT => 25,
            TAG_SNAPSHOT_LEASE => 4,
            TAG_EXTENSION_SCHEMA => 12,
            TAG_INLINED_ROWS => 26,
            TAG_COUNTERS => 2,
            TAG_TABLE_BY_ID => 9,
            TAG_SYSTEM => 2,
            _ => 1,
        };
        if key.len() < minimum {
            result.errors.push(format!(
                "catalog key for tag 0x{tag:02X} is too short: {} bytes (minimum {minimum})",
                key.len()
            ));
        }
        if tag == TAG_COUNTERS && key.len() != 2 && key.len() != 10 {
            result
                .errors
                .push(format!("invalid counter key length {}", key.len()));
        }
        if tag == TAG_TABLE_BY_ID && key.len() != 9 && key.len() != 25 {
            result
                .errors
                .push(format!("invalid 0xFC index key length {}", key.len()));
        }
    }
    Ok(())
}

async fn verify_format_version(db: &Db, result: &mut VerifyResult) -> CatalogResult<()> {
    let key = keys::key_system(SYSTEM_CATALOG_FORMAT_VERSION);
    match db.get(&key).await? {
        None => result
            .errors
            .push("missing catalog-format-version".to_string()),
        Some(data) => match values::decode_format_version(&data) {
            Ok(version) if version == CATALOG_FORMAT_VERSION => {}
            Ok(version) => result.errors.push(format!(
                "format version mismatch: expected {}, got {version}",
                CATALOG_FORMAT_VERSION
            )),
            Err(error) => result
                .errors
                .push(format!("invalid catalog-format-version: {error}")),
        },
    }
    Ok(())
}

fn verify_mvcc<M, F>(
    result: &mut VerifyResult,
    latest: u64,
    rows: &[(Vec<u8>, M)],
    label: &str,
    fields: F,
) where
    F: Fn(&M) -> (u64, u64, Option<u64>),
{
    for (_, row) in rows {
        let (id, begin, end) = fields(row);
        if begin > latest && latest > 0 {
            result.errors.push(format!(
                "{label} {id}: begin_snapshot {begin} is after latest snapshot {latest}"
            ));
        }
        if let Some(end) = end {
            if end <= begin {
                result.errors.push(format!(
                    "{label} {id}: end_snapshot ({end}) <= begin_snapshot ({begin})"
                ));
            }
        }
    }
}

fn verify_duplicate_names(
    result: &mut VerifyResult,
    schemas: &[(Vec<u8>, SchemaRow)],
    tables: &[(Vec<u8>, TableRow)],
    columns: &[(Vec<u8>, ColumnRow)],
    views: &[(Vec<u8>, ViewRow)],
    macros: &[(Vec<u8>, MacroRow)],
) {
    let schema_names = schemas
        .iter()
        .map(|(_, row)| VersionedName {
            id: row.schema_id,
            owner: 0,
            name: row.schema_name.clone(),
            begin: row.begin_snapshot,
            end: row.end_snapshot,
        })
        .collect::<Vec<_>>();
    let table_names = tables
        .iter()
        .map(|(_, row)| VersionedName {
            id: row.table_id,
            owner: row.schema_id,
            name: row.table_name.clone(),
            begin: row.begin_snapshot,
            end: row.end_snapshot,
        })
        .collect::<Vec<_>>();
    let column_names = columns
        .iter()
        .map(|(_, row)| VersionedName {
            id: row.column_id,
            owner: row.table_id,
            name: row.column_name.clone(),
            begin: row.begin_snapshot,
            end: row.end_snapshot,
        })
        .collect::<Vec<_>>();
    let view_names = views
        .iter()
        .map(|(_, row)| VersionedName {
            id: row.view_id,
            owner: row.schema_id,
            name: row.view_name.clone(),
            begin: row.begin_snapshot,
            end: row.end_snapshot,
        })
        .collect::<Vec<_>>();
    let macro_names = macros
        .iter()
        .map(|(_, row)| VersionedName {
            id: row.macro_id,
            owner: row.schema_id,
            name: row.macro_name.clone(),
            begin: row.begin_snapshot,
            end: row.end_snapshot,
        })
        .collect::<Vec<_>>();
    verify_name_set(result, "schema", &schema_names);
    verify_name_set(result, "table", &table_names);
    verify_name_set(result, "column", &column_names);
    verify_name_set(result, "view", &view_names);
    verify_name_set(result, "macro", &macro_names);
}

fn verify_name_set(result: &mut VerifyResult, label: &str, rows: &[VersionedName]) {
    // ponytail: O(n²) is intentional; catalogs are metadata-sized and this
    // keeps interval overlap detection exact without another index structure.
    for (index, left) in rows.iter().enumerate() {
        for right in rows.iter().skip(index + 1) {
            if left.owner != right.owner || left.name != right.name {
                continue;
            }
            let left_end = left.end.unwrap_or(u64::MAX);
            let right_end = right.end.unwrap_or(u64::MAX);
            if left.begin < right_end && right.begin < left_end {
                result.errors.push(format!(
                    "overlapping live {label} name '{}' (ids {} and {})",
                    left.name, left.id, right.id
                ));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_foreign_keys(
    result: &mut VerifyResult,
    schema_ids: &HashSet<u64>,
    table_ids: &HashSet<u64>,
    column_ids: &HashSet<u64>,
    macro_ids: &HashSet<u64>,
    impl_ids: &HashSet<u64>,
    file_ids: &HashSet<u64>,
    partition_ids: &HashSet<u64>,
    sort_ids: &HashSet<u64>,
    mapping_ids: &HashSet<u64>,
    snapshot_ids: &HashSet<u64>,
    object_ids: &HashSet<u64>,
    tables: &[(Vec<u8>, TableRow)],
    columns: &[(Vec<u8>, ColumnRow)],
    views: &[(Vec<u8>, ViewRow)],
    macros: &[(Vec<u8>, MacroRow)],
    macro_impls: &[(Vec<u8>, MacroImplRow)],
    macro_parameters: &[(Vec<u8>, MacroParametersRow)],
    data_files: &[(Vec<u8>, DataFileRow)],
    delete_files: &[(Vec<u8>, DeleteFileRow)],
    scheduled: &[(Vec<u8>, FilesScheduledForDeletionRow)],
    inlined_tables: &[(Vec<u8>, InlinedDataTablesRow)],
    column_mappings: &[(Vec<u8>, ColumnMappingRow)],
    name_mappings: &[(Vec<u8>, NameMappingRow)],
    table_stats: &[(Vec<u8>, TableStatsRow)],
    table_column_stats: &[(Vec<u8>, TableColumnStatsRow)],
    file_column_stats: &[(Vec<u8>, FileColumnStatsRow)],
    file_variant_stats: &[(Vec<u8>, FileVariantStatsRow)],
    partition_info: &[(Vec<u8>, PartitionInfoRow)],
    partition_columns: &[(Vec<u8>, PartitionColumnRow)],
    file_partition_values: &[(Vec<u8>, FilePartitionValueRow)],
    sort_info: &[(Vec<u8>, SortInfoRow)],
    sort_expressions: &[(Vec<u8>, SortExpressionRow)],
    tags: &[(Vec<u8>, TagRow)],
    column_tags: &[(Vec<u8>, ColumnTagRow)],
    schema_versions: &[(Vec<u8>, SchemaVersionsRow)],
    inline_inserts: &[(Vec<u8>, InlinedInsertRow)],
    inline_deletes: &[(Vec<u8>, InlinedDeleteRow)],
    snapshot_changes: &[(Vec<u8>, SnapshotChangesRow)],
) {
    for (_, row) in tables {
        require(result, schema_ids, row.schema_id, "table", "schema");
    }
    for (_, row) in columns {
        require(result, table_ids, row.table_id, "column", "table");
        if let Some(parent) = row.parent_column {
            require(result, column_ids, parent, "column", "parent column");
        }
    }
    for (_, row) in views {
        require(result, schema_ids, row.schema_id, "view", "schema");
    }
    for (_, row) in macros {
        require(result, schema_ids, row.schema_id, "macro", "schema");
    }
    for (_, row) in macro_impls {
        require(
            result,
            macro_ids,
            row.macro_id,
            "macro implementation",
            "macro",
        );
    }
    for (_, row) in macro_parameters {
        require(result, macro_ids, row.macro_id, "macro parameter", "macro");
        require(
            result,
            impl_ids,
            row.impl_id,
            "macro parameter",
            "implementation",
        );
        // DuckLake uses this ID as a parameter slot/type descriptor; it need
        // not resolve to a catalog column.
    }
    for (_, row) in data_files {
        require(result, table_ids, row.table_id, "data file", "table");
    }
    for (_, row) in delete_files {
        require(
            result,
            file_ids,
            row.data_file_id,
            "delete file",
            "data file",
        );
        if let Some(table_id) = row.table_id {
            require(result, table_ids, table_id, "delete file", "table");
        }
    }
    for (_, row) in scheduled {
        require(
            result,
            file_ids,
            row.data_file_id,
            "scheduled deletion",
            "data file",
        );
    }
    for (_, row) in inlined_tables {
        require(
            result,
            table_ids,
            row.table_id,
            "inlined data table",
            "table",
        );
    }
    for (_, row) in column_mappings {
        require(result, table_ids, row.table_id, "column mapping", "table");
        if let Some(column_id) = row.column_id {
            require(result, column_ids, column_id, "column mapping", "column");
        }
    }
    for (_, row) in name_mappings {
        require(
            result,
            mapping_ids,
            row.mapping_id,
            "name mapping",
            "mapping",
        );
        require(result, column_ids, row.column_id, "name mapping", "column");
        if let Some(parent) = row.parent_column {
            require(result, column_ids, parent, "name mapping", "parent column");
        }
    }
    for (_, row) in table_stats {
        require(result, table_ids, row.table_id, "table stats", "table");
    }
    for (_, row) in table_column_stats {
        require(
            result,
            table_ids,
            row.table_id,
            "table column stats",
            "table",
        );
        require(
            result,
            column_ids,
            row.column_id,
            "table column stats",
            "column",
        );
    }
    for (_, row) in file_column_stats {
        require(
            result,
            table_ids,
            row.table_id,
            "file column stats",
            "table",
        );
        require(
            result,
            column_ids,
            row.column_id,
            "file column stats",
            "column",
        );
        require(
            result,
            file_ids,
            row.data_file_id,
            "file column stats",
            "data file",
        );
    }
    for (_, row) in file_variant_stats {
        require(
            result,
            table_ids,
            row.table_id,
            "file variant stats",
            "table",
        );
        require(
            result,
            column_ids,
            row.column_id,
            "file variant stats",
            "column",
        );
        require(
            result,
            file_ids,
            row.data_file_id,
            "file variant stats",
            "data file",
        );
    }
    for (_, row) in partition_info {
        require(result, table_ids, row.table_id, "partition", "table");
    }
    for (_, row) in partition_columns {
        require(
            result,
            partition_ids,
            row.partition_id,
            "partition column",
            "partition",
        );
        require(
            result,
            column_ids,
            row.column_id,
            "partition column",
            "column",
        );
        if let Some(table_id) = row.table_id {
            require(result, table_ids, table_id, "partition column", "table");
        }
    }
    for (_, row) in file_partition_values {
        require(
            result,
            table_ids,
            row.table_id,
            "file partition value",
            "table",
        );
        require(
            result,
            file_ids,
            row.data_file_id,
            "file partition value",
            "data file",
        );
    }
    for (_, row) in sort_info {
        require(result, table_ids, row.table_id, "sort", "table");
    }
    for (_, row) in sort_expressions {
        require(result, sort_ids, row.sort_id, "sort expression", "sort");
        require(
            result,
            column_ids,
            row.column_id,
            "sort expression",
            "column",
        );
        if let Some(table_id) = row.table_id {
            require(result, table_ids, table_id, "sort expression", "table");
        }
    }
    for (_, row) in tags {
        if !object_ids.contains(&row.object_id) {
            result.errors.push(format!(
                "tag references missing catalog object {}",
                row.object_id
            ));
        }
    }
    for (_, row) in column_tags {
        require(result, table_ids, row.table_id, "column tag", "table");
        require(result, column_ids, row.column_id, "column tag", "column");
    }
    for (_, row) in schema_versions {
        require(result, table_ids, row.table_id, "schema version", "table");
    }
    for (_, row) in inline_inserts {
        require(result, table_ids, row.table_id, "inlined insert", "table");
    }
    for (_, row) in inline_deletes {
        require(result, table_ids, row.table_id, "inlined delete", "table");
        require(
            result,
            file_ids,
            row.data_file_id,
            "inlined delete",
            "data file",
        );
    }
    for (_, row) in snapshot_changes {
        require(
            result,
            snapshot_ids,
            row.snapshot_id,
            "snapshot change",
            "snapshot",
        );
        if let Some(schema_id) = row.schema_id {
            require(result, schema_ids, schema_id, "snapshot change", "schema");
        }
        if let Some(table_id) = row.table_id {
            require(result, table_ids, table_id, "snapshot change", "table");
        }
    }
}

fn require<T: std::hash::Hash + Eq + std::fmt::Display>(
    result: &mut VerifyResult,
    set: &HashSet<T>,
    id: T,
    row_kind: &str,
    target_kind: &str,
) {
    if !set.contains(&id) {
        result
            .errors
            .push(format!("{row_kind} references missing {target_kind} {id}"));
    }
}

fn verify_paths(
    result: &mut VerifyResult,
    schemas: &[(Vec<u8>, SchemaRow)],
    tables: &[(Vec<u8>, TableRow)],
    data_files: &[(Vec<u8>, DataFileRow)],
    delete_files: &[(Vec<u8>, DeleteFileRow)],
) {
    for (_, row) in schemas {
        check_path(
            result,
            "schema",
            row.schema_id,
            row.path.as_deref(),
            row.path_is_relative,
        );
    }
    for (_, row) in tables {
        check_path(
            result,
            "table",
            row.table_id,
            row.path.as_deref(),
            row.path_is_relative,
        );
    }
    for (_, row) in data_files {
        check_path(
            result,
            "data file",
            row.data_file_id,
            Some(&row.path),
            row.path_is_relative,
        );
    }
    for (_, row) in delete_files {
        check_path(
            result,
            "delete file",
            row.delete_file_id,
            Some(&row.path),
            row.path_is_relative,
        );
    }
}

fn check_path(
    result: &mut VerifyResult,
    kind: &str,
    id: u64,
    path: Option<&str>,
    relative: Option<bool>,
) {
    if path.is_some_and(str::is_empty) {
        result.errors.push(format!("{kind} {id} has an empty path"));
    }
    if let Some(path) = path {
        let inferred = rocklake_core::path::is_path_relative(path);
        if relative.is_some_and(|value| value != inferred) {
            result.errors.push(format!(
                "{kind} {id} disagrees with path_is_relative for path '{path}'"
            ));
        }
        if path.split('/').any(|part| part == "..") {
            result
                .errors
                .push(format!("{kind} {id} path contains traversal: '{path}'"));
        }
    }
    if relative == Some(true) && path.is_some_and(|path| path.starts_with('/')) {
        result.errors.push(format!(
            "{kind} {id} marks an absolute path as path_is_relative"
        ));
    }
}

fn verify_extensions(result: &mut VerifyResult, rows: &[(Vec<u8>, ExtensionSchemaRow)]) {
    for (_, row) in rows {
        if serde_json::from_str::<serde_json::Value>(&row.data_json).is_err() {
            result.errors.push(format!(
                "extension row {}:{} contains invalid JSON",
                row.extension_id, row.row_id
            ));
        }
    }
}

async fn verify_indexes(
    db: &Db,
    result: &mut VerifyResult,
    tables: &[(Vec<u8>, TableRow)],
    data_files: &[(Vec<u8>, DataFileRow)],
    delete_files: &[(Vec<u8>, DeleteFileRow)],
    file_column_stats: &[(Vec<u8>, FileColumnStatsRow)],
) -> CatalogResult<()> {
    let mut table_indexes = HashMap::new();
    let mut iter = db.scan_prefix(&[TAG_TABLE_BY_ID]).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        match kv.key.len() {
            9 => {
                let table_id = be_u64(&kv.key[1..9]);
                match values::decode_counter(&kv.value) {
                    Ok(schema_id) => {
                        table_indexes.insert(table_id, schema_id);
                        if !tables
                            .iter()
                            .any(|(_, row)| row.table_id == table_id && row.schema_id == schema_id)
                        {
                            result.errors.push(format!(
                                "table_by_id index for table {table_id} points to schema {schema_id}"
                            ));
                        }
                    }
                    Err(error) => result
                        .errors
                        .push(format!("invalid table_by_id index: {error}")),
                }
            }
            25 => {
                let snapshot_id = be_u64(&kv.key[1..9]);
                let table_id = be_u64(&kv.key[9..17]);
                let file_id = be_u64(&kv.key[17..25]);
                match values::decode_value::<SecondaryIndexEntry>(&kv.value) {
                    Ok(entry) => {
                        if entry.data_file_id != file_id {
                            result.errors.push(format!(
                                "secondary index {snapshot_id}/{table_id}/{file_id} contains file {}",
                                entry.data_file_id
                            ));
                        }
                        if let Some((_, file)) = data_files.iter().find(|(_, row)| {
                            row.table_id == table_id && row.data_file_id == file_id
                        }) {
                            if file.path != entry.path {
                                result.errors.push(format!(
                                    "secondary index for data file {file_id} has stale path"
                                ));
                            }
                        } else {
                            result.errors.push(format!(
                                "secondary index references missing data file {table_id}/{file_id}"
                            ));
                        }
                    }
                    Err(error) => result
                        .errors
                        .push(format!("invalid secondary index value: {error}")),
                }
            }
            length => result
                .errors
                .push(format!("invalid 0xFC index key length {length}")),
        }
    }
    for (_, row) in tables {
        if table_indexes.get(&row.table_id) != Some(&row.schema_id) {
            result.errors.push(format!(
                "missing or stale table_by_id index for table {}",
                row.table_id
            ));
        }
    }

    let mut iter = db.scan_prefix(&[TAG_DATA_FILE_BY_SNAPSHOT]).await?;
    let mut data_file_indexes = HashSet::new();
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        if kv.key.len() != 25 {
            result.errors.push(format!(
                "invalid data-file index key length {}",
                kv.key.len()
            ));
            continue;
        }
        let table_id = be_u64(&kv.key[1..9]);
        let file_id = be_u64(&kv.key[17..25]);
        if let Some((primary_key, file)) = data_files
            .iter()
            .find(|(_, row)| row.table_id == table_id && row.data_file_id == file_id)
        {
            let expected = keys::key_data_file(table_id, file_id);
            if db.get(&expected).await?.as_deref() != Some(kv.value.as_ref()) {
                result.errors.push(format!(
                    "data-file index for {table_id}/{file_id} does not match primary value"
                ));
            }
            if kv.key
                != keys::key_data_file_by_snapshot(
                    table_id,
                    file.begin_snapshot.unwrap_or(0),
                    file_id,
                )
            {
                result.errors.push(format!(
                    "data-file index for {table_id}/{file_id} has wrong snapshot key"
                ));
            }
            data_file_indexes.insert(primary_key.clone());
        } else {
            result.errors.push(format!(
                "data-file index references missing data file {table_id}/{file_id}"
            ));
        }
    }
    for (primary_key, row) in data_files {
        if !data_file_indexes.contains(primary_key) {
            result.errors.push(format!(
                "missing data-file index for {}/{}",
                row.table_id, row.data_file_id
            ));
        }
    }
    verify_data_file_order_index(db, result, data_files).await?;
    verify_delete_file_index(db, result, delete_files).await?;
    verify_file_column_stats_index(db, result, data_files, file_column_stats).await?;
    Ok(())
}

async fn verify_data_file_order_index(
    db: &Db,
    result: &mut VerifyResult,
    data_files: &[(Vec<u8>, DataFileRow)],
) -> CatalogResult<()> {
    let mut keys_seen = HashSet::new();
    let mut iter = db.scan_prefix(&[TAG_DATA_FILE_BY_ORDER]).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        if kv.key.len() != 25 {
            result.errors.push(format!(
                "invalid data-file order index key length {}",
                kv.key.len()
            ));
            continue;
        }
        let table_id = keys::decode_u64(&kv.key[1..9])?;
        let file_order = keys::decode_u64(&kv.key[9..17])?;
        let file_id = keys::decode_u64(&kv.key[17..25])?;
        let Some((primary_key, file)) = data_files
            .iter()
            .find(|(_, row)| row.table_id == table_id && row.data_file_id == file_id)
        else {
            result.errors.push(format!(
                "data-file order index references missing data file {table_id}/{file_id}"
            ));
            continue;
        };
        if file.file_order.unwrap_or(file_id) != file_order {
            result.errors.push(format!(
                "data-file order index for {table_id}/{file_id} has wrong file_order"
            ));
        }
        if values::decode_value::<DataFileRow>(&kv.value).ok().as_ref() != Some(file) {
            result.errors.push(format!(
                "data-file order index for {table_id}/{file_id} does not match primary value"
            ));
        }
        keys_seen.insert(primary_key.clone());
    }
    if keys_seen.is_empty() {
        return Ok(());
    }
    for (primary_key, row) in data_files {
        if !keys_seen.contains(primary_key) {
            result.errors.push(format!(
                "missing data-file order index for {}/{}",
                row.table_id, row.data_file_id
            ));
        }
    }
    Ok(())
}

async fn verify_delete_file_index(
    db: &Db,
    result: &mut VerifyResult,
    delete_files: &[(Vec<u8>, DeleteFileRow)],
) -> CatalogResult<()> {
    let mut ids_seen = HashSet::new();
    let mut iter = db.scan_prefix(&[TAG_DELETE_FILE_BY_TABLE]).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        if kv.key.len() != 25 {
            result.errors.push(format!(
                "invalid delete-file index key length {}",
                kv.key.len()
            ));
            continue;
        }
        let table_id = keys::decode_u64(&kv.key[1..9])?;
        let begin_snapshot = keys::decode_u64(&kv.key[9..17])?;
        let delete_file_id = keys::decode_u64(&kv.key[17..25])?;
        let Ok(index_row) = values::decode_value::<DeleteFileRow>(&kv.value) else {
            result.errors.push(format!(
                "invalid delete-file index value for {table_id}/{delete_file_id}"
            ));
            continue;
        };
        let Some((primary_key, row)) = delete_files
            .iter()
            .find(|(_, row)| row.delete_file_id == delete_file_id)
        else {
            result.errors.push(format!(
                "delete-file index references missing delete file {table_id}/{delete_file_id}"
            ));
            continue;
        };
        if index_row != *row || begin_snapshot != row.begin_snapshot.unwrap_or(row.snapshot_id) {
            result.errors.push(format!(
                "delete-file index for {table_id}/{delete_file_id} does not match primary value"
            ));
        }
        ids_seen.insert(primary_key.clone());
    }
    if ids_seen.is_empty() {
        return Ok(());
    }
    for (primary_key, row) in delete_files {
        if !ids_seen.contains(primary_key) && row.table_id.is_some() {
            result.errors.push(format!(
                "missing delete-file index for {}",
                row.delete_file_id
            ));
        }
    }
    Ok(())
}

async fn verify_file_column_stats_index(
    db: &Db,
    result: &mut VerifyResult,
    data_files: &[(Vec<u8>, DataFileRow)],
    file_column_stats: &[(Vec<u8>, FileColumnStatsRow)],
) -> CatalogResult<()> {
    let mut stats_seen = HashSet::new();
    let mut iter = db.scan_prefix(&[TAG_FILE_COLUMN_STATS_BY_SNAPSHOT]).await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        if kv.key.len() != 33 {
            result.errors.push(format!(
                "invalid file-column-stats index key length {}",
                kv.key.len()
            ));
            continue;
        }
        let table_id = keys::decode_u64(&kv.key[1..9])?;
        let column_id = keys::decode_u64(&kv.key[9..17])?;
        let begin_snapshot = keys::decode_u64(&kv.key[17..25])?;
        let data_file_id = keys::decode_u64(&kv.key[25..33])?;
        let Ok(index_row) = values::decode_value::<FileColumnStatsRow>(&kv.value) else {
            result.errors.push(format!(
                "invalid file-column-stats index value for {table_id}/{column_id}/{data_file_id}"
            ));
            continue;
        };
        let Some((primary_key, row)) = file_column_stats.iter().find(|(_, row)| {
            row.table_id == table_id
                && row.column_id == column_id
                && row.data_file_id == data_file_id
        }) else {
            result.errors.push(format!(
                "file-column-stats index references missing stats {table_id}/{column_id}/{data_file_id}"
            ));
            continue;
        };
        let expected_begin = data_files
            .iter()
            .find(|(_, file)| file.table_id == table_id && file.data_file_id == data_file_id)
            .map(|(_, file)| file.begin_snapshot.unwrap_or(0));
        if index_row != *row || expected_begin != Some(begin_snapshot) {
            result.errors.push(format!(
                "file-column-stats index for {table_id}/{column_id}/{data_file_id} does not match primary value"
            ));
        }
        stats_seen.insert(primary_key.clone());
    }
    if stats_seen.is_empty() {
        return Ok(());
    }
    for (primary_key, row) in file_column_stats {
        if !stats_seen.contains(primary_key) {
            result.errors.push(format!(
                "missing file-column-stats index for {}/{}/{}",
                row.table_id, row.column_id, row.data_file_id
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn verify_counters(
    db: &Db,
    result: &mut VerifyResult,
    latest: u64,
    snapshots: &[(Vec<u8>, SnapshotRow)],
    schemas: &[(Vec<u8>, SchemaRow)],
    tables: &[(Vec<u8>, TableRow)],
    columns: &[(Vec<u8>, ColumnRow)],
    views: &[(Vec<u8>, ViewRow)],
    macros: &[(Vec<u8>, MacroRow)],
    macro_impls: &[(Vec<u8>, MacroImplRow)],
    column_mappings: &[(Vec<u8>, ColumnMappingRow)],
    partitions: &[(Vec<u8>, PartitionInfoRow)],
    sorts: &[(Vec<u8>, SortInfoRow)],
    data_files: &[(Vec<u8>, DataFileRow)],
    delete_files: &[(Vec<u8>, DeleteFileRow)],
    scheduled: &[(Vec<u8>, FilesScheduledForDeletionRow)],
    table_stats: &[(Vec<u8>, TableStatsRow)],
    file_column_stats: &[(Vec<u8>, FileColumnStatsRow)],
    file_variant_stats: &[(Vec<u8>, FileVariantStatsRow)],
    inline_inserts: &[(Vec<u8>, InlinedInsertRow)],
    inline_deletes: &[(Vec<u8>, InlinedDeleteRow)],
) -> CatalogResult<()> {
    let max_catalog = schemas
        .iter()
        .map(|(_, row)| row.schema_id)
        .chain(tables.iter().map(|(_, row)| row.table_id))
        .chain(columns.iter().map(|(_, row)| row.column_id))
        .chain(views.iter().map(|(_, row)| row.view_id))
        .chain(macros.iter().map(|(_, row)| row.macro_id))
        .chain(macro_impls.iter().map(|(_, row)| row.impl_id))
        .chain(column_mappings.iter().map(|(_, row)| row.mapping_id))
        .chain(partitions.iter().map(|(_, row)| row.partition_id))
        .chain(sorts.iter().map(|(_, row)| row.sort_id))
        .max()
        .unwrap_or(0);
    let max_file = data_files
        .iter()
        .map(|(_, row)| row.data_file_id)
        .chain(delete_files.iter().map(|(_, row)| row.delete_file_id))
        .chain(scheduled.iter().map(|(_, row)| row.data_file_id))
        .chain(file_column_stats.iter().map(|(_, row)| row.data_file_id))
        .chain(file_variant_stats.iter().map(|(_, row)| row.data_file_id))
        .max()
        .unwrap_or(0);
    check_counter(
        result,
        db,
        "next_snapshot_id",
        COUNTER_NEXT_SNAPSHOT_ID,
        latest.saturating_add(1),
    )
    .await?;
    check_counter(
        result,
        db,
        "next_catalog_id",
        COUNTER_NEXT_CATALOG_ID,
        max_catalog.saturating_add(1),
    )
    .await?;
    check_counter(
        result,
        db,
        "next_file_id",
        COUNTER_NEXT_FILE_ID,
        max_file.saturating_add(1),
    )
    .await?;

    let mut max_columns = HashMap::<u64, u64>::new();
    for (_, row) in columns {
        max_columns
            .entry(row.table_id)
            .and_modify(|max| *max = (*max).max(row.column_id))
            .or_insert(row.column_id);
    }
    let mut iter = db
        .scan_prefix(&[TAG_COUNTERS, COUNTER_NEXT_COLUMN_ID_PREFIX])
        .await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        if kv.key.len() != 10 {
            result.errors.push(format!(
                "invalid per-table column counter key length {}",
                kv.key.len()
            ));
            continue;
        }
        let table_id = be_u64(&kv.key[2..10]);
        let expected = max_columns
            .get(&table_id)
            .copied()
            .unwrap_or(0)
            .saturating_add(1);
        match values::decode_counter(&kv.value) {
            Ok(current) if current != expected => result.errors.push(format!(
                "column counter for table {table_id} is {current}, expected {expected}"
            )),
            Err(error) => result.errors.push(format!(
                "invalid column counter for table {table_id}: {error}"
            )),
            _ => {}
        }
    }

    let mut max_row_ids = HashMap::<u64, u64>::new();
    for (_, row) in data_files {
        if let Some(start) = row.row_id_start {
            let end = start.saturating_add(row.record_count);
            max_row_ids
                .entry(row.table_id)
                .and_modify(|max| *max = (*max).max(end))
                .or_insert(end);
        }
    }
    for (_, row) in table_stats {
        if let Some(next) = row.next_row_id {
            max_row_ids
                .entry(row.table_id)
                .and_modify(|max| *max = (*max).max(next))
                .or_insert(next);
        }
    }
    for (_, row) in inline_inserts {
        max_row_ids
            .entry(row.table_id)
            .and_modify(|max| *max = (*max).max(row.row_id.saturating_add(1)))
            .or_insert(row.row_id.saturating_add(1));
    }
    for (_, row) in inline_deletes {
        max_row_ids
            .entry(row.table_id)
            .and_modify(|max| *max = (*max).max(row.row_id.saturating_add(1)))
            .or_insert(row.row_id.saturating_add(1));
    }
    let mut iter = db
        .scan_prefix(&[TAG_COUNTERS, COUNTER_NEXT_ROWID_PREFIX])
        .await?;
    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| CatalogError::SlateDb(e.to_string()))?
    {
        if kv.key.len() != 10 {
            result.errors.push(format!(
                "invalid per-table rowid counter key length {}",
                kv.key.len()
            ));
            continue;
        }
        let table_id = be_u64(&kv.key[2..10]);
        let expected = max_row_ids.get(&table_id).copied().unwrap_or(0);
        match values::decode_counter(&kv.value) {
            Ok(current) if current != expected => result.errors.push(format!(
                "rowid counter for table {table_id} is {current}, expected {expected}"
            )),
            Err(error) => result.errors.push(format!(
                "invalid rowid counter for table {table_id}: {error}"
            )),
            _ => {}
        }
    }

    if let Some((_, snapshot)) = snapshots.iter().max_by_key(|(_, row)| row.snapshot_id) {
        if let Some(hint) = snapshot.next_catalog_id {
            check_hint(result, "snapshot next_catalog_id", hint, max_catalog + 1);
        }
        if let Some(hint) = snapshot.next_file_id {
            check_hint(result, "snapshot next_file_id", hint, max_file + 1);
        }
    }
    Ok(())
}

async fn check_counter(
    result: &mut VerifyResult,
    db: &Db,
    name: &str,
    id: u8,
    expected: u64,
) -> CatalogResult<()> {
    match db.get(keys::key_counter(id)).await? {
        None => result.errors.push(format!("missing counter: {name}")),
        Some(data) => match values::decode_counter(&data) {
            Ok(current) if current != expected => result
                .errors
                .push(format!("counter {name} is {current}, expected {expected}")),
            Ok(_) => {}
            Err(error) => result
                .errors
                .push(format!("invalid counter {name}: {error}")),
        },
    }
    Ok(())
}

fn check_hint(result: &mut VerifyResult, name: &str, actual: u64, expected: u64) {
    if actual != expected {
        result
            .errors
            .push(format!("{name} is {actual}, expected {expected}"));
    }
}

fn be_u64(bytes: &[u8]) -> u64 {
    u64::from_be_bytes(bytes.try_into().expect("verified key width"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
