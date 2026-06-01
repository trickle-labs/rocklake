//! v0.47.3 MVCC Visibility Filtering Tests
//! Verify that snapshot-based visibility filtering works correctly for all catalog tables.

use object_store::path::Path as ObjectPath;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_core::mvcc::SnapshotId;
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
async fn schema_visible_at_begin_snapshot() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w = store.begin_write();
    let schema_id = w.create_schema("test").await.unwrap();
    let snap = w.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap);

    let reader = store.read_latest();
    let schemas = reader.list_schemas().await.unwrap();
    assert_eq!(schemas.len(), 1);
    assert_eq!(schemas[0].begin_snapshot, 1);
}

#[tokio::test]
async fn table_visible_at_begin_snapshot() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    let table_id = w2.create_table(schema_id, "t1", None).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].begin_snapshot, 2);
}

#[tokio::test]
async fn column_visible_at_begin_snapshot() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    let _col_id = w2
        .add_column(table_id, "c1", "INT", 0, true, None)
        .await
        .unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let (_table, cols) = reader.describe_table(table_id).await.unwrap().unwrap();
    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].begin_snapshot, 2);
}

#[tokio::test]
async fn data_file_visible_at_begin_snapshot() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    w2.register_data_file(table_id, "file.parquet", "PARQUET", 100, 1000)
        .await
        .unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let files = reader.list_data_files(table_id).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].begin_snapshot, 2);
}

#[tokio::test]
async fn tag_visible_at_begin_snapshot() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    w2.set_tag(table_id, "owner", "team-a").await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let tags = reader.list_all_tags().await.unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].begin_snapshot, 2);
}

#[tokio::test]
async fn sort_info_visible_at_begin_snapshot() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    w2.define_sort_order(table_id, "timestamp DESC NULLS LAST")
        .await
        .unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let sorts = reader.list_all_sort_info().await.unwrap();
    assert_eq!(sorts.len(), 1);
    assert_eq!(sorts[0].begin_snapshot, 2);
}

#[tokio::test]
async fn view_visible_at_begin_snapshot() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    w2.create_view_with_opts(schema_id, "v1", "SELECT 1", None, None)
        .await
        .unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let views = reader.list_all_views(schema_id).await.unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].begin_snapshot, 2);
}

#[tokio::test]
async fn snapshot_ordering_monotonic() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut snaps = Vec::new();
    for i in 0..5 {
        let mut w = store.begin_write();
        let schema_id = w.create_schema(&format!("s{}", i)).await.unwrap();
        let snap = w.create_snapshot(None, None).await.unwrap();
        snaps.push(snap);
        store.commit_writer(snap);
    }

    // Verify snapshot IDs are monotonically increasing
    for i in 1..snaps.len() {
        assert!(snaps[i].0 > snaps[i - 1].0, "snapshot IDs should increase");
    }
}
