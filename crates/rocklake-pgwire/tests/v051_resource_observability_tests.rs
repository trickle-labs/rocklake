use std::sync::atomic::Ordering;
use std::sync::Arc;

use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use rocklake_catalog::metrics::CatalogMetrics;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_pgwire::{run_server_with_shutdown, ServerConfig};
use tempfile::TempDir;
use tokio::sync::{oneshot, Mutex};

#[test]
fn default_limits_are_bounded() {
    let config = ServerConfig::default();
    assert!(config.max_active_scans > 0);
    assert!(config.stream_queue_depth > 0);
    assert!(config.max_buffered_rows > 0);
    assert!(config.max_response_bytes > 0);
    assert!(!config.slow_operation_threshold.is_zero());
}

#[test]
fn metrics_render_histogram_and_resource_signals() {
    let metrics = CatalogMetrics::new(10);
    metrics.set_resource_limits(2, 8, 64, 4096);
    metrics.set_active_scans(1);
    metrics.record_pgwire_query(10_000);
    metrics.record_pgwire_response_with_timing(3, 300, 2_000, 10_000);
    metrics.increment_resource_limit_exhaustions();
    metrics.increment_stream_backpressure();

    let output = metrics.render_prometheus();
    assert!(output.contains("# TYPE rocklake_pgwire_query_duration_seconds histogram"));
    assert!(output.contains("rocklake_pgwire_query_duration_seconds_bucket{le=\"0.010\"} 1"));
    assert!(output.contains("rocklake_active_scans 1"));
    assert!(output.contains("rocklake_resource_limit_exhaustions_total 1"));
    assert!(output.contains("rocklake_stream_backpressure_total 1"));
    assert!(output.contains("rocklake_pgwire_response_rows_per_second 300"));
    assert!(output.contains("rocklake_pgwire_response_bytes_per_second 30000"));
    assert!(output.contains("rocklake_process_rss_bytes"));
    assert!(output.contains("rocklake_process_peak_rss_bytes"));
}

#[test]
fn query_histogram_is_cumulative_once() {
    let metrics = CatalogMetrics::new(10);
    metrics.record_pgwire_query(500);
    metrics.record_pgwire_query(5_000);
    metrics.record_pgwire_query(50_000);

    let output = metrics.render_prometheus();
    let buckets: Vec<u64> = output
        .lines()
        .filter(|line| line.starts_with("rocklake_pgwire_query_duration_seconds_bucket"))
        .map(|line| line.split_whitespace().last().unwrap().parse().unwrap())
        .collect();
    assert!(buckets.windows(2).all(|pair| pair[0] <= pair[1]));
    assert_eq!(buckets.last(), Some(&3));
    assert!(output.contains("rocklake_pgwire_query_duration_seconds_count 3"));
}

#[test]
fn query_phase_metrics_have_matching_sums_and_counts() {
    let metrics = CatalogMetrics::new(10);
    metrics.observe_pgwire_admission_us(100_000);
    metrics.observe_sql_classification_us(200_000);
    metrics.observe_pgwire_execution_us(300_000);
    metrics.observe_pgwire_response_delivery_us(400_000);

    let output = metrics.render_prometheus();
    assert!(output.contains("rocklake_pgwire_admission_seconds_sum 0.100000"));
    assert!(output.contains("rocklake_pgwire_admission_seconds_count 1"));
    assert!(output.contains("rocklake_pgwire_execution_seconds_sum 0.300000"));
    assert!(output.contains("rocklake_pgwire_execution_seconds_count 1"));
    assert!(output.contains("rocklake_pgwire_response_delivery_seconds_sum 0.400000"));
    assert!(output.contains("rocklake_pgwire_response_delivery_seconds_count 1"));
    assert!(output.contains("rocklake_pgwire_sql_classification_seconds_sum 0.200000"));
    assert!(output.contains("rocklake_pgwire_sql_classification_seconds_count 1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_simple_and_extended_queries_record_full_request_metrics() {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(LocalFileSystem::new_with_prefix(dir.path()).unwrap());
    let catalog = CatalogStore::open(OpenOptions {
        object_store: store,
        path: ObjectPath::from(""),
        encryption: None,
    })
    .await
    .unwrap();
    let catalog = Arc::new(Mutex::new(catalog));
    let metrics = Arc::new(CatalogMetrics::new(50));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(run_server_with_shutdown(
        ServerConfig {
            bind_addr: addr,
            metrics: Some(metrics.clone()),
            ..ServerConfig::default()
        },
        catalog,
        shutdown_rx,
    ));

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let (client, connection) = tokio_postgres::connect(
        &format!(
            "host={} port={} user=duckdb dbname=rocklake",
            addr.ip(),
            addr.port()
        ),
        tokio_postgres::NoTls,
    )
    .await
    .unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });

    client.simple_query("SELECT 1").await.unwrap();
    client.query_one("SELECT 1", &[]).await.unwrap();
    drop(client);
    let _ = shutdown_tx.send(());
    server.await.unwrap().unwrap();

    assert_eq!(metrics.pgwire_queries_total.load(Ordering::Relaxed), 2);
    assert_eq!(
        metrics
            .pgwire_sql_classification_count
            .load(Ordering::Relaxed),
        2
    );
    assert_eq!(metrics.pgwire_admission_count.load(Ordering::Relaxed), 2);
    assert_eq!(metrics.pgwire_execution_count.load(Ordering::Relaxed), 2);
    assert_eq!(
        metrics
            .pgwire_response_delivery_count
            .load(Ordering::Relaxed),
        2
    );
    assert_eq!(metrics.pgwire_ttfr_count.load(Ordering::Relaxed), 2);
    assert!(
        metrics
            .pgwire_query_duration_us_total
            .load(Ordering::Relaxed)
            >= metrics.pgwire_execution_us_total.load(Ordering::Relaxed)
    );
}
