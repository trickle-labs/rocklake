# Monitoring

SlateDuck exposes Prometheus-compatible metrics that give you visibility into catalog operations, resource usage, and performance characteristics. This page describes the available metrics, what they mean, and how to set up effective monitoring and alerting.

## Metrics Endpoint

SlateDuck exposes metrics in Prometheus text format. The metrics are collected in-process using atomic counters and can be scraped by any Prometheus-compatible monitoring system (Prometheus, Grafana Agent, Datadog, etc.).

## Available Metrics

### Operation Counters

| Metric | Type | Description |
|--------|------|-------------|
| `slateduck_snapshots_created_total` | Counter | Total snapshots created since process start |
| `slateduck_files_per_snapshot` | Gauge | Data files registered in the most recent snapshot |
| `slateduck_mean_rows_scanned` | Gauge | Average rows examined per read operation |

### Object Store Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `slateduck_object_store_requests_total` | Counter | Total requests to object storage (GET + PUT) |
| `slateduck_object_store_bytes_read_total` | Counter | Total bytes read from object storage |
| `slateduck_object_store_bytes_written_total` | Counter | Total bytes written to object storage |
| `slateduck_object_store_throttles_total` | Counter | Total 429/503 responses from object storage |
| `slateduck_object_store_retries_total` | Counter | Total retried requests |

### Session Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `slateduck_active_sessions` | Gauge | Currently connected clients |
| `slateduck_max_sessions` | Gauge | Maximum allowed concurrent sessions |

### Writer Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `slateduck_writer_epoch` | Gauge | Current writer epoch (increments on writer failover) |
| `slateduck_writer_epoch_age_ms` | Gauge | Time since this writer acquired the epoch |
| `slateduck_last_query_keys_scanned` | Gauge | Keys scanned in the most recent query |

## Recommended Alerts

**High object store throttle rate:** If `slateduck_object_store_throttles_total` increases rapidly, you are hitting S3 request rate limits. Consider request rate partitioning or prefix distribution.

**Writer epoch change:** A change in `slateduck_writer_epoch` means a failover occurred. Investigate why the previous writer died.

**Session saturation:** If `slateduck_active_sessions` approaches `slateduck_max_sessions`, new connections will be rejected. Scale up the session limit or investigate connection leaks.

**Stale writer:** If `slateduck_writer_epoch_age_ms` is very large with no snapshots being created, the writer may be alive but not processing requests. Check for deadlocks or resource exhaustion.

## Grafana Dashboard

A Grafana dashboard JSON is available in the repository at `docs/assets/grafana-dashboard.json`. It provides panels for operation throughput, latency percentiles, object store usage, and session counts.

## Integration with Cloud Monitoring

For AWS deployments, you can forward metrics to CloudWatch using the CloudWatch agent's Prometheus scraping feature. For GCP, use the Managed Prometheus service. For Azure, use Azure Monitor's Prometheus integration.
