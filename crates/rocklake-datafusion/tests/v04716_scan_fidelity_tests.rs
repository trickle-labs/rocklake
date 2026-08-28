use datafusion::prelude::SessionContext;
use object_store::path::Path as ObjectPath;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_core::mvcc::SnapshotId;
use rocklake_datafusion::RockLakeCatalogProvider;
use std::sync::Arc;
use tempfile::TempDir;

fn local_opts(dir: &TempDir) -> OpenOptions {
    OpenOptions {
        object_store: Arc::new(
            object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap(),
        ),
        path: ObjectPath::from("catalog"),
        encryption: None,
    }
}

async fn table_store(dir: &TempDir) -> (CatalogStore, u64) {
    let mut store = CatalogStore::open(local_opts(dir)).await.unwrap();
    let mut writer = store.begin_write();
    let schema_id = writer.create_schema("main").await.unwrap();
    let table_id = writer
        .create_table(schema_id, "events", None)
        .await
        .unwrap();
    writer
        .add_column(table_id, "id", "INTEGER", 0, false, None)
        .await
        .unwrap();
    let commit = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(commit);
    (store, table_id)
}

async fn query_rows(provider: RockLakeCatalogProvider) -> datafusion::error::Result<usize> {
    let ctx = SessionContext::new();
    ctx.register_catalog("duck", Arc::new(provider));
    let batches = ctx
        .sql("SELECT id FROM duck.main.events")
        .await?
        .collect()
        .await?;
    Ok(batches.iter().map(|batch| batch.num_rows()).sum())
}

#[tokio::test]
async fn empty_table_is_the_only_empty_scan() {
    let dir = TempDir::new().unwrap();
    let (store, _) = table_store(&dir).await;

    assert_eq!(
        query_rows(RockLakeCatalogProvider::new(store, Some(SnapshotId::new(1))).unwrap())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn unsupported_registered_format_is_an_error() {
    let dir = TempDir::new().unwrap();
    let (mut store, table_id) = table_store(&dir).await;
    let mut writer = store.begin_write();
    writer
        .register_data_file(table_id, "events/file.csv", "csv", 1, 1)
        .await
        .unwrap();
    let commit = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(commit);

    let error = query_rows(RockLakeCatalogProvider::new(store, Some(SnapshotId::new(2))).unwrap())
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("csv"), "unexpected error: {error}");
}

#[tokio::test]
async fn visible_delete_and_inlined_rows_are_explicitly_unsupported() {
    let dir = TempDir::new().unwrap();
    let (mut store, table_id) = table_store(&dir).await;
    let mut writer = store.begin_write();
    let data_file_id = writer
        .register_data_file(table_id, "events/file.parquet", "parquet", 1, 1)
        .await
        .unwrap();
    writer
        .register_delete_file(data_file_id, "events/delete.parquet", 1, 1)
        .await
        .unwrap();
    let commit = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(commit);

    let error = query_rows(RockLakeCatalogProvider::new(store, Some(SnapshotId::new(2))).unwrap())
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("delete files"), "unexpected error: {error}");

    let dir = TempDir::new().unwrap();
    let (mut store, table_id) = table_store(&dir).await;
    let mut writer = store.begin_write();
    writer
        .register_inlined_insert(table_id, 1, 1, b"[\"1\"]".to_vec())
        .await
        .unwrap();
    let commit = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(commit);

    let error = query_rows(RockLakeCatalogProvider::new(store, Some(SnapshotId::new(2))).unwrap())
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("inlined inserts"),
        "unexpected error: {error}"
    );

    let dir = TempDir::new().unwrap();
    let (mut store, table_id) = table_store(&dir).await;
    let mut writer = store.begin_write();
    writer
        .register_inlined_delete(table_id, 0, 1)
        .await
        .unwrap();
    let commit = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(commit);

    let error = query_rows(RockLakeCatalogProvider::new(store, Some(SnapshotId::new(2))).unwrap())
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("inlined deletes"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn absolute_registered_paths_do_not_require_data_path() {
    use arrow::array::Int32Array;
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow::record_batch::RecordBatch;
    use parquet::arrow::ArrowWriter;

    let dir = TempDir::new().unwrap();
    let data_path = dir.path().join("absolute.parquet");
    let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int32, false)]));
    let batch =
        RecordBatch::try_new(schema.clone(), vec![Arc::new(Int32Array::from(vec![7]))]).unwrap();
    let file = std::fs::File::create(&data_path).unwrap();
    let mut parquet = ArrowWriter::try_new(file, schema, None).unwrap();
    parquet.write(&batch).unwrap();
    parquet.close().unwrap();

    let (mut store, table_id) = table_store(&dir).await;
    let mut writer = store.begin_write();
    writer
        .register_data_file(table_id, data_path.to_str().unwrap(), "parquet", 1, 1)
        .await
        .unwrap();
    let commit = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(commit);

    assert_eq!(
        query_rows(RockLakeCatalogProvider::new(store, Some(SnapshotId::new(2))).unwrap())
            .await
            .unwrap(),
        1
    );
}
