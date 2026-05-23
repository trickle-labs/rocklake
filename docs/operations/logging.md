# Logging

SlateDuck uses structured logging via the `tracing` crate, providing configurable verbosity levels, JSON output for log aggregation systems, and contextual spans for correlating related log entries.

## Log Levels

| Level | What Gets Logged |
|-------|-----------------|
| `error` | Unrecoverable failures: storage errors, corruption detected, writer fencing |
| `warn` | Recoverable issues: throttled requests, retry attempts, deprecated features |
| `info` | Operational milestones: startup, writer epoch change, GC completion, session connect/disconnect |
| `debug` | Per-operation details: SQL classification results, cache hits/misses, scan counts |
| `trace` | Wire-level details: PG protocol messages, raw key bytes, value sizes |

The default level is `info`. Set via the `RUST_LOG` environment variable:

```bash
# Standard production logging
RUST_LOG=info slateduck --storage s3://bucket/catalog/

# Debugging catalog operations
RUST_LOG=slateduck_catalog=debug slateduck --storage s3://bucket/catalog/

# Full wire-level tracing (very verbose)
RUST_LOG=trace slateduck --storage s3://bucket/catalog/
```

## Structured Output

For log aggregation systems (Elasticsearch, Loki, CloudWatch Logs), enable JSON output:

```bash
SLATEDUCK_LOG_FORMAT=json slateduck --storage s3://bucket/catalog/
```

JSON log entries include structured fields for filtering and aggregation:

```json
{"timestamp":"2024-03-15T14:30:22Z","level":"info","target":"slateduck_pgwire","message":"session connected","fields":{"remote_addr":"10.0.1.5:52431","session_id":"abc123"}}
```

## Operational Scenarios

**Debugging a slow query:** Set `RUST_LOG=slateduck_catalog=debug` to see how many keys are scanned per operation and whether cache hits are occurring.

**Investigating a writer fencing error:** Set `RUST_LOG=slateduck_catalog=debug,slateduck_pgwire=debug` to see epoch checks and write attempts.

**Troubleshooting connectivity:** Set `RUST_LOG=slateduck_catalog=trace` to see individual object store requests and their latencies.

## Log Rotation

SlateDuck writes to stdout by default. Use your deployment platform's log rotation mechanism (Docker log drivers, systemd journal, Kubernetes log collection). Do not configure SlateDuck to write directly to files in production — let the platform handle log lifecycle.
