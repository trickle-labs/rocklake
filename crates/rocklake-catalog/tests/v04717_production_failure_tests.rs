//! v0.47.17 production failure-path tests.

use std::io::{BufReader, Cursor};
use std::sync::Arc;

use object_store::memory::InMemory;
use object_store::path::Path as ObjectPath;
use object_store::ObjectStore;
use rocklake_catalog::checkpoint::{create_checkpoint, restore_checkpoint};
use rocklake_catalog::export::{export_catalog, import_catalog};
use rocklake_catalog::fault_injection::ErrorInjectedStore;
use rocklake_catalog::verify::verify_catalog;
use rocklake_catalog::{CatalogError, CatalogStore, OpenOptions};

fn options(store: Arc<dyn ObjectStore>, path: &str) -> OpenOptions {
    OpenOptions {
        object_store: store,
        path: ObjectPath::from(path),
        encryption: None,
    }
}

async fn baseline(store: &mut CatalogStore) -> (u64, u64, u64, u64) {
    let mut writer = store.begin_write();
    let schema_id = writer.create_schema("main").await.unwrap();
    let table_id = writer
        .create_table(schema_id, "events", Some("events"))
        .await
        .unwrap();
    let column_id = writer
        .add_column(table_id, "id", "BIGINT", 0, false, None)
        .await
        .unwrap();
    let file_id = writer
        .register_data_file(table_id, "events/part-1.parquet", "parquet", 3, 256)
        .await
        .unwrap();
    let commit = writer
        .create_snapshot(Some("test"), Some("baseline"))
        .await
        .unwrap();
    store.commit_writer(commit);
    (schema_id, table_id, column_id, file_id)
}

#[tokio::test]
async fn injected_object_store_failure_is_not_silent_on_reopen() {
    let inner: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let injected = Arc::new(ErrorInjectedStore::new(Arc::clone(&inner)));
    injected.inject_put_error("injected SlateDB commit failure");
    assert!(injected
        .put(
            &ObjectPath::from("catalog/failure-marker"),
            b"failure".to_vec().into()
        )
        .await
        .is_err());

    let reopened = CatalogStore::open(options(inner, "catalog")).await.unwrap();
    assert!(reopened.read_latest().list_schemas().await.unwrap().is_empty());
    assert!(verify_catalog(reopened.db()).await.unwrap().is_ok());
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn overlapping_writers_fence_stale_commit() {
    let object_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let opts = || options(Arc::clone(&object_store), "catalog");
    let mut first = CatalogStore::open(opts()).await.unwrap();
    let mut second = CatalogStore::open(opts()).await.unwrap();

    let mut stale_writer = first.begin_write();
    stale_writer.create_schema("stale").await.unwrap();
    assert!(matches!(
        stale_writer.create_snapshot(None, None).await,
        Err(CatalogError::WriterEpochMismatch | CatalogError::TransactionConflict(_))
    ));

    let mut current_writer = second.begin_write();
    let schema_id = current_writer.create_schema("current").await.unwrap();
    let commit = current_writer.create_snapshot(None, None).await.unwrap();
    second.commit_writer(commit);
    let schemas = second.read_latest().list_schemas().await.unwrap();
    assert_eq!(schemas.len(), 1);
    assert_eq!(schemas[0].schema_id, schema_id);
    assert_eq!(schemas[0].schema_name, "current");

    drop(first);
    second.close().await.unwrap();
}

#[tokio::test]
async fn checkpoint_restore_recovers_exact_rows_and_counters() {
    let object_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let mut store = CatalogStore::open(options(Arc::clone(&object_store), "catalog"))
        .await
        .unwrap();
    let (schema_id, table_id, column_id, file_id) = baseline(&mut store).await;
    let snapshot_id = store.latest_committed_snapshot_id();
    assert_eq!(snapshot_id, 1);
    let checkpoint = create_checkpoint(store.db(), Some("baseline"))
        .await
        .unwrap();

    let mut writer = store.begin_write();
    writer.create_schema("later").await.unwrap();
    writer.mark_data_file_deleted(file_id).await.unwrap();
    let later = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(later);
    store.close().await.unwrap();

    let db = slatedb::Db::open(ObjectPath::from("catalog"), Arc::clone(&object_store))
        .await
        .unwrap();
    let restored = restore_checkpoint(&db, checkpoint.id).await.unwrap();
    let restore_id = restored.restore_snapshot_id.unwrap();
    db.close().await.unwrap();

    let reopened = CatalogStore::open(options(Arc::clone(&object_store), "catalog"))
        .await
        .unwrap();
    assert_eq!(reopened.latest_committed_snapshot_id(), restore_id);
    let schemas = reopened.read_latest().list_schemas().await.unwrap();
    assert_eq!(schemas.len(), 1);
    assert_eq!(schemas[0].schema_id, schema_id);
    assert_eq!(schemas[0].schema_name, "main");
    let (table, columns) = reopened
        .read_latest()
        .describe_table(table_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(table.table_name, "events");
    assert_eq!(columns.len(), 1);
    assert_eq!(columns[0].column_id, column_id);
    assert_eq!(reopened.read_latest().list_data_files(table_id).await.unwrap().len(), 1);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn export_import_preserves_exact_rows_and_verifies() {
    let source_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let mut source = CatalogStore::open(options(Arc::clone(&source_store), "catalog"))
        .await
        .unwrap();
    let (schema_id, table_id, column_id, _) = baseline(&mut source).await;
    assert!(verify_catalog(source.db()).await.unwrap().is_ok());
    let expected = source.read_latest().describe_table(table_id).await.unwrap();
    let mut bytes = Vec::new();
    let exported = export_catalog(source.db(), None, &mut bytes).await.unwrap();
    assert!(exported.rows_exported >= 4);
    source.close().await.unwrap();

    let target_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let target_db = slatedb::Db::builder(ObjectPath::from("catalog"), Arc::clone(&target_store))
        .build()
        .await
        .unwrap();
    let imported = import_catalog(&target_db, BufReader::new(Cursor::new(bytes)))
        .await
        .unwrap();
    assert_eq!(imported.rows_imported, exported.rows_exported);
    target_db.close().await.unwrap();
    let target = CatalogStore::open(options(target_store, "catalog"))
        .await
        .unwrap();
    let actual = target.read_latest().describe_table(table_id).await.unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.unwrap().1[0].column_id, column_id);
    assert_eq!(schema_id, 1);
    target.close().await.unwrap();
}
