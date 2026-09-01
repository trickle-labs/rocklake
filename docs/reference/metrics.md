# Metrics Reference

RockLake exposes Prometheus text metrics from a separate HTTP listener when
`--metrics-port` is set. The path defaults to `/metrics` and can be changed
with `--metrics-path` or `ROCKLAKE_METRICS_PATH`.

```bash
rocklake serve \
  --catalog s3://bucket/catalog/ \
  --metrics-port 9090 \
  --metrics-path /metrics
```

The endpoint is disabled unless a metrics port is configured. It serves only
the configured path; other paths return 404.

## Exported metrics

The v0.51.0 binary exports these Prometheus metrics:

- Catalog: `rocklake_snapshots_created_total`, `rocklake_files_per_snapshot`.
- Object store: `rocklake_object_store_requests_total`,
  `rocklake_object_store_bytes_read_total`,
  `rocklake_object_store_bytes_written_total`,
  `rocklake_object_store_throttles_total`,
  `rocklake_object_store_retries_total`.
- Sessions: `rocklake_active_sessions`, `rocklake_idle_sessions`,
  `rocklake_max_sessions`.
- Queries: `rocklake_last_query_keys_scanned`,
  `rocklake_pgwire_queries_total`,
  `rocklake_pgwire_query_duration_seconds` (histogram),
  `rocklake_pgwire_response_rows_total`,
  `rocklake_pgwire_response_bytes_total`,
  `rocklake_pgwire_response_rows_per_second`,
  `rocklake_pgwire_response_bytes_per_second`,
  `rocklake_pgwire_time_to_first_row_seconds`,
  `rocklake_pgwire_sql_classification_seconds`,
  `rocklake_pgwire_errors_total`.
- Lifecycle: `rocklake_gc_retain_from_snapshot`,
  `rocklake_excision_bytes_deleted_total`,
  `rocklake_cdc_record_count_mismatch_total`.
- SlateDB estimates: `rocklake_slatedb_sst_count`,
  `rocklake_slatedb_compaction_lag_ms`, `rocklake_slatedb_memtable_bytes`.
- DataFusion: `rocklake_datafusion_bridge_queue_depth`.
- Resource limits: `rocklake_active_scans`, `rocklake_max_active_scans`,
  `rocklake_stream_queue_depth`, `rocklake_max_buffered_rows`,
  `rocklake_pgwire_peak_buffered_rows`, `rocklake_max_response_bytes`,
  `rocklake_process_rss_bytes`, `rocklake_process_peak_rss_bytes`,
  `rocklake_resource_limit_exhaustions_total`,
  `rocklake_stream_backpressure_total`.

Operation latency is exported as `_sum` and `_count` series under
`rocklake_catalog_op_duration_seconds` with an `op` label.

## Prometheus scrape configuration

```yaml
scrape_configs:
  - job_name: rocklake
    static_configs:
      - targets: ["127.0.0.1:9090"]
```
