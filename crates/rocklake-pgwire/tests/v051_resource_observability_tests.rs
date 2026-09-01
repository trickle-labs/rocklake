use rocklake_catalog::metrics::CatalogMetrics;
use rocklake_pgwire::ServerConfig;

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
