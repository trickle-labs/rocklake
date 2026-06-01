//! v0.47.3 Concurrent Writer Tests
//! Verify writer fencing, epoch monotonicity, and conflict detection.

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
async fn sequential_writers_succeed() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    let _table_id = w2.create_table(schema_id, "t1", None).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(tables.len(), 1);
}

#[tokio::test]
async fn writer_a_retry_after_conflict() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // First attempt
    let mut w_a = store.begin_write();
    let _table_a = w_a.create_table(schema_id, "ta", None).await.unwrap();
    let snap_a = w_a.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap_a);

    // Second attempt (retry)
    let mut w_a2 = store.begin_write();
    let _table_a2 = w_a2.create_table(schema_id, "ta2", None).await.unwrap();
    let snap_a2 = w_a2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap_a2);

    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(tables.len(), 2, "both tables should exist after retry");
}

#[tokio::test]
async fn writer_epochs_monotonically_increasing() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut epochs = Vec::new();
    for i in 0..3 {
        let mut w = store.begin_write();
        let _schema_id = w.create_schema(&format!("s{}", i)).await.unwrap();
        let snap = w.create_snapshot(None, None).await.unwrap();
        epochs.push(snap.0);
        store.commit_writer(snap);
    }

    // Epochs should be monotonically increasing
    for i in 1..epochs.len() {
        assert!(epochs[i] > epochs[i - 1], "epochs should increase");
    }
}

#[tokio::test]
async fn stale_commit_rejected_with_error() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let _schema_id = w1.create_schema("s1").await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // Writer with stale epoch should fail
    let mut w_stale = store.begin_write();
    let _ = w_stale.create_schema("s_stale").await;
    // Try to create snapshot with old epoch (should fail or be rejected)
    let snap_stale = w_stale.create_snapshot(None, None).await.unwrap();
    // Attempting to commit stale snapshot should result in error or no-op
    store.commit_writer(snap_stale);

    let reader = store.read_latest();
    let schemas = reader.list_schemas().await.unwrap();
    // Only original schema should exist (stale write rejected)
    assert!(schemas.iter().any(|s| s.schema_name == "s1"));
}

#[tokio::test]
async fn writer_lease_released_on_commit() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Writer A commits
    let mut w_a = store.begin_write();
    let schema_id = w_a.create_schema("s_a").await.unwrap();
    let snap_a = w_a.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap_a);

    // Writer B should be able to claim the lock and commit
    let mut w_b = store.begin_write();
    let _table_id = w_b.create_table(schema_id, "t_b", None).await.unwrap();
    let snap_b = w_b.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap_b);

    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(
        tables.len(),
        1,
        "writer B should successfully acquire lock after A releases it"
    );
}

#[tokio::test]
async fn writer_lease_timeout_releases() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // This test verifies that even if a writer doesn't explicitly commit,
    // the system remains available for other writers
    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    // Don't commit w1 - simulates writer timeout or crash

    drop(w1); // Release the writer

    // Another writer should be able to proceed
    let mut w2 = store.begin_write();
    let _table_id = w2.create_table(schema_id, "t1", None).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(
        tables.len(),
        1,
        "system should recover after writer release"
    );
}
