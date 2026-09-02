# Monitoring

A well-monitored RockLake deployment tells you three things at a glance: Is it healthy? Is it performing well? Is anything trending toward a problem? RockLake exposes Prometheus-compatible metrics that give you visibility into catalog operations, resource usage, storage interactions, and session state. Combined with proper alerting, these metrics let you catch issues before they affect users.

This page covers the metrics endpoint configuration, the complete metrics catalog with explanations, alerting rules for common failure modes, Grafana dashboard setup, and integration with cloud-native monitoring services.

## Enabling Metrics

RockLake exposes metrics in Prometheus exposition format on a configurable HTTP endpoint:

```bash
rocklake serve \
    --catalog s3://bucket/catalog/ \
    --bind 0.0.0.0:5432 \
    --metrics-port 9090 \
    --metrics-path /metrics
```

Or via environment variables:

```bash
export ROCKLAKE_METRICS_PATH=/metrics
```

The metrics endpoint is a plain HTTP server (separate from the PG-wire listener) that responds to GET requests with the current metric values in Prometheus text format.

### Prometheus Scrape Configuration

```yaml
scrape_configs:
  - job_name: 'rocklake'
    scrape_interval: 15s
    static_configs:
      - targets: ['rocklake:9090']
    metrics_path: /metrics
```

For Kubernetes with Prometheus Operator:

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: rocklake
  namespace: rocklake
spec:
  selector:
    matchLabels:
      app.kubernetes.io/name: rocklake
  endpoints:
    - port: metrics
      interval: 15s
      path: /metrics
```

## Complete Metrics Catalog

The following metrics are emitted by `CatalogMetrics::render_prometheus()` in
`crates/rocklake-catalog/src/metrics.rs`. All are exposed in Prometheus
text format on the configured `--metrics-path` endpoint.

### Snapshot / Catalog Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `rocklake_snapshots_created_total` | Counter | Total catalog snapshots (transactions) committed |
| `rocklake_files_per_snapshot` | Gauge | Data files registered in the most recent snapshot |
| `rocklake_last_query_keys_scanned` | Gauge | SlateDB keys scanned in the last catalog query |

### Object Storage Metrics

These track interactions with the underlying object store (S3/GCS/Azure/local):

| Metric | Type | Description |
|--------|------|-------------|
| `rocklake_object_store_requests_total` | Counter | Total object-store requests issued |
| `rocklake_object_store_bytes_read_total` | Counter | Total bytes read from the object store |
| `rocklake_object_store_bytes_written_total` | Counter | Total bytes written to the object store |
| `rocklake_object_store_throttles_total` | Counter | 429/503 throttle responses from the object store |
| `rocklake_object_store_retries_total` | Counter | Retried object-store requests (transient failures) |

### Session Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `rocklake_connections_open` | Gauge | Currently connected PG-wire clients |
| `rocklake_connections_idle` | Gauge | Connected clients not currently querying |
| `rocklake_queries_in_flight` | Gauge | Queries currently executing |
| `rocklake_max_sessions` | Gauge | Maximum sessions configured via `--max-sessions` |

The v0.51.3 lifecycle names replace the deprecated aliases
`rocklake_active_sessions` (`rocklake_connections_open`) and
`rocklake_idle_sessions` (`rocklake_connections_idle`). The aliases remain
available through v0.53.x. Do not remove them before v0.54.0.

### Query and Resource Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `rocklake_pgwire_query_duration_seconds` | Histogram | PG-wire query duration with bounded latency buckets |
| `rocklake_pgwire_admission_seconds` | Summary | Time spent deciding scan admission |
| `rocklake_pgwire_execution_seconds` | Summary | Time spent in catalog execution |
| `rocklake_pgwire_response_delivery_seconds` | Summary | Time spent encoding and delivering a response |
| `rocklake_pgwire_response_rows_total` | Counter | Rows delivered to clients |
| `rocklake_pgwire_response_bytes_total` | Counter | Response bytes delivered to clients |
| `rocklake_pgwire_response_rows_per_second` | Gauge | Rows/sec for the last response |
| `rocklake_pgwire_response_bytes_per_second` | Gauge | Bytes/sec for the last response |
| `rocklake_pgwire_time_to_first_row_seconds` | Summary | Time from request start to first delivered row |
| `rocklake_pgwire_sql_classification_seconds` | Summary | SQL classifier latency |
| `rocklake_active_scans` | Gauge | Catalog scans currently holding a permit |
| `rocklake_max_active_scans` | Gauge | Maximum concurrent catalog scans |
| `rocklake_stream_queue_depth` | Gauge | Legacy configured value; no independent runtime effect |
| `rocklake_max_buffered_rows` | Gauge | Legacy configured value; no independent runtime effect |
| `rocklake_pgwire_peak_buffered_rows` | Gauge | Observed peak response rows buffered |
| `rocklake_max_response_bytes` | Gauge | Maximum response bytes allowed per request |
| `rocklake_process_rss_bytes` | Gauge | Current process resident set size |
| `rocklake_process_peak_rss_bytes` | Gauge | Peak observed process resident set size |
| `rocklake_resource_limit_exhaustions_total` | Counter | Requests rejected or stopped by a resource limit |
| `rocklake_stream_backpressure_total` | Counter | Stream backpressure observations |

`stream_queue_depth` and `max_buffered_rows` remain accepted configuration
keys for compatibility. They have no independent runtime effect, and RockLake
emits one warning when either key is configured. Do not add them to new
configuration files.

### Writer Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `rocklake_writer_epoch_age_ms` | Gauge | Milliseconds since the current writer epoch was acquired |

### CDC Data-Quality Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `rocklake_cdc_record_count_mismatch_total` | Counter | Times a Parquet file's scanned row count differed from catalog metadata (N-04 data-quality guard) |

## Capacity Rejection (SQLSTATE 53300)

When incoming load reaches or exceeds configured server thresholds, RockLake immediately rejects excess operations with SQLSTATE `53300` (`too_many_connections` or `configuration_limit_exceeded`) rather than degrading latency or causing unbounded memory growth:

- **Connection capacity (`--max-sessions`)**: When total open PG-wire client connections reach `--max-sessions`, new incoming connections are rejected with `53300`.
- **Concurrent scan capacity (`--max-active-scans`)**: When concurrent active scans reach `--max-active-scans`, admission control rejects new scans with `53300`.
- **Response byte limit (`--max-response-bytes`)**: If a single query generates a response exceeding `--max-response-bytes`, the query is terminated with `53300`.

Every capacity rejection increments the counter `rocklake_resource_limit_exhaustions_total`. Operators can observe headroom and set alerts on:
- Connection saturation ratio: `rocklake_connections_open / rocklake_max_sessions`
- Scan saturation ratio: `rocklake_active_scans / rocklake_max_active_scans`
- Rejection rate: `rate(rocklake_resource_limit_exhaustions_total[5m])`

## Alerting Rules

### Critical Alerts (Page Immediately)

```yaml
groups:
  - name: rocklake-critical
    rules:
      - alert: RockLakeDown
        expr: up{job="rocklake"} == 0
        for: 30s
        labels:
          severity: critical
        annotations:
          summary: "RockLake is down"
          description: "No metrics received from RockLake for 30 seconds"

      - alert: RockLakeSessionsExhausted
        expr: rocklake_connections_open / rocklake_max_sessions > 0.95
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "RockLake session capacity >95% — new connections will be rejected"
```

### Warning Alerts (Investigate Within Hours)

```yaml
      - alert: RockLakeStorageThrottling
        expr: rate(rocklake_object_store_throttles_total[5m]) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Object storage is throttling RockLake requests"

      - alert: RockLakeHighRetryRate
        expr: rate(rocklake_object_store_retries_total[5m]) > 5
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Elevated object-store retry rate — transient failures"

      - alert: RockLakeWriterEpochStale
        expr: rocklake_writer_epoch_age_ms > 300000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Writer epoch is more than 5 minutes old — check for stuck writer"

      - alert: RockLakeCDCMismatch
        expr: increase(rocklake_cdc_record_count_mismatch_total[1h]) > 0
        labels:
          severity: warning
        annotations:
          summary: "CDC record-count mismatch detected — Parquet file row counts differ from catalog metadata"
```

## Grafana Dashboard

### Recommended Panels

A comprehensive RockLake dashboard includes these panels:

**Row 1: Overview**

- Current connections (gauge) — `rocklake_connections_open`
- Session capacity (gauge) — `rocklake_connections_open / rocklake_max_sessions`
- Snapshots/min (graph) — `rate(rocklake_snapshots_created_total[1m])`

**Row 2: Object Storage**

- Storage requests/sec (graph) — `rate(rocklake_object_store_requests_total[1m])`
- Bytes read/written (graph) — `rate(rocklake_object_store_bytes_read_total[1m])` / `rate(rocklake_object_store_bytes_written_total[1m])`
- Throttle rate (graph) — `rate(rocklake_object_store_throttles_total[1m])`
- Retry rate (graph) — `rate(rocklake_object_store_retries_total[1m])`

**Row 3: Writer Health**

- Writer epoch age (graph) — `rocklake_writer_epoch_age_ms`
- Files per snapshot (graph) — `rocklake_files_per_snapshot`

**Row 4: Data Quality**

- CDC mismatch total (stat) — `rocklake_cdc_record_count_mismatch_total`
- Keys scanned per query (graph) — `rocklake_last_query_keys_scanned`

## Cloud-Native Monitoring Integration

### AWS CloudWatch

Use the CloudWatch Agent's Prometheus scraping to forward metrics:

```json
{
  "metrics": {
    "metrics_collected": {
      "prometheus": {
        "prometheus_config_path": "/etc/cwagent/prometheus.yaml",
        "emf_processor": {
          "metric_namespace": "RockLake",
          "metric_unit": {
            "rocklake_writer_epoch_age_ms": "Milliseconds",
            "rocklake_connections_open": "Count"
          }
        }
      }
    }
  }
}
```

### Google Cloud Managed Prometheus

On GKE with Managed Prometheus, the ServiceMonitor configuration works automatically — Google scrapes Prometheus endpoints and stores metrics in Cloud Monitoring.

### Datadog

Use the Datadog Agent's OpenMetrics integration:

```yaml
# datadog-agent/conf.d/openmetrics.d/conf.yaml
instances:
  - prometheus_url: http://rocklake:9090/metrics
    namespace: rocklake
    metrics:
      - rocklake_*
```

## What "Normal" Looks Like

Understanding baseline behavior helps identify anomalies:

| Metric | Healthy Range | Concerning |
|--------|--------------|------------|
| `rocklake_object_store_throttles_total` rate | 0 | Any sustained rate |
| `rocklake_object_store_retries_total` rate | < 1/min | > 5/min |
| `rocklake_connections_open` / `rocklake_max_sessions` | < 80% | > 95% |
| `rocklake_writer_epoch_age_ms` | < 60 000 ms | > 300 000 ms |
| `rocklake_cdc_record_count_mismatch_total` | 0 | Any increase |
| `rocklake_catalog_op_duration_seconds_sum / count` (per op) | < 0.5 s avg | > 2 s avg |
| `rocklake_pgwire_errors_total{sqlstate="40001"}` rate | 0 | Any sustained rate |
| `rocklake_gc_retain_from_snapshot` | Advancing each GC run | Static for > 7 days |
| `rocklake_slatedb_compaction_lag_ms` | < 5 000 ms | > 30 000 ms |

## OpenTelemetry Tracing (v0.39.0)

RockLake can export distributed traces to any OpenTelemetry-compatible backend
(Jaeger, Tempo, OTLP/HTTP collectors).

### Enabling OTLP Export

```bash
rocklake serve \
    --catalog s3://bucket/catalog/ \
    --otlp-endpoint http://jaeger:4318
```

Or via environment variable:

```bash
export ROCKLAKE_OTLP_ENDPOINT=http://jaeger:4318
rocklake serve --catalog s3://bucket/catalog/
```

When `--otlp-endpoint` is not set (the default), no spans are exported and
there is zero overhead.

### Instrumented Operations

The following catalog write paths are instrumented with OTLP spans:

| Span | Path |
|------|------|
| `create_snapshot` | Catalog snapshot commit |
| `register_data_file` | Data file registration |
| `commit_transaction` | Full transaction commit |
| PG-wire request lifecycle | Startup, query parse, execute, response |

### Jaeger Quick-Start

```yaml
# docker-compose.yml
services:
  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "4318:4318"   # OTLP HTTP
      - "16686:16686" # Jaeger UI
```

Then set `--otlp-endpoint http://localhost:4318` and open
`http://localhost:16686` to view traces.

## Further Reading

- **[Diagnostics](diagnostics.md)** — `rocklake diagnose` health report and orphan file sweep
- **[Health Checks](health-checks.md)** — Probing operational readiness
- **[Logging](logging.md)** — Complementary diagnostic information
- **[Troubleshooting](troubleshooting.md)** — Investigating alerts
- **[Configuration](../deployment/configuration.md)** — Metrics endpoint configuration
