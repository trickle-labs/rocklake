# Troubleshooting

Start with a read-only diagnostic report and preserve its output:

```bash
rocklake diagnose --catalog ./catalog --json > diagnose.json
rocklake verify catalog --catalog ./catalog
rocklake verify data-files --catalog ./catalog
```

## The server does not start

Check the catalog URL, object-store credentials, and listener address. The
binary creates a local catalog directory when running in writer mode. A reader
requires an existing catalog.

```bash
rocklake serve --catalog ./catalog --bind 127.0.0.1:5432
rocklake serve --catalog ./catalog --mode reader --bind 127.0.0.1:5433
```

If startup reports a writer-fencing error, another writer owns the catalog.
Stop the old writer before starting a replacement. Multiple read-only
instances can share the catalog.

## Clients cannot connect

Confirm the listener is reachable and that the client uses the configured
address, port, TLS mode, and credentials:

```bash
nc -z 127.0.0.1 5432
psql -h 127.0.0.1 -p 5432 -c "SELECT 1"
```

When password authentication is configured, set both `--auth-user` and
`--auth-password` (or the corresponding environment variables). TLS is
server-side only; mutual TLS is not supported.

## Catalog verification fails

Run the two verification commands separately to isolate catalog and data-file
problems:

```bash
rocklake verify catalog --catalog ./catalog
rocklake verify data-files --catalog ./catalog
```

Preview a repair before applying it:

```bash
rocklake repair --catalog ./catalog --dry-run
rocklake repair --catalog ./catalog --apply
```

Create a catalog export before applying repairs or excision:

```bash
rocklake export-catalog --catalog ./catalog --out before-change.ndjson
```

## Queries fail or return unexpected metadata

RockLake implements the bounded SQL surface emitted by DuckDB's `ducklake`
extension, not arbitrary PostgreSQL SQL. Check the client version and inspect
the catalog state:

```bash
rocklake inspect snapshot --catalog ./catalog
rocklake inspect cache-utilization --catalog ./catalog
rocklake inspect api-costs --catalog ./catalog
```

For historical catalog state, use an exact snapshot ID with
`export-catalog --at-snapshot <id>`. Snapshot ID `0` is an exact ID where an
API accepts an ID; it is not a latest sentinel.

## Logging

Use `RUST_LOG` to increase detail without exposing raw SQL or credentials:

```bash
RUST_LOG=info rocklake serve --catalog ./catalog
RUST_LOG=rocklake_catalog=debug,rocklake_pgwire=debug \
  rocklake serve --catalog ./catalog
```

See [Logging](logging.md), [Diagnostics](diagnostics.md), and
[Verify & Repair](verify-repair.md) for the supported operational commands.
