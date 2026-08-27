//! v0.47.15 catalog-fidelity regression tests.

use std::io::BufReader;
use std::sync::Arc;

use object_store::memory::InMemory;
use object_store::path::Path as ObjectPath;
use rocklake_catalog::export::import_catalog;
use rocklake_catalog::migrate_from_ducklake::{
    migrate_from_source, DuckLakeSource, SqliteDuckLakeSource, DUCKLAKE_V1_0_CATALOG_VERSION,
};
use rocklake_catalog::repair::{repair_apply, RepairAction, RepairPlan};
use rocklake_catalog::verify::verify_catalog;
use rocklake_core::{keys, tags, values};
use slatedb::Db;

async fn fresh_db() -> Db {
    let store = Arc::new(InMemory::new()) as Arc<dyn object_store::ObjectStore>;
    Db::builder(ObjectPath::from("catalog"), store)
        .build()
        .await
        .unwrap()
}

#[tokio::test]
async fn malformed_import_leaves_target_unchanged() {
    let db = fresh_db().await;
    let input = [
        serde_json::json!({
            "table": "ducklake_snapshot",
            "data": {
                "snapshot_id": 1,
                "schema_version": 1,
                "snapshot_time": "2026-08-27T00:00:00Z"
            }
        })
        .to_string(),
        serde_json::json!({
            "table": "ducklake_table",
            "data": {"table_id": 2, "table_name": "broken"}
        })
        .to_string(),
    ]
    .join("\n");

    assert!(import_catalog(&db, BufReader::new(input.as_bytes()))
        .await
        .is_err());
    let mut rows = db.scan::<&[u8], _>(std::ops::RangeFull).await.unwrap();
    assert!(rows.next().await.unwrap().is_none());
    db.close().await.unwrap();
}

#[tokio::test]
async fn repair_rejects_bad_action_without_partial_write() {
    let db = fresh_db().await;
    rocklake_catalog::init::initialize_catalog(&db)
        .await
        .unwrap();
    let counter_key = keys::key_counter(tags::COUNTER_NEXT_SNAPSHOT_ID);
    let before = db.get(&counter_key).await.unwrap();

    let plan = RepairPlan {
        actions: vec![
            RepairAction::FixCounter {
                name: "next_snapshot_id".to_string(),
                current: 1,
                correct: 2,
            },
            RepairAction::RemoveDanglingStats {
                key_hex: "not-hex".to_string(),
            },
        ],
        unrecoverable_errors: Vec::new(),
    };
    assert!(repair_apply(&db, &plan).await.is_err());
    assert_eq!(db.get(&counter_key).await.unwrap(), before);
    db.close().await.unwrap();
}

#[tokio::test]
async fn verifier_reports_unknown_storage_tags() {
    let db = fresh_db().await;
    rocklake_catalog::init::initialize_catalog(&db)
        .await
        .unwrap();
    db.put([0x99, 0x01], values::encode_counter(1))
        .await
        .unwrap();

    let report = verify_catalog(&db).await.unwrap();
    assert!(!report.is_ok());
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("unknown catalog key tag")));
    db.close().await.unwrap();
}

#[tokio::test]
async fn sqlite_migration_reads_canonical_and_legacy_fields() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE ducklake_snapshot (snapshot_id INTEGER, schema_version INTEGER, snapshot_time TEXT, author TEXT, message TEXT);
             INSERT INTO ducklake_snapshot VALUES (1, 7, '2026-08-27T00:00:00Z', NULL, NULL);
             CREATE TABLE ducklake_schema (schema_id INTEGER, schema_name TEXT, begin_snapshot INTEGER, end_snapshot INTEGER);
             INSERT INTO ducklake_schema VALUES (1, 'main', 1, NULL);
             CREATE TABLE ducklake_table (table_id INTEGER, schema_id INTEGER, table_name TEXT, begin_snapshot INTEGER, end_snapshot INTEGER, data_path TEXT);
             INSERT INTO ducklake_table VALUES (2, 1, 'events', 1, NULL, 'events');
             CREATE TABLE ducklake_column (column_id INTEGER, table_id INTEGER, column_name TEXT, column_type TEXT, column_order INTEGER, begin_snapshot INTEGER, end_snapshot INTEGER, nulls_allowed INTEGER);
             INSERT INTO ducklake_column VALUES (3, 2, 'id', 'INTEGER', 0, 1, NULL, 0);
             CREATE TABLE ducklake_data_file (data_file_id INTEGER, table_id INTEGER, path TEXT, file_format TEXT, record_count INTEGER, file_size_bytes INTEGER, begin_snapshot INTEGER, end_snapshot INTEGER);
             INSERT INTO ducklake_data_file VALUES (4, 2, 'events/part.parquet', 'parquet', 10, 100, 1, NULL);",
        )
        .unwrap();
    let mut source = SqliteDuckLakeSource::from_connection(connection, None).unwrap();
    assert_eq!(
        source.catalog_version().unwrap(),
        DUCKLAKE_V1_0_CATALOG_VERSION
    );

    let db = fresh_db().await;
    let report = migrate_from_source(&mut source, &db, &[], false)
        .await
        .unwrap();
    assert_eq!(report.source_snapshot_id, 1);
    assert_eq!(report.data_file_count, 1);
    assert_eq!(report.total_skipped(), 0);
    db.close().await.unwrap();
}
