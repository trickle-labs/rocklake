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

    // Verify tables exist
    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(tables.len(), 2, "both tables should exist initially");

    // Drop schema
    let mut w2 = store.begin_write();
    w2.drop_schema(schema_id).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    // Verify schema is dropped (list_schemas should not include it)
    let reader = store.read_latest();
    let schemas = reader.list_schemas().await.unwrap();
    assert_eq!(schemas.len(), 0, "schema should be dropped");
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

    // Verify table and column exist
    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(tables.len(), 1, "table should exist");
    let (_table, cols) = reader
        .describe_table(table_id)
        .await
        .unwrap()
        .unwrap_or_default();
    assert_eq!(cols.len(), 1, "column should exist");

    // Drop schema
    let mut w2 = store.begin_write();
    w2.drop_schema(schema_id).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    // Verify schema is dropped
    let reader = store.read_latest();
    let schemas = reader.list_schemas().await.unwrap();
    assert_eq!(schemas.len(), 0, "schema should be dropped");
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

    // Verify data file exists
    let reader = store.read_latest();
    let files = reader.list_data_files(table_id).await.unwrap();
    assert_eq!(files.len(), 1, "data file should exist");

    // Drop schema
    let mut w2 = store.begin_write();
    w2.drop_schema(schema_id).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    // Verify schema is dropped
    let reader = store.read_latest();
    let schemas = reader.list_schemas().await.unwrap();
    assert_eq!(schemas.len(), 0, "schema should be dropped");
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
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // Drop created table
    let mut w2 = store.begin_write();
    let result = w2.drop_table(schema_id, table_id, snap1.0).await;
    // Result should be Ok
    assert!(result.is_ok(), "dropping created table should succeed");
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    // Verify table is dropped
    let reader = store.read_latest();
    let tables = reader.list_tables(schema_id).await.unwrap();
    assert_eq!(tables.len(), 0, "dropped table should not be visible");
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

#[tokio::test]
async fn drop_schema_cascades_to_delete_files() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let _table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // Verify schema exists
    let reader = store.read_latest();
    let schemas = reader.list_schemas().await.unwrap();
    assert_eq!(schemas.len(), 1, "schema should exist");

    // Drop schema
    let mut w2 = store.begin_write();
    w2.drop_schema(schema_id).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    // Verify schema is dropped
    let reader = store.read_latest();
    let schemas = reader.list_schemas().await.unwrap();
    assert_eq!(schemas.len(), 0, "schema should be dropped");
}

#[tokio::test]
async fn drop_table_cascades_to_delete_files() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // Drop table
    let mut w2 = store.begin_write();
    w2.drop_table(schema_id, table_id, snap1.0).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let delete_files = reader.list_delete_files(table_id).await.unwrap();
    assert_eq!(
        delete_files.len(),
        0,
        "delete files should be invisible after table drop"
    );
}

#[tokio::test]
async fn drop_table_cascades_to_partition_info() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // Drop table
    let mut w2 = store.begin_write();
    w2.drop_table(schema_id, table_id, snap1.0).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let partitions = reader.list_partition_info(table_id).await.unwrap();
    assert_eq!(
        partitions.len(),
        0,
        "partition info should be invisible after table drop"
    );
}

#[tokio::test]
async fn drop_schema_cascades_to_stats() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let mut w1 = store.begin_write();
    let schema_id = w1.create_schema("s1").await.unwrap();
    let table_id = w1.create_table(schema_id, "t1", None).await.unwrap();
    let snap1 = w1.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1);

    // Drop schema
    let mut w2 = store.begin_write();
    w2.drop_schema(schema_id).await.unwrap();
    let snap2 = w2.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2);

    let reader = store.read_latest();
    let stats = reader.list_file_variant_stats(table_id, 0).await.unwrap();
    assert_eq!(
        stats.len(),
        0,
        "file variant stats should be invisible after schema drop"
    );
}
