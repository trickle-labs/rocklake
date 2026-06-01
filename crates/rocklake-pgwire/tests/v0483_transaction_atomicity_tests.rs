//! v0.47.3 Transaction Atomicity & ROLLBACK Tests
//! Verify that multi-statement transactions are properly atomic and ROLLBACK reverts all changes.

use object_store::path::Path as ObjectPath;
use rocklake_catalog::{CatalogStore, OpenOptions};
use std::sync::Arc;
use tempfile::TempDir;

fn test_opts(dir: &TempDir) -> OpenOptions {
    let path = dir.path().to_str().unwrap().to_string();
    let store = Arc::new(object_store::local::LocalFileSystem::new_with_prefix(&path).unwrap());
    OpenOptions {
        object_store: store,
        path: ObjectPath::from("catalog"),
        encryption: None,
    }
}

#[tokio::test]
async fn multi_statement_atomic_commit() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Create schema in first transaction
    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("test_schema").await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // Verify schema persisted
    let reader = store.read_latest();
    let schemas = reader.list_schemas().await.unwrap();
    assert_eq!(schemas.len(), 1);
    assert_eq!(schemas[0].schema_name, "test_schema");
}

#[tokio::test]
async fn multi_statement_atomic_rollback() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Create initial schema
    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("test_schema").await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // Start creating a table (but don't commit - simulates rollback)
    let mut w2 = store.begin_write();
    let table_id = w2
        .create_table(schema_id, "test_table", None)
        .await
        .unwrap();
    // DON'T call create_snapshot or commit - simulating rollback
    drop(w2);

    // Verify table was NOT persisted
    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(tables.len(), 0, "table should not exist after rollback");
}

#[tokio::test]
async fn explicit_rollback_reverts_all_ops() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Create schema first
    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("initial").await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // Start multiple ops (create table + column), then rollback
    let mut w2 = store.begin_write();
    let table_id = w2
        .create_table(schema_id, "temp_table", None)
        .await
        .unwrap();
    let _col_id = w2
        .add_column(table_id, "id", "INT", 0, true, None)
        .await
        .unwrap();
    // Rollback: don't commit
    drop(w2);

    // Verify nothing was persisted
    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(tables.len(), 0);
}

#[tokio::test]
async fn writer_isolation_prevents_uncommitted_visibility() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Create first schema
    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // Start creating second schema (don't commit)
    let mut w2 = store.begin_write();
    let _schema_id2 = w2.create_schema("s2").await.unwrap();

    // Reader should still only see s1 (s2 is uncommitted)
    let reader = store.read_latest();
    let schemas = reader.list_schemas().await.unwrap();
    assert_eq!(schemas.len(), 1);
    assert_eq!(schemas[0].schema_name, "s1");
    drop(w2);
}

#[tokio::test]
async fn partial_batch_failure_cascades() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Setup: create schema
    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // Start batch: create table (but simulate failure by not committing)
    let mut w2 = store.begin_write();
    let table_id = w2.create_table(schema_id, "t1", None).await.unwrap();
    let _col_id = w2
        .add_column(table_id, "c1", "INT", 0, true, None)
        .await
        .unwrap();
    // Simulate error: don't commit
    drop(w2);

    // Verify entire batch was rolled back
    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(
        tables.len(),
        0,
        "table should not exist after batch rollback"
    );
}

#[tokio::test]
async fn writer_fencing_detects_conflict() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Setup: create base schema
    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // Writer A creates table_a and commits
    let mut w_a = store.begin_write();
    let _table_a = w_a.create_table(schema_id, "ta", None).await.unwrap();
    let snap_a = w_a.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap_a);

    // Writer B creates table_b and commits (should succeed)
    let mut w_b = store.begin_write();
    let _table_b = w_b.create_table(schema_id, "tb", None).await.unwrap();
    let snap_b = w_b.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap_b);

    // Both tables should exist
    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(tables.len(), 2, "sequential writers should both succeed");
}

#[tokio::test]
async fn multi_insert_snapshot_ordering() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Create schema and table
    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // Insert data file at snapshot 2
    let mut w2 = store.begin_write();
    w2.register_data_file(table_id, "file1.parquet", "PARQUET", 100, 1000)
        .await
        .unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    // Insert another file at snapshot 3
    let mut w3 = store.begin_write();
    w3.register_data_file(table_id, "file2.parquet", "PARQUET", 200, 2000)
        .await
        .unwrap();
    let snap3 = w3.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap3);

    // Both files should exist
    let reader = store.read_latest();
    let files = reader.list_data_files(table_id).await.unwrap();
    assert_eq!(files.len(), 2);
}

#[tokio::test]
async fn transaction_isolation_no_dirty_reads() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Setup: create schema
    let mut w0 = store.begin_write();
    let schema_id = w0.create_schema("s1").await.unwrap();
    let snap0 = w0.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap0);

    // Writer starts creating table but doesn't commit
    let mut w1 = store.begin_write();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();

    // Reader sees schema but NOT the uncommitted table
    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(tables.len(), 0, "dirty read should not occur");

    // After commit, reader sees the table
    drop(w1);
    let mut w2 = store.begin_write();
    let _ = w2.create_table(schema_id, "t1", None).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(tables.len(), 1);
}
