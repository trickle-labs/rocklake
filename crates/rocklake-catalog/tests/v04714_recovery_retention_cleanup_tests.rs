//! v0.47.14 recovery, retention, and cleanup safety gates.

use object_store::memory::InMemory;
use object_store::path::Path as ObjectPath;
use object_store::ObjectStore;
use rocklake_catalog::checkpoint::{create_checkpoint, restore_checkpoint};
use rocklake_catalog::cleanup::{orphaned_file_sweep, process_scheduled_deletions_report_at};
use rocklake_catalog::error::CatalogError;
use rocklake_catalog::excise::{excise_apply, excise_plan};
use rocklake_catalog::gc::{gc_apply, pin_snapshot, unpin_snapshot};
use rocklake_catalog::lease::hold_snapshot;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_core::keys;
use rocklake_core::rows::{DataFileRow, FilesScheduledForDeletionRow};
use rocklake_core::values;
use std::sync::Arc;
use tempfile::TempDir;

fn in_memory_options(name: &str, object_store: Arc<dyn ObjectStore>) -> OpenOptions {
    OpenOptions {
        object_store,
        path: ObjectPath::from(format!("v04714-{name}")),
        encryption: None,
    }
}

#[tokio::test]
async fn checkpoint_restore_round_trip_restores_retired_and_later_state() {
    let dir = TempDir::new().unwrap();
    let object_store: Arc<dyn ObjectStore> =
        Arc::new(object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap());
    let opts = OpenOptions {
        object_store: Arc::clone(&object_store),
        path: ObjectPath::from("catalog"),
        encryption: None,
    };
    let mut store = CatalogStore::open(opts.clone()).await.unwrap();

    let (schema_id, table_id, data_file_id) = {
        let mut writer = store.begin_write();
        let schema_id = writer.create_schema("before_restore").await.unwrap();
        let table_id = writer.create_table(schema_id, "facts", None).await.unwrap();
        let data_file_id = writer
            .register_data_file_with_metadata(
                table_id,
                "data/facts.parquet",
                "parquet",
                2,
                128,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let commit = writer
            .create_snapshot(None, Some("baseline"))
            .await
            .unwrap();
        store.commit_writer(commit);
        (schema_id, table_id, data_file_id)
    };
    let checkpoint = create_checkpoint(store.db(), Some("baseline"))
        .await
        .unwrap();

    {
        let mut writer = store.begin_write();
        writer.create_schema("after_checkpoint").await.unwrap();
        writer.mark_data_file_deleted(data_file_id).await.unwrap();
        let commit = writer
            .create_snapshot(None, Some("later state"))
            .await
            .unwrap();
        store.commit_writer(commit);
    }
    store.close().await.unwrap();

    let db = slatedb::Db::open(ObjectPath::from("catalog"), object_store)
        .await
        .unwrap();
    let restored = restore_checkpoint(&db, checkpoint.id).await.unwrap();
    let restore_snapshot = restored.restore_snapshot_id.unwrap();
    assert!(restore_snapshot > checkpoint.snapshot_id);
    db.close().await.unwrap();

    let reopened = CatalogStore::open(opts).await.unwrap();
    assert_eq!(reopened.latest_committed_snapshot_id(), restore_snapshot);
    let reader = reopened
        .read_at(rocklake_core::mvcc::SnapshotId::new(restore_snapshot))
        .unwrap();
    assert_eq!(reader.list_schemas().await.unwrap().len(), 1);
    assert!(reader
        .list_schemas()
        .await
        .unwrap()
        .iter()
        .any(|row| row.schema_id == schema_id));
    assert_eq!(reader.list_data_files(table_id).await.unwrap().len(), 1);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn pins_and_leases_bound_gc_without_off_by_one() {
    let object_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let mut store = CatalogStore::open(in_memory_options("gc", object_store))
        .await
        .unwrap();
    let mut writer = store.begin_write();
    writer.create_schema("schema").await.unwrap();
    let commit = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(commit);

    pin_snapshot(store.db(), 1).await.unwrap();
    assert_eq!(gc_apply(store.db(), 1).await.unwrap().new_retain_from, 1);
    assert!(matches!(
        gc_apply(store.db(), 2).await,
        Err(CatalogError::PinnedSnapshotBlocks { .. })
    ));
    unpin_snapshot(store.db(), 1).await.unwrap();

    hold_snapshot(store.db(), "reader", 1, 60).await.unwrap();
    assert!(matches!(
        gc_apply(store.db(), 2).await,
        Err(CatalogError::PinnedSnapshotBlocks { .. })
    ));
    assert!(matches!(
        hold_snapshot(store.db(), "old", 0, 60).await,
        Err(CatalogError::SnapshotOutOfRetention { .. })
    ));
    store.close().await.unwrap();
}

#[tokio::test]
async fn excision_removes_only_retired_data_files() {
    let object_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let mut store = CatalogStore::open(in_memory_options("excise", object_store))
        .await
        .unwrap();
    let (table_id, file_id) = {
        let mut writer = store.begin_write();
        let schema_id = writer.create_schema("schema").await.unwrap();
        let table_id = writer.create_table(schema_id, "table", None).await.unwrap();
        let file_id = writer
            .register_data_file_with_metadata(
                table_id,
                "live.parquet",
                "parquet",
                1,
                1,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let commit = writer.create_snapshot(None, None).await.unwrap();
        store.commit_writer(commit);
        (table_id, file_id)
    };
    gc_apply(store.db(), 2).await.unwrap();
    assert!(excise_plan(store.db(), 2)
        .await
        .unwrap()
        .data_files_eligible
        .is_empty());

    let mut writer = store.begin_write();
    writer.mark_data_file_deleted(file_id).await.unwrap();
    let commit = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(commit);
    gc_apply(store.db(), 3).await.unwrap();
    let plan = excise_plan(store.db(), 3).await.unwrap();
    assert_eq!(plan.data_files_eligible, vec!["live.parquet"]);
    let result = excise_apply(store.db(), 3, "test").await.unwrap();
    assert_eq!(result.keys_failed, 0);
    assert!(store
        .read_at(rocklake_core::mvcc::SnapshotId::new(2))
        .unwrap()
        .list_data_files(table_id)
        .await
        .unwrap()
        .is_empty());
    store.close().await.unwrap();
}

#[tokio::test]
async fn scheduled_deletion_waits_for_file_retirement() {
    let object_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let store = CatalogStore::open(in_memory_options("scheduled", Arc::clone(&object_store)))
        .await
        .unwrap();
    let db = store.db();
    let file = DataFileRow {
        data_file_id: 7,
        table_id: 1,
        path: "retired.parquet".to_string(),
        file_format: "parquet".to_string(),
        record_count: 1,
        file_size_bytes: 1,
        footer_size: None,
        encryption_key: None,
        begin_snapshot: Some(1),
        end_snapshot: Some(2),
        file_order: None,
        path_is_relative: Some(true),
        row_id_start: None,
        partition_id: None,
        mapping_id: None,
        partial_max: None,
    };
    db.put(&keys::key_data_file(1, 7), &values::encode_value(&file))
        .await
        .unwrap();
    let schedule = FilesScheduledForDeletionRow {
        data_file_id: 7,
        schedule_start: 1,
        path: "retired.parquet".to_string(),
        file_type: Some("data".to_string()),
        path_is_relative: Some(true),
    };
    db.put(
        &keys::key_files_scheduled_for_deletion(1, 7),
        &values::encode_value(&schedule),
    )
    .await
    .unwrap();
    object_store
        .put(
            &ObjectPath::from("data/retired.parquet"),
            b"data".to_vec().into(),
        )
        .await
        .unwrap();

    let data_prefix = ObjectPath::from("data");
    let report = process_scheduled_deletions_report_at(db, &object_store, &data_prefix, 1)
        .await
        .unwrap();
    assert_eq!(report.deleted, 0);
    assert_eq!(report.skipped, 1);
    assert!(object_store
        .head(&ObjectPath::from("data/retired.parquet"))
        .await
        .is_ok());

    let report = process_scheduled_deletions_report_at(db, &object_store, &data_prefix, 2)
        .await
        .unwrap();
    assert_eq!(report.deleted, 1);
    assert!(object_store
        .head(&ObjectPath::from("data/retired.parquet"))
        .await
        .is_err());
    store.close().await.unwrap();
}

#[tokio::test]
async fn orphan_sweep_uses_the_same_canonical_prefix_as_catalog_paths() {
    let object_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let mut store = CatalogStore::open(in_memory_options("orphans", Arc::clone(&object_store)))
        .await
        .unwrap();
    let mut writer = store.begin_write();
    let schema_id = writer.create_schema("schema").await.unwrap();
    let table_id = writer.create_table(schema_id, "table", None).await.unwrap();
    writer
        .register_data_file_with_metadata(
            table_id,
            "retained.parquet",
            "parquet",
            1,
            1,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let commit = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(commit);

    object_store
        .put(
            &ObjectPath::from("data/retained.parquet"),
            b"retained".to_vec().into(),
        )
        .await
        .unwrap();
    object_store
        .put(
            &ObjectPath::from("data/orphan.parquet"),
            b"orphan".to_vec().into(),
        )
        .await
        .unwrap();

    let result = orphaned_file_sweep(
        store.db(),
        &object_store,
        &ObjectPath::from("data"),
        0,
        true,
    )
    .await
    .unwrap();
    assert_eq!(result.orphaned_files, vec!["data/orphan.parquet"]);
    assert_eq!(result.deleted_files, vec!["data/orphan.parquet"]);
    assert!(result.deletion_failures.is_empty());
    assert!(object_store
        .head(&ObjectPath::from("data/retained.parquet"))
        .await
        .is_ok());
    assert!(object_store
        .head(&ObjectPath::from("data/orphan.parquet"))
        .await
        .is_err());
    store.close().await.unwrap();
}
