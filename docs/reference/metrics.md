# Metrics Reference

This page lists all Prometheus metrics exposed by SlateDuck.

## Operation Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `slateduck_operations_total` | Counter | `operation` | Total operations by type |
| `slateduck_operation_duration_seconds` | Histogram | `operation` | Operation latency distribution |
| `slateduck_snapshots_created_total` | Counter | — | Total snapshots created |
| `slateduck_files_per_snapshot` | Gauge | — | Data files in the latest snapshot |

## Object Store Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `slateduck_object_store_requests_total` | Counter | `method` | Requests by method (GET, PUT, DELETE) |
| `slateduck_object_store_request_duration_seconds` | Histogram | `method` | Request latency distribution |
| `slateduck_object_store_bytes_read_total` | Counter | — | Total bytes read from storage |
| `slateduck_object_store_bytes_written_total` | Counter | — | Total bytes written to storage |
| `slateduck_object_store_throttles_total` | Counter | — | HTTP 429/503 responses |
| `slateduck_object_store_retries_total` | Counter | — | Retried requests |

## Cache Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `slateduck_cache_hits_total` | Counter | — | Cache hits (hot key + block cache) |
| `slateduck_cache_misses_total` | Counter | — | Cache misses requiring storage fetch |
| `slateduck_cache_size_bytes` | Gauge | — | Current cache memory usage |

## Session Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `slateduck_active_sessions` | Gauge | — | Currently connected clients |
| `slateduck_max_sessions` | Gauge | — | Session limit |
| `slateduck_sessions_total` | Counter | — | Total sessions created |

## Writer Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `slateduck_writer_epoch` | Gauge | — | Current writer epoch |
| `slateduck_write_batch_size` | Histogram | — | Keys per write batch |
| `slateduck_last_query_keys_scanned` | Gauge | — | Keys scanned in most recent query |
| `slateduck_mean_rows_scanned` | Gauge | — | Average rows scanned per read |

## Catalog Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `slateduck_schemas_count` | Gauge | — | Number of live schemas |
| `slateduck_tables_count` | Gauge | — | Number of live tables |
| `slateduck_latest_snapshot_id` | Gauge | — | Highest snapshot ID |
| `slateduck_retain_from` | Gauge | — | Current GC retention horizon |

## Metric Naming Conventions

All metrics follow Prometheus naming conventions:
- Prefix: `slateduck_`
- Suffix: `_total` for counters, `_seconds` for durations, `_bytes` for sizes
- Labels: lowercase with underscores
