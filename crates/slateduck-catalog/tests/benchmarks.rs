//! Benchmark baseline for Phase 2.
//!
//! Records p50/p95/p99/p99.9 for key catalog operations on InMemory store.

use object_store::memory::InMemory;
use slateduck_catalog::{CatalogStore, OpenOptions};
use slateduck_core::rows::FileColumnStatsRow;
use slateduck_core::stats::{prune_files, DuckLakeType, PrunePredicate};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    let idx = ((sorted.len() as f64) * p / 100.0) as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn opts(path: &str) -> OpenOptions {
    OpenOptions {
        path: path.to_string(),
        object_store: Arc::new(InMemory::new()),
        retention_days: 7,
    }
}

#[tokio::test]
async fn benchmark_get_current_snapshot() {
    let catalog = CatalogStore::open(opts("bench/snap")).await.unwrap();
    let mut writer = catalog.begin_write().await.unwrap();
    writer.create_snapshot("{}", None, None).await.unwrap();

    let mut latencies = Vec::new();
    for _ in 0..100 {
        let start = Instant::now();
        let _ = catalog.current_snapshot_id().await.unwrap();
        latencies.push(start.elapsed());
    }

    latencies.sort();
    let p50 = percentile(&latencies, 50.0);
    let p95 = percentile(&latencies, 95.0);
    let p99 = percentile(&latencies, 99.0);

    // Sanity check: InMemory should be fast
    assert!(p50 < Duration::from_millis(50), "p50={p50:?}");
    assert!(p95 < Duration::from_millis(200), "p95={p95:?}");
    assert!(p99 < Duration::from_millis(500), "p99={p99:?}");

    eprintln!("get_current_snapshot: p50={p50:?} p95={p95:?} p99={p99:?}");
    catalog.close().await.unwrap();
}

#[tokio::test]
async fn benchmark_list_data_files_100() {
    let catalog = CatalogStore::open(opts("bench/files100")).await.unwrap();
    let mut writer = catalog.begin_write().await.unwrap();

    let schema_id = writer.create_schema("main", 1).await.unwrap();
    let table_id = writer.create_table(schema_id, "t", "u", 1).await.unwrap();

    // Register 100 files (debug-mode-friendly; roadmap target is 10K)
    for i in 0..100u64 {
        writer
            .register_data_file(
                table_id,
                &format!("/data/file_{i:06}.parquet"),
                false,
                4096,
                100,
                1,
            )
            .await
            .unwrap();
    }

    let snap = writer.create_snapshot("{}", None, None).await.unwrap();

    let mut latencies = Vec::new();
    for _ in 0..10 {
        let reader = catalog.read_at(snap).await;
        let start = Instant::now();
        let files = reader.list_data_files(table_id).await.unwrap();
        latencies.push(start.elapsed());
        assert_eq!(files.len(), 100);
    }

    latencies.sort();
    let p50 = percentile(&latencies, 50.0);
    let p95 = percentile(&latencies, 95.0);

    // 100 file scan should complete quickly
    assert!(p50 < Duration::from_secs(5), "p50={p50:?}");
    assert!(p95 < Duration::from_secs(10), "p95={p95:?}");

    eprintln!("list_data_files(100): p50={p50:?} p95={p95:?}");
    catalog.close().await.unwrap();
}

#[tokio::test]
async fn benchmark_describe_table_100_columns() {
    let catalog = CatalogStore::open(opts("bench/cols100")).await.unwrap();
    let mut writer = catalog.begin_write().await.unwrap();

    let schema_id = writer.create_schema("main", 1).await.unwrap();
    let table_id = writer.create_table(schema_id, "t", "u", 1).await.unwrap();

    for i in 0..100 {
        writer
            .add_column(table_id, &format!("col_{i}"), "VARCHAR", true, None, 1)
            .await
            .unwrap();
    }

    let snap = writer.create_snapshot("{}", None, None).await.unwrap();

    let mut latencies = Vec::new();
    for _ in 0..100 {
        let reader = catalog.read_at(snap).await;
        let start = Instant::now();
        let cols = reader.describe_table(table_id).await.unwrap();
        latencies.push(start.elapsed());
        assert_eq!(cols.len(), 100);
    }

    latencies.sort();
    let p50 = percentile(&latencies, 50.0);
    let p95 = percentile(&latencies, 95.0);
    let p99 = percentile(&latencies, 99.0);

    assert!(p50 < Duration::from_millis(500), "p50={p50:?}");
    assert!(p95 < Duration::from_secs(2), "p95={p95:?}");
    assert!(p99 < Duration::from_secs(5), "p99={p99:?}");

    eprintln!("describe_table(100 cols): p50={p50:?} p95={p95:?} p99={p99:?}");
    catalog.close().await.unwrap();
}

#[tokio::test]
async fn benchmark_prune_files() {
    // Build stats for 1000 files
    let stats: Vec<FileColumnStatsRow> = (0..1000)
        .map(|i| FileColumnStatsRow {
            table_id: 1,
            column_id: 1,
            data_file_id: i,
            min_value: Some(format!("{}", i * 100)),
            max_value: Some(format!("{}", (i + 1) * 100 - 1)),
            null_count: Some(0),
            contains_nan: false,
        })
        .collect();

    let mut latencies = Vec::new();
    for _ in 0..100 {
        let start = Instant::now();
        let result = prune_files(
            &stats,
            &PrunePredicate::Equal("50000".to_string()),
            &DuckLakeType::Integer,
        )
        .unwrap();
        latencies.push(start.elapsed());
        assert_eq!(result.len(), 1); // Only one file contains 50000
    }

    latencies.sort();
    let p50 = percentile(&latencies, 50.0);
    let p95 = percentile(&latencies, 95.0);

    assert!(p50 < Duration::from_millis(10), "p50={p50:?}");
    assert!(p95 < Duration::from_millis(50), "p95={p95:?}");

    eprintln!("prune_files(1000 stats): p50={p50:?} p95={p95:?}");
}

#[tokio::test]
async fn benchmark_create_snapshot_100_files() {
    let catalog = CatalogStore::open(opts("bench/snap100")).await.unwrap();
    let mut writer = catalog.begin_write().await.unwrap();

    let schema_id = writer.create_schema("main", 1).await.unwrap();
    let table_id = writer.create_table(schema_id, "t", "u", 1).await.unwrap();

    let mut latencies = Vec::new();
    for batch in 0..10 {
        // Register 10 files per batch
        for i in 0..10 {
            let idx = batch * 10 + i;
            writer
                .register_data_file(
                    table_id,
                    &format!("/data/file_{idx:06}.parquet"),
                    false,
                    4096,
                    100,
                    (batch + 1) as u64,
                )
                .await
                .unwrap();
        }
        let start = Instant::now();
        writer.create_snapshot("{}", None, None).await.unwrap();
        latencies.push(start.elapsed());
    }

    latencies.sort();
    let p50 = percentile(&latencies, 50.0);
    let p95 = percentile(&latencies, 95.0);

    assert!(p50 < Duration::from_secs(2), "p50={p50:?}");
    assert!(p95 < Duration::from_secs(5), "p95={p95:?}");

    eprintln!("create_snapshot(10 files): p50={p50:?} p95={p95:?}");
    catalog.close().await.unwrap();
}
