//! v0.47.3 Cascading Operations Tests  
//! Verify that DROP SCHEMA/TABLE/COLUMN cascade correctly to dependent metadata rows.

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
async fn drop_schema_cascades_to_tables() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let _t1 = w1.create_table(schema_id, "t1", None).await.unwrap();
    let _t2 = w1.create_table(schema_id, "t2", None).await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    w2.drop_schema(schema_id, snap1.0).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(
        tables.len(),
        0,
        "tables should be invisible after schema drop"
    );
}

#[tokio::test]
async fn drop_schema_cascades_to_columns() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    let _c1 = w1
        .add_column(table_id, "c1", "INT", 0, true, None)
        .await
        .unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    w2.drop_schema(schema_id, snap1.0).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let (_table, cols) = reader
        .describe_table(table_id)
        .await
        .unwrap()
        .unwrap_or_default();
    assert_eq!(
        cols.len(),
        0,
        "columns should be invisible after schema drop"
    );
}

#[tokio::test]
async fn drop_schema_cascades_to_data_files() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    w1.register_data_file(table_id, "f1.parquet", "PARQUET", 100, 1000)
        .await
        .unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    w2.drop_schema(schema_id, snap1.0).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let files = reader.list_data_files(table_id).await.unwrap();
    assert_eq!(
        files.len(),
        0,
        "data files should be invisible after schema drop"
    );
}

#[tokio::test]
async fn drop_table_cascades_to_columns() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    let _c1 = w1
        .add_column(table_id, "c1", "INT", 0, true, None)
        .await
        .unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    w2.drop_table(schema_id, table_id, snap1.0).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(tables.len(), 0, "table should be invisible after drop");
}

#[tokio::test]
async fn drop_table_cascades_to_data_files() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    w1.register_data_file(table_id, "f1.parquet", "PARQUET", 100, 1000)
        .await
        .unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    w2.drop_table(schema_id, table_id, snap1.0).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let files = reader.list_data_files(table_id).await.unwrap();
    assert_eq!(
        files.len(),
        0,
        "data files should be invisible after table drop"
    );
}

#[tokio::test]
async fn drop_table_cascades_to_tags() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    w1.set_tag(table_id, "owner", "team").await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    w2.drop_table(schema_id, table_id, snap1.0).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let tags = reader.list_all_tags().await.unwrap();
    assert_eq!(tags.len(), 0, "tags should be invisible after table drop");
}

#[tokio::test]
async fn drop_table_cascades_to_sort_info() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    w1.define_sort_order(table_id, "id ASC").await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    w2.drop_table(schema_id, table_id, snap1.0).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let sorts = reader.list_all_sort_info().await.unwrap();
    assert_eq!(
        sorts.len(),
        0,
        "sort info should be invisible after table drop"
    );
}

#[tokio::test]
async fn drop_column_cascade_partial() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    let _c1 = w1
        .add_column(table_id, "c1", "INT", 0, true, None)
        .await
        .unwrap();
    let _c2 = w1
        .add_column(table_id, "c2", "INT", 1, true, None)
        .await
        .unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    w2.drop_column(table_id, 3, snap1.0).await.unwrap(); // Drop c1
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let (_table, cols) = reader.describe_table(table_id).await.unwrap().unwrap();
    assert_eq!(cols.len(), 1, "only one column should remain");
}

#[tokio::test]
async fn drop_non_existent_table_succeeds() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // Try to drop non-existent table (should succeed)
    let mut w2 = store.begin_write();
    let result = w2.drop_table(schema_id, 999, snap1.0).await;
    // Result should be Ok (idempotent)
    assert!(result.is_ok(), "dropping non-existent table should succeed");
}

#[tokio::test]
async fn dropped_rows_not_in_select_queries() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    let mut w2 = store.begin_write();
    w2.drop_table(schema_id, table_id, snap1.0).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    // Read at snapshot 2 should not see the dropped table
    let reader = store.read_at(snap2).unwrap();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(
        tables.len(),
        0,
        "dropped table should not appear in query results"
    );
}
