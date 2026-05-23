//! Binary key encoding/decoding for all 28 DuckLake tables plus system namespaces.
//!
//! All keys use big-endian integers and a leading `u8` table tag.
//! Tables with MVCC versioning include `begin_snapshot` in the key so
//! historical versions are never overwritten.

use crate::error::{Result, SlateDuckError};
use crate::tags::*;

/// Maximum size for encoded inlined-row values (64 MiB).
pub const MAX_INLINED_ROW_SIZE: usize = 64 * 1024 * 1024;

// -- Key builders --

/// Build a key for ducklake_metadata: tag | scope | scope_id | key_bytes (length-prefixed UTF-8).
pub fn metadata_key(scope: u8, scope_id: u64, key: &str) -> Vec<u8> {
    let key_bytes = key.as_bytes();
    let mut buf = Vec::with_capacity(1 + 1 + 8 + 2 + key_bytes.len());
    buf.push(TAG_DUCKLAKE_METADATA);
    buf.push(scope);
    buf.extend_from_slice(&scope_id.to_be_bytes());
    buf.extend_from_slice(&(key_bytes.len() as u16).to_be_bytes());
    buf.extend_from_slice(key_bytes);
    buf
}

/// Build a key for ducklake_snapshot: tag | snapshot_id.
pub fn snapshot_key(snapshot_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(9);
    buf.push(TAG_DUCKLAKE_SNAPSHOT);
    buf.extend_from_slice(&snapshot_id.to_be_bytes());
    buf
}

/// Build a key for ducklake_snapshot_changes: tag | snapshot_id.
pub fn snapshot_changes_key(snapshot_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(9);
    buf.push(TAG_DUCKLAKE_SNAPSHOT_CHANGES);
    buf.extend_from_slice(&snapshot_id.to_be_bytes());
    buf
}

/// Build a key for ducklake_schema: tag | schema_id | begin_snapshot.
pub fn schema_key(schema_id: u64, begin_snapshot: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(17);
    buf.push(TAG_DUCKLAKE_SCHEMA);
    buf.extend_from_slice(&schema_id.to_be_bytes());
    buf.extend_from_slice(&begin_snapshot.to_be_bytes());
    buf
}

/// Build a key for ducklake_table: tag | schema_id | table_id | begin_snapshot.
pub fn table_key(schema_id: u64, table_id: u64, begin_snapshot: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(25);
    buf.push(TAG_DUCKLAKE_TABLE);
    buf.extend_from_slice(&schema_id.to_be_bytes());
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&begin_snapshot.to_be_bytes());
    buf
}

/// Build a key for ducklake_column: tag | table_id | column_id | begin_snapshot.
pub fn column_key(table_id: u64, column_id: u64, begin_snapshot: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(25);
    buf.push(TAG_DUCKLAKE_COLUMN);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&column_id.to_be_bytes());
    buf.extend_from_slice(&begin_snapshot.to_be_bytes());
    buf
}

/// Build a key for ducklake_view: tag | schema_id | view_id | begin_snapshot.
pub fn view_key(schema_id: u64, view_id: u64, begin_snapshot: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(25);
    buf.push(TAG_DUCKLAKE_VIEW);
    buf.extend_from_slice(&schema_id.to_be_bytes());
    buf.extend_from_slice(&view_id.to_be_bytes());
    buf.extend_from_slice(&begin_snapshot.to_be_bytes());
    buf
}

/// Build a key for ducklake_macro: tag | schema_id | macro_id | begin_snapshot.
pub fn macro_key(schema_id: u64, macro_id: u64, begin_snapshot: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(25);
    buf.push(TAG_DUCKLAKE_MACRO);
    buf.extend_from_slice(&schema_id.to_be_bytes());
    buf.extend_from_slice(&macro_id.to_be_bytes());
    buf.extend_from_slice(&begin_snapshot.to_be_bytes());
    buf
}

/// Build a key for ducklake_macro_impl: tag | macro_id | impl_id.
pub fn macro_impl_key(macro_id: u64, impl_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(17);
    buf.push(TAG_DUCKLAKE_MACRO_IMPL);
    buf.extend_from_slice(&macro_id.to_be_bytes());
    buf.extend_from_slice(&impl_id.to_be_bytes());
    buf
}

/// Build a key for ducklake_macro_parameters: tag | macro_id | impl_id | column_id.
pub fn macro_parameters_key(macro_id: u64, impl_id: u64, column_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(25);
    buf.push(TAG_DUCKLAKE_MACRO_PARAMETERS);
    buf.extend_from_slice(&macro_id.to_be_bytes());
    buf.extend_from_slice(&impl_id.to_be_bytes());
    buf.extend_from_slice(&column_id.to_be_bytes());
    buf
}

/// Build a key for ducklake_data_file: tag | table_id | data_file_id.
pub fn data_file_key(table_id: u64, data_file_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(17);
    buf.push(TAG_DUCKLAKE_DATA_FILE);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&data_file_id.to_be_bytes());
    buf
}

/// Build a key for ducklake_delete_file: tag | data_file_id | delete_file_id.
pub fn delete_file_key(data_file_id: u64, delete_file_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(17);
    buf.push(TAG_DUCKLAKE_DELETE_FILE);
    buf.extend_from_slice(&data_file_id.to_be_bytes());
    buf.extend_from_slice(&delete_file_id.to_be_bytes());
    buf
}

/// Build a key for ducklake_files_scheduled_for_deletion: tag | schedule_start | data_file_id.
pub fn files_scheduled_for_deletion_key(schedule_start: u64, data_file_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(17);
    buf.push(TAG_DUCKLAKE_FILES_SCHEDULED_FOR_DELETION);
    buf.extend_from_slice(&schedule_start.to_be_bytes());
    buf.extend_from_slice(&data_file_id.to_be_bytes());
    buf
}

/// Build a key for ducklake_inlined_data_tables: tag | table_id | schema_version.
pub fn inlined_data_tables_key(table_id: u64, schema_version: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(17);
    buf.push(TAG_DUCKLAKE_INLINED_DATA_TABLES);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&schema_version.to_be_bytes());
    buf
}

/// Build a key for ducklake_column_mapping: tag | table_id | mapping_id.
pub fn column_mapping_key(table_id: u64, mapping_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(17);
    buf.push(TAG_DUCKLAKE_COLUMN_MAPPING);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&mapping_id.to_be_bytes());
    buf
}

/// Build a key for ducklake_name_mapping: tag | mapping_id | column_id | source_name_hash.
pub fn name_mapping_key(mapping_id: u64, column_id: u64, source_name_hash: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(25);
    buf.push(TAG_DUCKLAKE_NAME_MAPPING);
    buf.extend_from_slice(&mapping_id.to_be_bytes());
    buf.extend_from_slice(&column_id.to_be_bytes());
    buf.extend_from_slice(&source_name_hash.to_be_bytes());
    buf
}

/// Build a key for ducklake_table_stats: tag | table_id.
pub fn table_stats_key(table_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(9);
    buf.push(TAG_DUCKLAKE_TABLE_STATS);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf
}

/// Build a key for ducklake_table_column_stats: tag | table_id | column_id.
pub fn table_column_stats_key(table_id: u64, column_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(17);
    buf.push(TAG_DUCKLAKE_TABLE_COLUMN_STATS);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&column_id.to_be_bytes());
    buf
}

/// Build a key for ducklake_file_column_stats: tag | table_id | column_id | data_file_id.
pub fn file_column_stats_key(table_id: u64, column_id: u64, data_file_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(25);
    buf.push(TAG_DUCKLAKE_FILE_COLUMN_STATS);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&column_id.to_be_bytes());
    buf.extend_from_slice(&data_file_id.to_be_bytes());
    buf
}

/// Build a key for ducklake_file_variant_stats: tag | table_id | column_id | variant_path_hash | data_file_id.
pub fn file_variant_stats_key(
    table_id: u64,
    column_id: u64,
    variant_path_hash: u64,
    data_file_id: u64,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(33);
    buf.push(TAG_DUCKLAKE_FILE_VARIANT_STATS);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&column_id.to_be_bytes());
    buf.extend_from_slice(&variant_path_hash.to_be_bytes());
    buf.extend_from_slice(&data_file_id.to_be_bytes());
    buf
}

/// Build a key for ducklake_partition_info: tag | table_id | partition_id | begin_snapshot.
pub fn partition_info_key(table_id: u64, partition_id: u64, begin_snapshot: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(25);
    buf.push(TAG_DUCKLAKE_PARTITION_INFO);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&partition_id.to_be_bytes());
    buf.extend_from_slice(&begin_snapshot.to_be_bytes());
    buf
}

/// Build a key for ducklake_partition_column: tag | partition_id | partition_key_index.
pub fn partition_column_key(partition_id: u64, partition_key_index: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(17);
    buf.push(TAG_DUCKLAKE_PARTITION_COLUMN);
    buf.extend_from_slice(&partition_id.to_be_bytes());
    buf.extend_from_slice(&partition_key_index.to_be_bytes());
    buf
}

/// Build a key for ducklake_file_partition_value: tag | table_id | partition_key_index | data_file_id.
pub fn file_partition_value_key(
    table_id: u64,
    partition_key_index: u64,
    data_file_id: u64,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(25);
    buf.push(TAG_DUCKLAKE_FILE_PARTITION_VALUE);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&partition_key_index.to_be_bytes());
    buf.extend_from_slice(&data_file_id.to_be_bytes());
    buf
}

/// Build a key for ducklake_sort_info: tag | table_id | sort_id | begin_snapshot.
pub fn sort_info_key(table_id: u64, sort_id: u64, begin_snapshot: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(25);
    buf.push(TAG_DUCKLAKE_SORT_INFO);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&sort_id.to_be_bytes());
    buf.extend_from_slice(&begin_snapshot.to_be_bytes());
    buf
}

/// Build a key for ducklake_sort_expression: tag | sort_id | sort_key_index.
pub fn sort_expression_key(sort_id: u64, sort_key_index: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(17);
    buf.push(TAG_DUCKLAKE_SORT_EXPRESSION);
    buf.extend_from_slice(&sort_id.to_be_bytes());
    buf.extend_from_slice(&sort_key_index.to_be_bytes());
    buf
}

/// Build a key for ducklake_tag: tag | object_id | tag_key_hash | begin_snapshot.
pub fn tag_key(object_id: u64, tag_key_hash: u64, begin_snapshot: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(25);
    buf.push(TAG_DUCKLAKE_TAG);
    buf.extend_from_slice(&object_id.to_be_bytes());
    buf.extend_from_slice(&tag_key_hash.to_be_bytes());
    buf.extend_from_slice(&begin_snapshot.to_be_bytes());
    buf
}

/// Build a key for ducklake_column_tag: tag | table_id | column_id | tag_key_hash | begin_snapshot.
pub fn column_tag_key(
    table_id: u64,
    column_id: u64,
    tag_key_hash: u64,
    begin_snapshot: u64,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(33);
    buf.push(TAG_DUCKLAKE_COLUMN_TAG);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&column_id.to_be_bytes());
    buf.extend_from_slice(&tag_key_hash.to_be_bytes());
    buf.extend_from_slice(&begin_snapshot.to_be_bytes());
    buf
}

/// Build a key for ducklake_schema_versions: tag | table_id | begin_snapshot.
pub fn schema_versions_key(table_id: u64, begin_snapshot: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(17);
    buf.push(TAG_DUCKLAKE_SCHEMA_VERSIONS);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&begin_snapshot.to_be_bytes());
    buf
}

// -- Dynamic inlined rows (0xFD) --

/// Build a key for inlined insert row: 0xFD | 0x01 | table_id | schema_version | row_id.
pub fn inlined_insert_key(table_id: u64, schema_version: u64, row_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(26);
    buf.push(TAG_DYNAMIC_INLINED_ROWS);
    buf.push(INLINED_SUBTYPE_INSERT);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&schema_version.to_be_bytes());
    buf.extend_from_slice(&row_id.to_be_bytes());
    buf
}

/// Build a key for inlined delete marker: 0xFD | 0x02 | table_id | data_file_id | row_id.
pub fn inlined_delete_key(table_id: u64, data_file_id: u64, row_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(26);
    buf.push(TAG_DYNAMIC_INLINED_ROWS);
    buf.push(INLINED_SUBTYPE_DELETE);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&data_file_id.to_be_bytes());
    buf.extend_from_slice(&row_id.to_be_bytes());
    buf
}

// -- Counter keys (0xFE) --

/// Build a key for a global counter: 0xFE | counter_id.
pub fn counter_key(counter_id: u8) -> Vec<u8> {
    vec![TAG_SLATEDUCK_COUNTERS, counter_id]
}

/// Build a key for a per-table counter: 0xFE | 0x10 | table_id.
pub fn table_counter_key(table_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(10);
    buf.push(TAG_SLATEDUCK_COUNTERS);
    buf.push(COUNTER_NEXT_COLUMN_ID_PREFIX);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf
}

// -- System keys (0xFF) --

/// Build a system key: 0xFF | key_name.
pub fn system_key(name: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(1 + name.len());
    buf.push(TAG_SLATEDUCK_SYSTEM);
    buf.extend_from_slice(name);
    buf
}

// -- Prefix builders for scans --

/// Prefix for scanning all rows of a given table tag.
pub fn table_prefix(tag: u8) -> Vec<u8> {
    vec![tag]
}

/// Prefix for scanning ducklake_table rows by schema_id.
pub fn table_by_schema_prefix(schema_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(9);
    buf.push(TAG_DUCKLAKE_TABLE);
    buf.extend_from_slice(&schema_id.to_be_bytes());
    buf
}

/// Prefix for scanning ducklake_column rows by table_id.
pub fn columns_by_table_prefix(table_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(9);
    buf.push(TAG_DUCKLAKE_COLUMN);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf
}

/// Prefix for scanning ducklake_data_file rows by table_id.
pub fn data_files_by_table_prefix(table_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(9);
    buf.push(TAG_DUCKLAKE_DATA_FILE);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf
}

/// Prefix for scanning ducklake_file_column_stats by table_id and column_id.
pub fn file_column_stats_prefix(table_id: u64, column_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(17);
    buf.push(TAG_DUCKLAKE_FILE_COLUMN_STATS);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf.extend_from_slice(&column_id.to_be_bytes());
    buf
}

/// Prefix for scanning inlined insert rows by table_id.
pub fn inlined_inserts_by_table_prefix(table_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(10);
    buf.push(TAG_DYNAMIC_INLINED_ROWS);
    buf.push(INLINED_SUBTYPE_INSERT);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf
}

/// Prefix for scanning inlined delete markers by table_id.
pub fn inlined_deletes_by_table_prefix(table_id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(10);
    buf.push(TAG_DYNAMIC_INLINED_ROWS);
    buf.push(INLINED_SUBTYPE_DELETE);
    buf.extend_from_slice(&table_id.to_be_bytes());
    buf
}

// -- Key parsing --

/// Extract the table tag from a raw key.
pub fn key_tag(key: &[u8]) -> Result<u8> {
    if key.is_empty() {
        return Err(SlateDuckError::Encoding("empty key".to_string()));
    }
    Ok(key[0])
}

/// Extract a u64 at the given byte offset in a key.
pub fn read_u64_at(key: &[u8], offset: usize) -> Result<u64> {
    if key.len() < offset + 8 {
        return Err(SlateDuckError::Encoding(format!(
            "key too short to read u64 at offset {offset}: len={}",
            key.len()
        )));
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&key[offset..offset + 8]);
    Ok(u64::from_be_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_ordering_snapshot() {
        let k1 = snapshot_key(1);
        let k2 = snapshot_key(2);
        let k3 = snapshot_key(100);
        assert!(k1 < k2);
        assert!(k2 < k3);
    }

    #[test]
    fn key_ordering_table() {
        // Same schema, different table IDs
        let k1 = table_key(1, 1, 1);
        let k2 = table_key(1, 2, 1);
        assert!(k1 < k2);

        // Same schema and table, different begin_snapshot
        let k3 = table_key(1, 1, 1);
        let k4 = table_key(1, 1, 2);
        assert!(k3 < k4);
    }

    #[test]
    fn key_ordering_column() {
        let k1 = column_key(1, 1, 1);
        let k2 = column_key(1, 2, 1);
        assert!(k1 < k2);
    }

    #[test]
    fn prefix_isolation() {
        // Different tags never share prefix
        let snap = snapshot_key(1);
        let schema = schema_key(1, 1);
        assert_ne!(snap[0], schema[0]);

        // scan_prefix for TAG_DUCKLAKE_SNAPSHOT won't match TAG_DUCKLAKE_SCHEMA
        let prefix = table_prefix(TAG_DUCKLAKE_SNAPSHOT);
        assert!(snap.starts_with(&prefix));
        assert!(!schema.starts_with(&prefix));
    }

    #[test]
    fn inlined_key_ordering() {
        let k1 = inlined_insert_key(1, 1, 1);
        let k2 = inlined_insert_key(1, 1, 2);
        let k3 = inlined_insert_key(1, 2, 1);
        assert!(k1 < k2);
        assert!(k2 < k3);
    }

    #[test]
    fn counter_key_layout() {
        let k = counter_key(COUNTER_NEXT_SNAPSHOT_ID);
        assert_eq!(k, vec![0xFE, 0x01]);

        let k = table_counter_key(42);
        assert_eq!(k[0], 0xFE);
        assert_eq!(k[1], 0x10);
        assert_eq!(read_u64_at(&k, 2).unwrap(), 42);
    }

    #[test]
    fn system_key_layout() {
        let k = system_key(SYSTEM_WRITER_EPOCH);
        assert_eq!(k[0], 0xFF);
        assert_eq!(&k[1..], b"writer-epoch");
    }

    #[test]
    fn file_column_stats_prefix_scan() {
        let prefix = file_column_stats_prefix(10, 5);
        let k1 = file_column_stats_key(10, 5, 1);
        let k2 = file_column_stats_key(10, 5, 2);
        let k3 = file_column_stats_key(10, 6, 1); // different column
        assert!(k1.starts_with(&prefix));
        assert!(k2.starts_with(&prefix));
        assert!(!k3.starts_with(&prefix));
    }

    #[test]
    fn metadata_key_structure() {
        let k = metadata_key(0x01, 0, "data_path");
        assert_eq!(k[0], TAG_DUCKLAKE_METADATA);
        assert_eq!(k[1], 0x01); // scope
                                // scope_id at offset 2..10
        assert_eq!(read_u64_at(&k, 2).unwrap(), 0);
        // length-prefixed key at offset 10
        let key_len = u16::from_be_bytes([k[10], k[11]]) as usize;
        assert_eq!(key_len, 9); // "data_path".len()
        assert_eq!(&k[12..12 + key_len], b"data_path");
    }
}
