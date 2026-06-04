#![cfg(feature = "minio-tests")]

use std::time::Instant;

use rocklake_catalog::writer::stats::FileColumnStatsInput;
use rocklake_core::types::DuckLakeType;
use rocklake_testkit::{CatalogHarness, MinioHarness};

static MINIO: tokio::sync::OnceCell<MinioHarness> = tokio::sync::OnceCell::const_new();

async fn minio() -> &'static MinioHarness {
    MINIO
        .get_or_init(|| async {
            MinioHarness::start("rocklake-catalog-tests")
                .await
                .expect("MinIO should start for catalog integration tests")
        })
        .await
}

async fn catalog(prefix: &str) -> CatalogHarness {
    CatalogHarness::on_minio(minio().await, prefix)
        .await
        .expect("catalog should open on MinIO")
}

#[tokio::test]
async fn catalog_open_and_initialize_on_minio() {
    let catalog = catalog("minio/open_and_initialize").await;
    let reader = catalog.reader_latest().await;
    let _schemas = reader.list_schemas().await.expect("list_schemas should succeed");
}

#[tokio::test]
async fn catalog_reopen_preserves_state_on_minio() {
    let catalog = catalog("minio/reopen_preserves_state").await;

    let mut writer = catalog.writer().await;
    let schema_id = writer.create_schema("analytics").await.expect("create_schema should succeed");
    let snapshot = writer
        .create_snapshot(Some("minio-tests"), Some("reopen"))
        .await
        .expect("create_snapshot should succeed");
    catalog.commit_writer(snapshot).await;
    let _ = schema_id;

    catalog.reopen().await.expect("reopen should succeed");
    let reader = catalog.reader_latest().await;
    let schemas = reader.list_schemas().await.expect("list_schemas should succeed");
    assert!(schemas.iter().any(|schema| schema.schema_name == "analytics"));
}

#[tokio::test]
async fn sequential_snapshot_ids_monotone_on_minio() {
    let catalog = catalog("minio/sequential_snapshot_ids").await;
    let mut previous = 0u64;

    for index in 0..20u32 {
        let mut writer = catalog.writer().await;
        writer
            .create_schema(&format!("schema_{index}"))
            .await
            .expect("create_schema should succeed");
        let snapshot = writer
            .create_snapshot(Some("minio-tests"), Some("monotone"))
            .await
            .expect("create_snapshot should succeed");
        catalog.commit_writer(snapshot).await;
        let current = snapshot.as_u64();
        assert!(current > previous, "snapshot ids must increase monotonically");
        previous = current;
    }
}

#[tokio::test]
async fn flush_visibility_barrier_on_minio() {
    let catalog = catalog("minio/flush_visibility_barrier").await;
    let mut latencies_ms = Vec::with_capacity(20);

    for index in 0..20u32 {
        let mut writer = catalog.writer().await;
        writer
            .create_schema(&format!("flush_{index}"))
            .await
            .expect("create_schema should succeed");
        let start = Instant::now();
        let snapshot = writer
            .create_snapshot(Some("minio-tests"), Some("flush"))
            .await
            .expect("create_snapshot should succeed");
        catalog.commit_writer(snapshot).await;
        let reader = catalog.reader_latest().await;
        let schemas = reader.list_schemas().await.expect("list_schemas should succeed");
        assert!(schemas.iter().any(|schema| schema.schema_name == format!("flush_{index}")));
        latencies_ms.push(start.elapsed().as_millis() as u64);
    }

    latencies_ms.sort_unstable();
    let p99_index = latencies_ms.len().saturating_sub(1);
    let p99 = latencies_ms[p99_index];
    assert!(p99 < 1500, "flush visibility p99 latency {p99}ms exceeds 1500ms");
}

#[tokio::test]
async fn reader_snapshot_isolation_on_minio() {
    let catalog = catalog("minio/reader_snapshot_isolation").await;

    let mut writer = catalog.writer().await;
    let schema_id = writer.create_schema("isolation").await.expect("create_schema should succeed");
    let snapshot_a = writer
        .create_snapshot(Some("minio-tests"), Some("snapshot-a"))
        .await
        .expect("create_snapshot should succeed");
    catalog.commit_writer(snapshot_a).await;

    let reader_at_a = catalog.reader_at(*snapshot_a).await.expect("reader_at should succeed");

    let mut writer = catalog.writer().await;
    writer
        .create_table(schema_id, "events", None)
        .await
        .expect("create_table should succeed");
    let snapshot_b = writer
        .create_snapshot(Some("minio-tests"), Some("snapshot-b"))
        .await
        .expect("create_snapshot should succeed");
    catalog.commit_writer(snapshot_b).await;

    let tables_at_a = reader_at_a.list_tables(schema_id).await.expect("list_tables should succeed");
    assert!(tables_at_a.is_empty(), "reader at the earlier snapshot should not see the later table");

    let latest = catalog.reader_latest().await;
    let tables = latest.list_tables(schema_id).await.expect("list_tables should succeed");
    assert!(tables.iter().any(|table| table.table_name == "events"));
}

#[tokio::test]
async fn large_file_registration_10k_files_on_minio() {
    let catalog = catalog("minio/large_file_registration").await;

    let mut writer = catalog.writer().await;
    let schema_id = writer.create_schema("bulk").await.expect("create_schema should succeed");
    let table_id = writer
        .create_table(schema_id, "events", None)
        .await
        .expect("create_table should succeed");
    let snapshot = writer
        .create_snapshot(Some("minio-tests"), Some("table-setup"))
        .await
        .expect("create_snapshot should succeed");
    catalog.commit_writer(snapshot).await;

    let batch_size = 250usize;
    for batch_start in (0..10_000usize).step_by(batch_size) {
        let mut writer = catalog.writer().await;
        for file_index in batch_start..(batch_start + batch_size).min(10_000) {
            writer
                .register_data_file(
                    table_id,
                    &format!("s3://rocklake-test/events/file-{file_index}.parquet"),
                    "parquet",
                    1_000,
                    4_096,
                )
                .await
                .expect("register_data_file should succeed");
        }
        let snapshot = writer
            .create_snapshot(Some("minio-tests"), Some("batch"))
            .await
            .expect("create_snapshot should succeed");
        catalog.commit_writer(snapshot).await;
    }

    let reader = catalog.reader_latest().await;
    let files = reader.list_data_files(table_id).await.expect("list_data_files should succeed");
    assert!(files.len() >= 10_000, "expected at least 10k registered files");
}

#[tokio::test]
async fn prune_files_zone_map_on_minio() {
    let catalog = catalog("minio/prune_files").await;

    let mut writer = catalog.writer().await;
    let schema_id = writer.create_schema("pruning").await.expect("create_schema should succeed");
    let table_id = writer
        .create_table(schema_id, "metrics", None)
        .await
        .expect("create_table should succeed");
    let file_low = writer
        .register_data_file(table_id, "s3://rocklake-test/low.parquet", "parquet", 100, 4096)
        .await
        .expect("register_data_file should succeed");
    let file_high = writer
        .register_data_file(table_id, "s3://rocklake-test/high.parquet", "parquet", 100, 4096)
        .await
        .expect("register_data_file should succeed");
    writer
        .upsert_file_column_stats(FileColumnStatsInput {
            table_id,
            column_id: 1,
            data_file_id: file_low,
            contains_null: false,
            min_value: Some("1"),
            max_value: Some("10"),
            contains_nan: false,
            column_size_bytes: None,
            value_count: None,
            null_count: None,
            extra_stats: None,
        })
        .await
        .expect("upsert_file_column_stats should succeed");
    writer
        .upsert_file_column_stats(FileColumnStatsInput {
            table_id,
            column_id: 1,
            data_file_id: file_high,
            contains_null: false,
            min_value: Some("100"),
            max_value: Some("200"),
            contains_nan: false,
            column_size_bytes: None,
            value_count: None,
            null_count: None,
            extra_stats: None,
        })
        .await
        .expect("upsert_file_column_stats should succeed");
    let snapshot = writer
        .create_snapshot(Some("minio-tests"), Some("pruning"))
        .await
        .expect("create_snapshot should succeed");
    catalog.commit_writer(snapshot).await;

    let reader = catalog.reader_latest().await;
    let kept = reader
        .prune_files(
            table_id,
            1,
            "5",
            &DuckLakeType::Integer {
                signed: true,
                width_bits: 32,
            },
        )
        .await
        .expect("prune_files should succeed");
    assert!(kept.contains(&file_low));
    assert!(!kept.contains(&file_high));
}
