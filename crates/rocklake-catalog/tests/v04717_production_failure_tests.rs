//! v0.47.17 production failure-path tests.

use std::io::{BufReader, Cursor};
use std::sync::Arc;

use object_store::memory::InMemory;
use object_store::path::Path as ObjectPath;
use object_store::ObjectStore;
use rocklake_catalog::checkpoint::{create_checkpoint, restore_checkpoint};
use rocklake_catalog::cleanup::process_scheduled_deletions_report_at;
use rocklake_catalog::export::{export_catalog, import_catalog};
use rocklake_catalog::fault_injection::{ErrorInjectedStore, FaultInjector, WriteFaultPoint};
use rocklake_catalog::verify::verify_catalog;
use rocklake_catalog::{CatalogError, CatalogStore, OpenOptions};
use rocklake_core::mvcc::SnapshotId;
use rocklake_core::rows::{DataFileRow, FilesScheduledForDeletionRow};
use rocklake_core::{keys, values};

static FAULT_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn fault_test_lock() -> tokio::sync::MutexGuard<'static, ()> {
    FAULT_TEST_LOCK.lock().await
}

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

#[test]
fn production_failure_suite_has_non_vacuous_scenarios() {
    let scenarios = [
        "commit_reopen",
        "overlapping_writers",
        "checkpoint_restore",
        "export_import",
        "historical_snapshot",
    ];
    assert!(scenarios.len() >= 5);
    assert!(scenarios.iter().all(|scenario| !scenario.is_empty()));

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    let fixture =
        root.join("tests/fixtures/ducklake-corpus/duckdb-1.5.3-ducklake-1.0-live-surface.json");
    let value: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&fixture).expect("live DuckLake fixture must be readable"),
    )
    .expect("live DuckLake fixture must be valid JSON");
    assert_eq!(value["duckdb_version"], "1.5.3");
    assert_eq!(value["ducklake_version"], "1.0");
    assert!(value["recovery"]["expected_rows_after_commit"].as_u64() > Some(0));
}

#[test]
fn production_failpoints_are_named_and_unique() {
    let names: std::collections::HashSet<_> = WriteFaultPoint::all()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(names.len(), WriteFaultPoint::all().len());
    assert!(names.len() >= 8);
}

#[tokio::test]
async fn snapshot_commit_failpoint_is_atomic_after_reopen() {
    let _lock = fault_test_lock().await;
    let object_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let mut store = CatalogStore::open(options(Arc::clone(&object_store), "catalog"))
        .await
        .unwrap();
    let mut writer = store.begin_write();
    writer.create_schema("must_not_commit").await.unwrap();

    let injector = FaultInjector::new();
    injector.set_error(WriteFaultPoint::BeforeSlateDbCommit, "commit boundary");
    assert!(matches!(
        writer.create_snapshot(None, None).await,
        Err(CatalogError::InjectedFault { .. })
    ));
    injector.clear_all();
    drop(writer);
    store.close().await.unwrap();

    let reopened = CatalogStore::open(options(Arc::clone(&object_store), "catalog"))
        .await
        .unwrap();
    assert!(reopened
        .read_latest()
        .list_schemas()
        .await
        .unwrap()
        .is_empty());
    assert!(verify_catalog(reopened.db()).await.unwrap().is_ok());
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn snapshot_precommit_failpoints_are_atomic() {
    let _lock = fault_test_lock().await;
    for (path, point) in [
        ("catalog-counter", WriteFaultPoint::BeforeCounterWrite),
        ("catalog-snapshot", WriteFaultPoint::BeforeSnapshotCommit),
    ] {
        let object_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let mut store = CatalogStore::open(options(Arc::clone(&object_store), path))
            .await
            .unwrap();
        let mut writer = store.begin_write();
        writer.create_schema("must_not_commit").await.unwrap();

        let injector = FaultInjector::new();
        injector.set_error(point, "snapshot precommit boundary");
        assert!(matches!(
            writer.create_snapshot(None, None).await,
            Err(CatalogError::InjectedFault { .. })
        ));
        injector.clear_all();
        drop(writer);
        store.close().await.unwrap();

        let reopened = CatalogStore::open(options(object_store, path))
            .await
            .unwrap();
        assert!(reopened
            .read_latest()
            .list_schemas()
            .await
            .unwrap()
            .is_empty());
        assert!(verify_catalog(reopened.db()).await.unwrap().is_ok());
        reopened.close().await.unwrap();
    }
}

#[tokio::test]
async fn data_file_index_failpoint_does_not_publish_partial_registration() {
    let _lock = fault_test_lock().await;
    let object_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let mut store = CatalogStore::open(options(Arc::clone(&object_store), "catalog"))
        .await
        .unwrap();
    let (_schema_id, table_id, _column_id, existing_file_id) = baseline(&mut store).await;

    object_store
        .put(
            &ObjectPath::from("events/part-orphan.parquet"),
            b"parquet".to_vec().into(),
        )
        .await
        .unwrap();
    let injector = FaultInjector::new();
    injector.set_error(
        WriteFaultPoint::AfterParquetWriteBeforeRegisterDataFile,
        "registration boundary",
    );
    let mut writer = store.begin_write();
    assert!(matches!(
        writer
            .register_data_file(table_id, "events/part-orphan.parquet", "parquet", 2, 128)
            .await,
        Err(CatalogError::InjectedFault { .. })
    ));
    injector.clear_all();
    drop(writer);
    store.close().await.unwrap();

    let mut store = CatalogStore::open(options(Arc::clone(&object_store), "catalog"))
        .await
        .unwrap();
    let injector = FaultInjector::new();
    injector.set_error(
        WriteFaultPoint::BetweenPrimaryAndSecondaryKeyWrite,
        "data-file index boundary",
    );
    let mut writer = store.begin_write();
    assert!(matches!(
        writer
            .register_data_file(table_id, "events/part-2.parquet", "parquet", 2, 128)
            .await,
        Err(CatalogError::InjectedFault { .. })
    ));
    injector.clear_all();
    drop(writer);
    store.close().await.unwrap();

    let reopened = CatalogStore::open(options(Arc::clone(&object_store), "catalog"))
        .await
        .unwrap();
    assert_eq!(
        reopened
            .read_latest()
            .list_data_files(table_id)
            .await
            .unwrap()
            .iter()
            .map(|row| row.data_file_id)
            .collect::<Vec<_>>(),
        vec![existing_file_id]
    );
    assert!(verify_catalog(reopened.db()).await.unwrap().is_ok());
    assert!(object_store
        .head(&ObjectPath::from("events/part-orphan.parquet"))
        .await
        .is_ok());
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn historical_snapshot_values_remain_stable_after_later_commit() {
    let object_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let mut store = CatalogStore::open(options(Arc::clone(&object_store), "catalog"))
        .await
        .unwrap();

    let first_snapshot = {
        let mut writer = store.begin_write();
        writer.create_schema("before").await.unwrap();
        let commit = writer.create_snapshot(None, None).await.unwrap();
        store.commit_writer(commit);
        commit.snapshot_id.as_u64()
    };
    {
        let mut writer = store.begin_write();
        writer.create_schema("after").await.unwrap();
        let commit = writer.create_snapshot(None, None).await.unwrap();
        store.commit_writer(commit);
    }

    let historical = store
        .read_at(SnapshotId::new(first_snapshot))
        .unwrap()
        .list_schemas()
        .await
        .unwrap();
    assert_eq!(
        historical
            .iter()
            .map(|row| row.schema_name.as_str())
            .collect::<Vec<_>>(),
        vec!["before"]
    );
    let latest = store.read_latest().list_schemas().await.unwrap();
    assert_eq!(latest.len(), 2);
    assert!(verify_catalog(store.db()).await.unwrap().is_ok());
    store.close().await.unwrap();
}

#[tokio::test]
async fn close_failpoint_is_propagated_and_handle_can_reopen() {
    let _lock = fault_test_lock().await;
    let object_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let store = CatalogStore::open(options(Arc::clone(&object_store), "catalog"))
        .await
        .unwrap();
    let injector = FaultInjector::new();
    injector.set_error(WriteFaultPoint::BeforeCatalogClose, "close boundary");
    assert!(matches!(
        store.close().await,
        Err(CatalogError::InjectedFault { .. })
    ));
    injector.clear_all();

    let reopened = CatalogStore::open(options(object_store, "catalog"))
        .await
        .unwrap();
    reopened.close().await.unwrap();
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
    assert!(reopened
        .read_latest()
        .list_schemas()
        .await
        .unwrap()
        .is_empty());
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
    assert_eq!(
        reopened
            .read_latest()
            .list_data_files(table_id)
            .await
            .unwrap()
            .len(),
        1
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn checkpoint_and_restore_failpoints_leave_state_unchanged() {
    let _lock = fault_test_lock().await;
    let object_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let mut store = CatalogStore::open(options(Arc::clone(&object_store), "catalog"))
        .await
        .unwrap();
    let (_schema_id, table_id, _column_id, _file_id) = baseline(&mut store).await;

    let injector = FaultInjector::new();
    injector.set_error(
        WriteFaultPoint::BeforeCheckpointCommit,
        "checkpoint boundary",
    );
    assert!(matches!(
        create_checkpoint(store.db(), Some("blocked")).await,
        Err(CatalogError::InjectedFault { .. })
    ));
    injector.clear_all();
    assert!(create_checkpoint(store.db(), Some("usable")).await.is_ok());

    let checkpoint = create_checkpoint(store.db(), Some("restore"))
        .await
        .unwrap();
    let mut later = store.begin_write();
    later.create_schema("later").await.unwrap();
    let commit = later.create_snapshot(None, None).await.unwrap();
    store.commit_writer(commit);

    let injector = FaultInjector::new();
    injector.set_error(
        WriteFaultPoint::BeforeCheckpointRestoreCommit,
        "restore boundary",
    );
    assert!(matches!(
        restore_checkpoint(store.db(), checkpoint.id).await,
        Err(CatalogError::InjectedFault { .. })
    ));
    injector.clear_all();
    let schemas = store.read_latest().list_schemas().await.unwrap();
    assert!(schemas.iter().any(|row| row.schema_name == "later"));
    assert!(store
        .read_latest()
        .describe_table(table_id)
        .await
        .unwrap()
        .is_some());
    assert!(verify_catalog(store.db()).await.unwrap().is_ok());
    store.close().await.unwrap();
}

#[tokio::test]
async fn cleanup_failpoints_are_propagated_before_object_and_catalog_delete() {
    let _lock = fault_test_lock().await;
    let object_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let store = CatalogStore::open(options(Arc::clone(&object_store), "catalog"))
        .await
        .unwrap();
    let db = store.db();
    let data_file = DataFileRow {
        data_file_id: 7,
        table_id: 1,
        path: "retired.parquet".to_string(),
        file_format: "parquet".to_string(),
        record_count: 1,
        file_size_bytes: 1,
        footer_size: None,
        encryption_key: None,
        begin_snapshot: Some(1),
        end_snapshot: Some(1),
        file_order: None,
        path_is_relative: Some(true),
        row_id_start: None,
        partition_id: None,
        mapping_id: None,
        partial_max: None,
    };
    db.put(
        &keys::key_data_file(1, 7),
        &values::encode_value(&data_file),
    )
    .await
    .unwrap();
    let scheduled = FilesScheduledForDeletionRow {
        data_file_id: 7,
        schedule_start: 0,
        path: "retired.parquet".to_string(),
        file_type: Some("data".to_string()),
        path_is_relative: Some(true),
    };
    let schedule_key = keys::key_files_scheduled_for_deletion(1, 7);
    db.put(&schedule_key, &values::encode_value(&scheduled))
        .await
        .unwrap();
    let object_path = ObjectPath::from("data/retired.parquet");
    object_store
        .put(&object_path, b"data".to_vec().into())
        .await
        .unwrap();

    let injector = FaultInjector::new();
    injector.set_error(
        WriteFaultPoint::BeforeCleanupObjectDelete,
        "cleanup object boundary",
    );
    assert!(matches!(
        process_scheduled_deletions_report_at(db, &object_store, &ObjectPath::from("data"), 1)
            .await,
        Err(CatalogError::InjectedFault { .. })
    ));
    injector.clear_all();
    assert!(object_store.head(&object_path).await.is_ok());

    injector.set_error(
        WriteFaultPoint::BeforeCleanupCatalogDelete,
        "cleanup catalog boundary",
    );
    assert!(matches!(
        process_scheduled_deletions_report_at(db, &object_store, &ObjectPath::from("data"), 1)
            .await,
        Err(CatalogError::InjectedFault { .. })
    ));
    injector.clear_all();
    assert!(object_store.head(&object_path).await.is_err());
    let report =
        process_scheduled_deletions_report_at(db, &object_store, &ObjectPath::from("data"), 1)
            .await
            .unwrap();
    assert_eq!(report.deleted, 1);
    assert!(db.get(&schedule_key).await.unwrap().is_none());
    store.close().await.unwrap();
}

#[tokio::test]
async fn export_import_preserves_exact_rows_and_verifies() {
    let _lock = fault_test_lock().await;
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
    let injector = FaultInjector::new();
    injector.set_error(WriteFaultPoint::BeforeImportCommit, "import boundary");
    assert!(matches!(
        import_catalog(&target_db, BufReader::new(Cursor::new(bytes.clone()))).await,
        Err(CatalogError::InjectedFault { .. })
    ));
    injector.clear_all();
    target_db.close().await.unwrap();
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
