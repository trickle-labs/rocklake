use std::sync::Arc;

use object_store::path::Path as ObjectPath;
use rocklake_catalog::writer::stats::FileColumnStatsInput;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_core::keys::MetadataScope;
use tempfile::TempDir;

fn test_opts(dir: &TempDir) -> OpenOptions {
    let store =
        Arc::new(object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap());
    OpenOptions {
        object_store: store,
        path: ObjectPath::from("catalog"),
        encryption: None,
    }
}

#[tokio::test]
async fn v10_upgrade_repairs_defaults_and_only_unsafe_bounds() {
    let dir = TempDir::new().unwrap();
    let mut catalog = CatalogStore::open(test_opts(&dir)).await.unwrap();
    let (table_id, parent_id, child_id, stats_column_id) = {
        let mut writer = catalog.begin_write();
        let schema_id = writer.create_schema("main").await.unwrap();
        let table_id = writer
            .create_table(schema_id, "events", None)
            .await
            .unwrap();
        let parent_id = writer
            .add_column(table_id, "payload", "struct", 0, true, None)
            .await
            .unwrap();
        let child_id = writer
            .add_column_with_opts(
                table_id,
                "amount",
                "int64",
                0,
                true,
                Some("42"),
                Some("42"),
                Some("literal"),
                Some("duckdb"),
                Some(parent_id),
            )
            .await
            .unwrap();
        let stats_column_id = writer
            .add_column(table_id, "value", "float64", 1, true, None)
            .await
            .unwrap();
        writer
            .set_metadata(MetadataScope::Global, 0, "version", "1.0")
            .unwrap();
        let committed = writer.create_snapshot(None, None).await.unwrap();
        catalog.commit_writer(committed);
        (table_id, parent_id, child_id, stats_column_id)
    };

    let file_id = {
        let mut writer = catalog.begin_write();
        writer
            .add_column_mapping(
                table_id,
                Some(10),
                Some(stats_column_id),
                Some("value"),
                Some("field_id"),
            )
            .await
            .unwrap();
        let file_id = writer
            .register_data_file_with_metadata(
                table_id,
                "events.parquet",
                "parquet",
                1,
                128,
                None,
                None,
                None,
                Some(10),
                None,
            )
            .await
            .unwrap();
        writer
            .upsert_file_column_stats_with_exactness(
                FileColumnStatsInput {
                    table_id,
                    column_id: stats_column_id,
                    data_file_id: file_id,
                    contains_null: false,
                    min_value: Some("0.1"),
                    max_value: Some("1.5"),
                    contains_nan: false,
                    column_size_bytes: None,
                    value_count: None,
                    null_count: None,
                    extra_stats: None,
                },
                None,
                None,
            )
            .await
            .unwrap();
        writer
            .upsert_table_column_stats(
                table_id,
                stats_column_id,
                false,
                Some("0.1"),
                Some("1.5"),
                Some(false),
                None,
            )
            .await
            .unwrap();
        let committed = writer.create_snapshot(None, None).await.unwrap();
        catalog.commit_writer(committed);
        file_id
    };

    {
        let mut writer = catalog.begin_write();
        writer
            .add_column_with_id(
                child_id,
                table_id,
                "amount",
                "int64",
                0,
                true,
                None,
                None,
                None,
                None,
                Some(parent_id),
            )
            .await
            .unwrap();
        writer
            .add_column_with_id(
                stats_column_id,
                table_id,
                "value",
                "varchar",
                1,
                true,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let committed = writer.create_snapshot(None, None).await.unwrap();
        catalog.commit_writer(committed);
    }

    {
        let mut writer = catalog.begin_write();
        assert!(writer.migrate_ducklake_v10_to_v11().await.unwrap());
        let committed = writer.create_snapshot(None, None).await.unwrap();
        catalog.commit_writer(committed);
    }

    let reader = catalog.read_latest();
    let version = reader
        .get_metadata(MetadataScope::Global, 0, "version")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(version.value, "1.1");

    let (_, columns) = reader.describe_table(table_id).await.unwrap().unwrap();
    let child = columns
        .iter()
        .find(|column| column.column_id == child_id)
        .unwrap();
    assert_eq!(child.initial_default.as_deref(), Some("42"));
    assert_eq!(child.default_value.as_deref(), Some("42"));
    assert_eq!(child.default_value_type.as_deref(), Some("literal"));
    assert_eq!(child.default_value_dialect.as_deref(), Some("duckdb"));

    let file_stats = reader
        .list_file_column_stats(table_id, stats_column_id)
        .await
        .unwrap();
    let file_stats = file_stats
        .iter()
        .find(|row| row.data_file_id == file_id)
        .unwrap();
    assert_eq!(file_stats.min_value, None);
    assert_eq!(file_stats.max_value.as_deref(), Some("1.5"));
    let table_stats = reader
        .list_all_table_column_stats()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.table_id == table_id && row.column_id == stats_column_id)
        .unwrap();
    assert_eq!(table_stats.min_value, None);
    assert_eq!(table_stats.max_value.as_deref(), Some("1.5"));

    let mut writer = catalog.begin_write();
    assert!(!writer.migrate_ducklake_v10_to_v11().await.unwrap());
}

#[tokio::test]
async fn v10_upgrade_backfills_missing_inlined_schema_version() {
    let dir = TempDir::new().unwrap();
    let mut catalog = CatalogStore::open(test_opts(&dir)).await.unwrap();
    let table_id = {
        let mut writer = catalog.begin_write();
        let schema_id = writer.create_schema("main").await.unwrap();
        let table_id = writer
            .create_table(schema_id, "events", None)
            .await
            .unwrap();
        writer
            .add_column(table_id, "a", "int64", 0, true, None)
            .await
            .unwrap();
        writer
            .register_schema_version(0, 0, table_id)
            .await
            .unwrap();
        writer
            .register_schema_version(1, 1, table_id)
            .await
            .unwrap();
        writer
            .set_metadata(MetadataScope::Global, 0, "version", "1.0")
            .unwrap();
        let committed = writer.create_snapshot(None, None).await.unwrap();
        catalog.commit_writer(committed);
        table_id
    };
    {
        let mut writer = catalog.begin_write();
        writer
            .add_column(table_id, "b", "int64", 1, true, None)
            .await
            .unwrap();
        writer
            .register_inlined_data_table(table_id, "ducklake_inlined_data_1_2", 2)
            .await
            .unwrap();
        let committed = writer.create_snapshot(None, None).await.unwrap();
        catalog.commit_writer(committed);
    }
    {
        let mut writer = catalog.begin_write();
        writer.migrate_ducklake_v10_to_v11().await.unwrap();
        let committed = writer.create_snapshot(None, None).await.unwrap();
        catalog.commit_writer(committed);
    }

    let versions = catalog
        .read_latest()
        .list_all_schema_versions()
        .await
        .unwrap();
    assert!(versions.iter().any(|row| {
        row.table_id == table_id && row.schema_version == 2 && row.begin_snapshot == 2
    }));
}

#[tokio::test]
async fn v10_migration_recovers_after_failed_snapshot_commit_and_reopen() {
    let dir = TempDir::new().unwrap();
    let table_id = {
        let mut catalog = CatalogStore::open(test_opts(&dir)).await.unwrap();
        let mut writer = catalog.begin_write();
        let schema_id = writer.create_schema("main").await.unwrap();
        let table_id = writer
            .create_table(schema_id, "events", None)
            .await
            .unwrap();
        writer
            .add_column(table_id, "a", "int64", 0, true, None)
            .await
            .unwrap();
        writer
            .register_schema_version(0, 0, table_id)
            .await
            .unwrap();
        writer
            .register_schema_version(1, 1, table_id)
            .await
            .unwrap();
        writer
            .set_metadata(MetadataScope::Global, 0, "version", "1.0")
            .unwrap();
        let committed = writer.create_snapshot(None, None).await.unwrap();
        catalog.commit_writer(committed);

        let mut writer = catalog.begin_write();
        writer
            .add_column(table_id, "b", "int64", 1, true, None)
            .await
            .unwrap();
        writer
            .register_inlined_data_table(table_id, "ducklake_inlined_data_1_2", 2)
            .await
            .unwrap();
        writer
            .register_inlined_insert(table_id, 2, 0, br#"["kept"]"#.to_vec())
            .await
            .unwrap();
        let committed = writer.create_snapshot(None, None).await.unwrap();
        catalog.commit_writer(committed);
        catalog.close().await.unwrap();
        table_id
    };

    let mut catalog = CatalogStore::open(test_opts(&dir)).await.unwrap();
    let mut writer = catalog.begin_write();
    assert!(writer.migrate_ducklake_v10_to_v11().await.unwrap());
    let injector = rocklake_catalog::fault_injection::FaultInjector::new();
    injector.clear_all();
    injector.set_error(
        rocklake_catalog::fault_injection::WriteFaultPoint::BeforeSnapshotCommit,
        "simulate interruption before migration commit",
    );
    assert!(writer.create_snapshot(None, None).await.is_err());
    injector.clear_all();
    drop(writer);
    catalog.close().await.unwrap();

    let mut catalog = CatalogStore::open(test_opts(&dir)).await.unwrap();
    {
        let reader = catalog.read_latest();
        assert_eq!(reader.get_snapshot().await.unwrap().unwrap().snapshot_id, 2);
        assert_eq!(
            reader
                .get_metadata(MetadataScope::Global, 0, "version")
                .await
                .unwrap()
                .unwrap()
                .value,
            "1.0"
        );
        assert!(reader
            .list_all_schema_versions()
            .await
            .unwrap()
            .iter()
            .all(|row| row.table_id != table_id || row.schema_version != 2));
        assert_eq!(
            reader.list_inlined_inserts(table_id).await.unwrap().len(),
            1
        );
    }

    let mut writer = catalog.begin_write();
    assert!(writer.migrate_ducklake_v10_to_v11().await.unwrap());
    let committed = writer.create_snapshot(None, None).await.unwrap();
    catalog.commit_writer(committed);
    catalog.close().await.unwrap();

    let catalog = CatalogStore::open(test_opts(&dir)).await.unwrap();
    let reader = catalog.read_latest();
    assert_eq!(
        reader
            .get_metadata(MetadataScope::Global, 0, "version")
            .await
            .unwrap()
            .unwrap()
            .value,
        "1.1"
    );
    assert!(reader
        .list_all_schema_versions()
        .await
        .unwrap()
        .iter()
        .any(|row| row.table_id == table_id && row.schema_version == 2));
    assert_eq!(
        reader.list_inlined_inserts(table_id).await.unwrap().len(),
        1
    );
}
