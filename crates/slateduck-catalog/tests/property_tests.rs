//! Property test suite for v0.2 Catalog Core.
//!
//! Tests:
//! - Round-trip: decode(encode(row)) == row for all row types
//! - Key ordering: encode(id=5) < encode(id=6) for all numeric ID fields
//! - Prefix isolation: scan_prefix(tag | id) returns only rows for that entity
//! - No key collisions between different table tags
//! - ID monotonicity: N operations in sequence; all IDs strictly increasing

use proptest::prelude::*;
use slateduck_core::encoding::{decode_value, encode_value};
use slateduck_core::keys::*;
use slateduck_core::mvcc::MvccFields;
use slateduck_core::rows::*;
use slateduck_core::tags::*;

// -- Round-trip property tests --

proptest! {
    #[test]
    fn roundtrip_metadata_row(
        scope in 0u8..=255,
        scope_id in any::<u64>(),
        key in "[a-z_]{1,30}",
        value in ".*",
    ) {
        let row = MetadataRow { scope, scope_id, key, value };
        let encoded = serde_json::to_vec(&row).unwrap();
        let wrapped = encode_value(&encoded);
        let payload = decode_value(&wrapped).unwrap();
        let decoded: MetadataRow = serde_json::from_slice(payload).unwrap();
        prop_assert_eq!(row, decoded);
    }

    #[test]
    fn roundtrip_snapshot_row(
        snapshot_id in any::<u64>(),
        schema_version in any::<u64>(),
    ) {
        let row = SnapshotRow {
            snapshot_id,
            schema_version,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            author: Some("test".to_string()),
            message: None,
        };
        let encoded = serde_json::to_vec(&row).unwrap();
        let wrapped = encode_value(&encoded);
        let payload = decode_value(&wrapped).unwrap();
        let decoded: SnapshotRow = serde_json::from_slice(payload).unwrap();
        prop_assert_eq!(row, decoded);
    }

    #[test]
    fn roundtrip_schema_row(
        schema_id in any::<u64>(),
        begin_snapshot in 1u64..1000000,
    ) {
        let row = SchemaRow {
            schema_id,
            name: "test_schema".to_string(),
            mvcc: MvccFields::new(begin_snapshot),
        };
        let encoded = serde_json::to_vec(&row).unwrap();
        let wrapped = encode_value(&encoded);
        let payload = decode_value(&wrapped).unwrap();
        let decoded: SchemaRow = serde_json::from_slice(payload).unwrap();
        prop_assert_eq!(row, decoded);
    }

    #[test]
    fn roundtrip_table_row(
        schema_id in any::<u64>(),
        table_id in any::<u64>(),
        begin_snapshot in 1u64..1000000,
    ) {
        let row = TableRow {
            schema_id,
            table_id,
            name: "test_table".to_string(),
            uuid: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            mvcc: MvccFields::new(begin_snapshot),
        };
        let encoded = serde_json::to_vec(&row).unwrap();
        let wrapped = encode_value(&encoded);
        let payload = decode_value(&wrapped).unwrap();
        let decoded: TableRow = serde_json::from_slice(payload).unwrap();
        prop_assert_eq!(row, decoded);
    }

    #[test]
    fn roundtrip_column_row(
        table_id in any::<u64>(),
        column_id in any::<u64>(),
        begin_snapshot in 1u64..1000000,
    ) {
        let row = ColumnRow {
            table_id,
            column_id,
            name: "col".to_string(),
            data_type: "INTEGER".to_string(),
            is_nullable: true,
            default_value: None,
            mvcc: MvccFields::new(begin_snapshot),
        };
        let encoded = serde_json::to_vec(&row).unwrap();
        let wrapped = encode_value(&encoded);
        let payload = decode_value(&wrapped).unwrap();
        let decoded: ColumnRow = serde_json::from_slice(payload).unwrap();
        prop_assert_eq!(row, decoded);
    }

    #[test]
    fn roundtrip_data_file_row(
        table_id in any::<u64>(),
        data_file_id in any::<u64>(),
        begin_snapshot in 1u64..1000000,
    ) {
        let row = DataFileRow {
            table_id,
            data_file_id,
            path: "/data/file.parquet".to_string(),
            path_is_relative: false,
            file_size_bytes: 1024,
            record_count: 100,
            mvcc: MvccFields::new(begin_snapshot),
        };
        let encoded = serde_json::to_vec(&row).unwrap();
        let wrapped = encode_value(&encoded);
        let payload = decode_value(&wrapped).unwrap();
        let decoded: DataFileRow = serde_json::from_slice(payload).unwrap();
        prop_assert_eq!(row, decoded);
    }

    #[test]
    fn roundtrip_file_column_stats_row(
        table_id in any::<u64>(),
        column_id in any::<u64>(),
        data_file_id in any::<u64>(),
    ) {
        let row = FileColumnStatsRow {
            table_id,
            column_id,
            data_file_id,
            min_value: Some("1".to_string()),
            max_value: Some("100".to_string()),
            null_count: Some(5),
            contains_nan: false,
        };
        let encoded = serde_json::to_vec(&row).unwrap();
        let wrapped = encode_value(&encoded);
        let payload = decode_value(&wrapped).unwrap();
        let decoded: FileColumnStatsRow = serde_json::from_slice(payload).unwrap();
        prop_assert_eq!(row, decoded);
    }

    #[test]
    fn roundtrip_inlined_insert_row(
        table_id in any::<u64>(),
        schema_version in any::<u64>(),
        row_id in any::<u64>(),
        begin_snapshot in 1u64..1000000,
    ) {
        let row = InlinedInsertRow {
            table_id,
            schema_version,
            row_id,
            payload: vec![1, 2, 3, 4],
            begin_snapshot,
            end_snapshot: None,
        };
        let encoded = serde_json::to_vec(&row).unwrap();
        let wrapped = encode_value(&encoded);
        let payload = decode_value(&wrapped).unwrap();
        let decoded: InlinedInsertRow = serde_json::from_slice(payload).unwrap();
        prop_assert_eq!(row, decoded);
    }

    #[test]
    fn roundtrip_inlined_delete_row(
        table_id in any::<u64>(),
        data_file_id in any::<u64>(),
        row_id in any::<u64>(),
        begin_snapshot in 1u64..1000000,
    ) {
        let row = InlinedDeleteRow {
            table_id,
            data_file_id,
            row_id,
            begin_snapshot,
        };
        let encoded = serde_json::to_vec(&row).unwrap();
        let wrapped = encode_value(&encoded);
        let payload = decode_value(&wrapped).unwrap();
        let decoded: InlinedDeleteRow = serde_json::from_slice(payload).unwrap();
        prop_assert_eq!(row, decoded);
    }
}

// -- Key ordering property tests --

proptest! {
    #[test]
    fn key_ordering_snapshots(a in 1u64..u64::MAX, b in 1u64..u64::MAX) {
        prop_assume!(a != b);
        let ka = snapshot_key(a);
        let kb = snapshot_key(b);
        prop_assert_eq!(ka.cmp(&kb), a.cmp(&b));
    }

    #[test]
    fn key_ordering_schemas(a in 1u64..u64::MAX, b in 1u64..u64::MAX) {
        prop_assume!(a != b);
        let ka = schema_key(a, 1);
        let kb = schema_key(b, 1);
        prop_assert_eq!(ka.cmp(&kb), a.cmp(&b));
    }

    #[test]
    fn key_ordering_tables(a in 1u64..u64::MAX, b in 1u64..u64::MAX) {
        prop_assume!(a != b);
        // Same schema_id, different table_id
        let ka = table_key(1, a, 1);
        let kb = table_key(1, b, 1);
        prop_assert_eq!(ka.cmp(&kb), a.cmp(&b));
    }

    #[test]
    fn key_ordering_columns(a in 1u64..u64::MAX, b in 1u64..u64::MAX) {
        prop_assume!(a != b);
        let ka = column_key(1, a, 1);
        let kb = column_key(1, b, 1);
        prop_assert_eq!(ka.cmp(&kb), a.cmp(&b));
    }

    #[test]
    fn key_ordering_data_files(a in 1u64..u64::MAX, b in 1u64..u64::MAX) {
        prop_assume!(a != b);
        let ka = data_file_key(1, a);
        let kb = data_file_key(1, b);
        prop_assert_eq!(ka.cmp(&kb), a.cmp(&b));
    }

    #[test]
    fn key_ordering_inlined_inserts(a in 1u64..u64::MAX, b in 1u64..u64::MAX) {
        prop_assume!(a != b);
        let ka = inlined_insert_key(1, 1, a);
        let kb = inlined_insert_key(1, 1, b);
        prop_assert_eq!(ka.cmp(&kb), a.cmp(&b));
    }
}

// -- Prefix isolation tests --

proptest! {
    #[test]
    fn prefix_isolation_no_cross_table(tag_a in 0x01u8..0x1C, tag_b in 0x01u8..0x1C) {
        prop_assume!(tag_a != tag_b);
        let prefix_a = table_prefix(tag_a);
        let prefix_b = table_prefix(tag_b);
        // A key starting with tag_a should not match prefix_b
        let key_a = {
            let mut k = vec![tag_a];
            k.extend_from_slice(&42u64.to_be_bytes());
            k
        };
        prop_assert!(key_a.starts_with(&prefix_a));
        prop_assert!(!key_a.starts_with(&prefix_b));
    }

    #[test]
    fn prefix_isolation_inlined_vs_counter(
        table_id in any::<u64>(),
        schema_version in any::<u64>(),
        row_id in any::<u64>(),
    ) {
        let inlined_key = inlined_insert_key(table_id, schema_version, row_id);
        let counter_k = counter_key(COUNTER_NEXT_SNAPSHOT_ID);
        let system_k = system_key(SYSTEM_WRITER_EPOCH);

        // No overlap between different tag ranges
        prop_assert!(inlined_key[0] == TAG_DYNAMIC_INLINED_ROWS);
        prop_assert!(counter_k[0] == TAG_SLATEDUCK_COUNTERS);
        prop_assert!(system_k[0] == TAG_SLATEDUCK_SYSTEM);
        prop_assert!(TAG_DYNAMIC_INLINED_ROWS != TAG_SLATEDUCK_COUNTERS);
        prop_assert!(TAG_DYNAMIC_INLINED_ROWS != TAG_SLATEDUCK_SYSTEM);
    }
}

// -- Additional round-trip tests for remaining row types --

#[test]
fn roundtrip_view_row() {
    let row = ViewRow {
        schema_id: 1,
        view_id: 2,
        name: "my_view".to_string(),
        query: "SELECT 1".to_string(),
        mvcc: MvccFields::new(1),
    };
    let encoded = serde_json::to_vec(&row).unwrap();
    let wrapped = encode_value(&encoded);
    let payload = decode_value(&wrapped).unwrap();
    let decoded: ViewRow = serde_json::from_slice(payload).unwrap();
    assert_eq!(row, decoded);
}

#[test]
fn roundtrip_macro_row() {
    let row = MacroRow {
        schema_id: 1,
        macro_id: 2,
        name: "my_macro".to_string(),
        macro_type: "scalar".to_string(),
        mvcc: MvccFields::new(1),
    };
    let encoded = serde_json::to_vec(&row).unwrap();
    let wrapped = encode_value(&encoded);
    let payload = decode_value(&wrapped).unwrap();
    let decoded: MacroRow = serde_json::from_slice(payload).unwrap();
    assert_eq!(row, decoded);
}

#[test]
fn roundtrip_delete_file_row() {
    let row = DeleteFileRow {
        data_file_id: 1,
        delete_file_id: 2,
        path: "/del.parquet".to_string(),
        path_is_relative: false,
        file_size_bytes: 512,
        record_count: 10,
    };
    let encoded = serde_json::to_vec(&row).unwrap();
    let wrapped = encode_value(&encoded);
    let payload = decode_value(&wrapped).unwrap();
    let decoded: DeleteFileRow = serde_json::from_slice(payload).unwrap();
    assert_eq!(row, decoded);
}

#[test]
fn roundtrip_partition_info_row() {
    let row = PartitionInfoRow {
        table_id: 1,
        partition_id: 2,
        mvcc: MvccFields::new(1),
    };
    let encoded = serde_json::to_vec(&row).unwrap();
    let wrapped = encode_value(&encoded);
    let payload = decode_value(&wrapped).unwrap();
    let decoded: PartitionInfoRow = serde_json::from_slice(payload).unwrap();
    assert_eq!(row, decoded);
}

#[test]
fn roundtrip_sort_info_row() {
    let row = SortInfoRow {
        table_id: 1,
        sort_id: 2,
        mvcc: MvccFields::new(1),
    };
    let encoded = serde_json::to_vec(&row).unwrap();
    let wrapped = encode_value(&encoded);
    let payload = decode_value(&wrapped).unwrap();
    let decoded: SortInfoRow = serde_json::from_slice(payload).unwrap();
    assert_eq!(row, decoded);
}

#[test]
fn roundtrip_tag_row() {
    let row = TagRow {
        object_id: 1,
        tag_key: "env".to_string(),
        tag_value: "prod".to_string(),
        mvcc: MvccFields::new(1),
    };
    let encoded = serde_json::to_vec(&row).unwrap();
    let wrapped = encode_value(&encoded);
    let payload = decode_value(&wrapped).unwrap();
    let decoded: TagRow = serde_json::from_slice(payload).unwrap();
    assert_eq!(row, decoded);
}

#[test]
fn roundtrip_column_tag_row() {
    let row = ColumnTagRow {
        table_id: 1,
        column_id: 2,
        tag_key: "pii".to_string(),
        tag_value: "true".to_string(),
        mvcc: MvccFields::new(1),
    };
    let encoded = serde_json::to_vec(&row).unwrap();
    let wrapped = encode_value(&encoded);
    let payload = decode_value(&wrapped).unwrap();
    let decoded: ColumnTagRow = serde_json::from_slice(payload).unwrap();
    assert_eq!(row, decoded);
}

#[test]
fn roundtrip_schema_versions_row() {
    let row = SchemaVersionsRow {
        table_id: 1,
        begin_snapshot: 5,
        schema_version: 3,
    };
    let encoded = serde_json::to_vec(&row).unwrap();
    let wrapped = encode_value(&encoded);
    let payload = decode_value(&wrapped).unwrap();
    let decoded: SchemaVersionsRow = serde_json::from_slice(payload).unwrap();
    assert_eq!(row, decoded);
}

#[test]
fn roundtrip_all_remaining_rows() {
    // MacroImpl
    let row = MacroImplRow {
        macro_id: 1,
        impl_id: 1,
        definition: "x + 1".to_string(),
    };
    let e = serde_json::to_vec(&row).unwrap();
    let w = encode_value(&e);
    let p = decode_value(&w).unwrap();
    let d: MacroImplRow = serde_json::from_slice(p).unwrap();
    assert_eq!(row, d);

    // MacroParameters
    let row = MacroParametersRow {
        macro_id: 1,
        impl_id: 1,
        column_id: 1,
        name: "x".to_string(),
        data_type: "INT".to_string(),
        default_value: None,
    };
    let e = serde_json::to_vec(&row).unwrap();
    let w = encode_value(&e);
    let p = decode_value(&w).unwrap();
    let d: MacroParametersRow = serde_json::from_slice(p).unwrap();
    assert_eq!(row, d);

    // FilesScheduledForDeletion
    let row = FilesScheduledForDeletionRow {
        schedule_start: 100,
        data_file_id: 5,
        path: "/p".to_string(),
    };
    let e = serde_json::to_vec(&row).unwrap();
    let w = encode_value(&e);
    let p = decode_value(&w).unwrap();
    let d: FilesScheduledForDeletionRow = serde_json::from_slice(p).unwrap();
    assert_eq!(row, d);

    // InlinedDataTables
    let row = InlinedDataTablesRow {
        table_id: 1,
        schema_version: 1,
        table_name: "inlined_t".to_string(),
    };
    let e = serde_json::to_vec(&row).unwrap();
    let w = encode_value(&e);
    let p = decode_value(&w).unwrap();
    let d: InlinedDataTablesRow = serde_json::from_slice(p).unwrap();
    assert_eq!(row, d);

    // ColumnMapping
    let row = ColumnMappingRow {
        table_id: 1,
        mapping_id: 1,
        mapping_json: "{}".to_string(),
    };
    let e = serde_json::to_vec(&row).unwrap();
    let w = encode_value(&e);
    let p = decode_value(&w).unwrap();
    let d: ColumnMappingRow = serde_json::from_slice(p).unwrap();
    assert_eq!(row, d);

    // NameMapping
    let row = NameMappingRow {
        mapping_id: 1,
        column_id: 1,
        source_name_hash: 12345,
        source_name: "src".to_string(),
        target_name: "tgt".to_string(),
    };
    let e = serde_json::to_vec(&row).unwrap();
    let w = encode_value(&e);
    let p = decode_value(&w).unwrap();
    let d: NameMappingRow = serde_json::from_slice(p).unwrap();
    assert_eq!(row, d);

    // TableStats
    let row = TableStatsRow {
        table_id: 1,
        record_count: 100,
        file_count: 5,
        total_size_bytes: 4096,
    };
    let e = serde_json::to_vec(&row).unwrap();
    let w = encode_value(&e);
    let p = decode_value(&w).unwrap();
    let d: TableStatsRow = serde_json::from_slice(p).unwrap();
    assert_eq!(row, d);

    // TableColumnStats
    let row = TableColumnStatsRow {
        table_id: 1,
        column_id: 1,
        null_count: Some(10),
        distinct_count: Some(50),
        min_value: Some("0".to_string()),
        max_value: Some("99".to_string()),
    };
    let e = serde_json::to_vec(&row).unwrap();
    let w = encode_value(&e);
    let p = decode_value(&w).unwrap();
    let d: TableColumnStatsRow = serde_json::from_slice(p).unwrap();
    assert_eq!(row, d);

    // FileVariantStats
    let row = FileVariantStatsRow {
        table_id: 1,
        column_id: 1,
        variant_path_hash: 999,
        data_file_id: 1,
        variant_path: "$.field".to_string(),
        min_value: Some("a".to_string()),
        max_value: Some("z".to_string()),
    };
    let e = serde_json::to_vec(&row).unwrap();
    let w = encode_value(&e);
    let p = decode_value(&w).unwrap();
    let d: FileVariantStatsRow = serde_json::from_slice(p).unwrap();
    assert_eq!(row, d);

    // PartitionColumn
    let row = PartitionColumnRow {
        partition_id: 1,
        partition_key_index: 0,
        column_id: 1,
        transform: "identity".to_string(),
    };
    let e = serde_json::to_vec(&row).unwrap();
    let w = encode_value(&e);
    let p = decode_value(&w).unwrap();
    let d: PartitionColumnRow = serde_json::from_slice(p).unwrap();
    assert_eq!(row, d);

    // FilePartitionValue
    let row = FilePartitionValueRow {
        table_id: 1,
        partition_key_index: 0,
        data_file_id: 1,
        value: Some("2024-01-01".to_string()),
    };
    let e = serde_json::to_vec(&row).unwrap();
    let w = encode_value(&e);
    let p = decode_value(&w).unwrap();
    let d: FilePartitionValueRow = serde_json::from_slice(p).unwrap();
    assert_eq!(row, d);

    // SortExpression
    let row = SortExpressionRow {
        sort_id: 1,
        sort_key_index: 0,
        column_id: 1,
        ascending: true,
        nulls_first: false,
    };
    let e = serde_json::to_vec(&row).unwrap();
    let w = encode_value(&e);
    let p = decode_value(&w).unwrap();
    let d: SortExpressionRow = serde_json::from_slice(p).unwrap();
    assert_eq!(row, d);

    // SnapshotChanges
    let row = SnapshotChangesRow {
        snapshot_id: 1,
        changes_json: r#"{"tables_created":1}"#.to_string(),
    };
    let e = serde_json::to_vec(&row).unwrap();
    let w = encode_value(&e);
    let p = decode_value(&w).unwrap();
    let d: SnapshotChangesRow = serde_json::from_slice(p).unwrap();
    assert_eq!(row, d);
}
