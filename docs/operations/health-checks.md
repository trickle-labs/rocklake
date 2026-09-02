# Health Checks

v0.51.3 does not publish a separate health HTTP endpoint. Use the PostgreSQL
wire listener for a liveness check and the read-only diagnostic commands for
catalog health.

## Liveness

Check that the listener accepts TCP connections, or run a trivial PostgreSQL
wire query with a compatible client:

```bash
nc -z 127.0.0.1 5432
psql -h 127.0.0.1 -p 5432 -c "SELECT 1"
```

## Catalog health

These commands open the catalog without modifying it:

```bash
rocklake diagnose --catalog ./catalog
rocklake diagnose --catalog ./catalog --json
rocklake verify catalog --catalog ./catalog
rocklake verify data-files --catalog ./catalog
```

Use `diagnose --json` for monitoring or CI ingestion. Use `verify catalog` to
check catalog key-value integrity and `verify data-files` to check registered
data-file accessibility.

## Metrics

Start the optional metrics listener with `--metrics-port`:

```bash
rocklake serve --catalog ./catalog --metrics-port 9090
curl http://127.0.0.1:9090/metrics
```

The metrics endpoint path can be changed with `--metrics-path` or
`ROCKLAKE_METRICS_PATH`. See [Metrics](../reference/metrics.md) for the
current metric names.
