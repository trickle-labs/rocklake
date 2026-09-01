# Logging

RockLake writes text logs to stderr. The process does not manage log files or
rotation; use the host process supervisor or log collector for that work.

## Filtering

Use the standard `RUST_LOG` filter. It is read when the process starts.

```bash
RUST_LOG=info rocklake serve --catalog s3://bucket/catalog/
RUST_LOG=info,rocklake_catalog=debug rocklake serve --catalog s3://bucket/catalog/
RUST_LOG=rocklake_pgwire=trace rocklake serve --catalog s3://bucket/catalog/
```

The main targets are `rocklake`, `rocklake_catalog`, `rocklake_pgwire`,
`rocklake_sql`, `rocklake_core`, and `rocklake_datafusion`.

## Operational collection

When running under systemd, logs are available through journald:

```bash
journalctl -u rocklake -f
journalctl -u rocklake -p err
```

The v0.50.0 binary emits text logs only. JSON output and a `--log-format` flag
are not part of the supported release surface.
