# CLI Reference

RockLake v0.63.0 uses one typed Clap parser. Unknown commands, flags, and
positional arguments fail before any catalog is opened. Use `--help` on the
binary or a command for the complete generated reference.

## Primary Commands

```text
serve
doctor
status
support bundle
capacity report
catalogs validate|list|status|create|register|promote|activate|rename|set-mode|disable|enable|remove
registry init|status|backup|restore|verify|migrate-static|register-node|renew-node
catalog backup|restore|gc|excise|checkpoint|export|import|export-catalog|migrate|verify|repair|jobs|maintenance|recovery|backup-set
debug diagnose|inspect|corpus|rebuild|sweep-orphans|pg-migrate|tune|warmup
config check|example
completions
```

Legacy flat commands (e.g. `rocklake backup`, `rocklake export-catalog`, `rocklake diagnose`) remain supported as hidden aliases for backward compatibility.

`rocklake --version --output json` reports the semantic version, certified Git
SHA, target triple, Rust version, all persisted version domains, catalog
read/write formats, and provenance availability. Source builds report
unavailable release metadata as `unknown`. `rocklake status --output json`
reports the same domains plus the catalog's snapshot and compatibility policy.

### `export`

```bash
rocklake export --catalog <path> [--output <path>] [--snapshot-id <id>]
```

---

### `export-catalog`

```bash
rocklake export-catalog --catalog <path> [--out <path>] [--at-snapshot <id>]
```

---

### `import`

```bash
rocklake import --catalog <path> --input <path>
```

---

## Server options

The fastest local path is:

~~~bash
rocklake serve ./lake
~~~

`rocklake serve ./lake` creates a local catalog directory when needed. A
`rocklake.toml` file can provide the same settings; precedence is built-in
defaults, TOML, environment, then command-line flags.

## Operational commands

~~~bash
rocklake doctor --catalog ./lake [--output human|json]
rocklake config check [--file rocklake.toml] [--output human|json]
rocklake config example
rocklake support bundle --catalog ./lake --output ./rocklake-support
rocklake backup create --catalog ./lake --out ./lake-backup
rocklake backup inspect ./lake-backup --output json
rocklake catalog migrate --catalog ./lake --dry-run
rocklake catalog migrate --catalog ./lake --apply
rocklake restore plan --backup ./lake-backup --catalog ./restored
rocklake restore apply --backup ./lake-backup --catalog ./restored
rocklake catalog jobs list --catalog ./lake --output json
rocklake catalog jobs status --catalog ./lake --id <job-uuid> --output json
rocklake catalog jobs cancel --catalog ./lake --id <job-uuid>
rocklake catalog jobs resume --catalog ./lake --id <job-uuid>
rocklake catalog maintenance schedule --catalog ./lake --id nightly \
  --task backup --interval-seconds 86400
rocklake catalog maintenance run --catalog ./lake --limit 1
rocklake catalog recovery report --drill lost-process --started-at <rfc3339> \
  --rpo-seconds <n> --rto-seconds <n> --verified --output recovery.json
rocklake catalog backup-set create --registry <location> --output <directory>
rocklake catalog backup-set inspect <directory> --output json
rocklake catalog backup-set plan --input <directory> --registry <location> \
  --catalog-root <new-catalog-root> --data-root <new-data-root>
rocklake catalog backup-set apply --input <directory> --registry <location> \
  --catalog-root <new-catalog-root> --data-root <new-data-root>
rocklake catalogs validate
rocklake catalogs list --output json
rocklake catalogs status
rocklake registry init --registry <location>
rocklake registry status --registry <location> --output json
rocklake registry backup --registry <location> --output <directory>
rocklake registry restore --registry <location> --input <directory>
rocklake registry verify --registry <location>
rocklake registry migrate-static --registry <location> --request-id <id>
rocklake capacity report --catalog ./lake --output json \
  --pricing-file benchmarks/pricing/us-east-1-2026-09-11.json
rocklake registry register-node --registry <location> --node-id <node> \
  --endpoint <host:port> --request-id <id>
rocklake registry renew-node --registry <location> --node-id <node> \
  --lease-id <id> --request-id <id>
rocklake catalogs create --registry <location> --id <uuid> --alias <name> \
  --catalog <location> --data <location> --credential-provider <name> \
  --request-id <id>
rocklake catalogs register --registry <location> --id <uuid> --alias <name> \
  --catalog <location> --data <location> --credential-provider <name> \
  --request-id <id>
rocklake catalogs rename --registry <location> --id <uuid> --alias <name> --request-id <id>
rocklake catalogs set-mode --registry <location> --id <uuid> --mode read-only --request-id <id>
rocklake catalogs disable --registry <location> --id <uuid> --request-id <id>
rocklake catalogs enable --registry <location> --id <uuid> --request-id <id>
rocklake catalogs remove --registry <location> --id <uuid> --request-id <id>
rocklake catalogs promote --registry <location> --id <uuid> --node-id <node> \
  --endpoint <host:port> --expected-generation <generation> --request-id <id>
rocklake catalogs activate --registry <location> --id <uuid> --node-id <node> \
  --assignment-generation <generation> --writer-epoch <epoch> --request-id <id>
~~~

```bash
rocklake serve \
  [PATH | --catalog <file://...,s3://...,gs://...,az://...>] \
  [--bind <host:port>] \
  [--mode writer|reader] [--read-only] \
  [--max-sessions <n>] \
  [--max-active-scans <n>] [--stream-queue-depth <n>] \
  [--max-buffered-rows <n>] [--max-response-bytes <n>] \
  [--slow-operation-threshold-ms <n>] \
  [--metrics-port <port>] [--metrics-path <path>] \
  [--tls-cert <path>] [--tls-key <path>] [--tls-required] \
  [--auth-user <name>] [--auth-password <secret> | --auth-password-file <path>] \
  [--s3-endpoint <url>] [--s3-path-style] \
  [--encryption-key <64-hex-digits> | --encryption-key-file <path>] \
  [--extension-schemas <name,...>] [--otlp-endpoint <url>]
```

`--read-only` is a deprecated compatibility alias for `--mode reader`. The
explicit mode form is preferred.

## Snapshot selection

Commands that inspect or export catalog state use the latest snapshot by
default. `export` accepts `--snapshot-id`; `export-catalog` accepts
`--at-snapshot`. These are exact snapshot IDs; zero is not a latest sentinel.

## Safety boundaries

GC and excision expose separate `plan` and `apply` subcommands. `repair` and
`migrate` expose explicit `--dry-run` and `--apply` options. All destructive
operations remain explicit in the command syntax.

The v0.63.0 migration registry contains a verified same-format no-op. Unknown
targets and downgrades fail before a write; an apply attempt is recorded in the
administrative job ledger.

Restore plans for existing catalog destinations print an overwrite token.
Applying such a plan requires both `--overwrite` and the exact
`--overwrite-token`; new destinations do not need either flag. Backup sets are
metadata-only unless their manifests explicitly include referenced-object
inventory, so copying a backup never implies copying Parquet data.

Long-running backup, restore, export, import, verification, repair, retention,
excision, orphan-sweep, and rebuild operations record a durable job. Pass
`--idempotency-key` to reuse a retry identity. A failed, cancelled, or
abandoned job is never resumed automatically; inspect its checkpoint and use
`catalog jobs resume` explicitly.
operations remain explicit in the command syntax.

## Environment variables

The supported RockLake variables are documented in
[Environment Variables](../reference/environment-vars.md). Provider SDK
variables such as `AWS_REGION` remain supported for object-store credentials.

## v0.51 operator flow

The supported release flow is deliberately small:

```bash
rocklake serve --catalog file:///path/to/catalog
```

Then attach from DuckDB, create or query data, and use the existing operator
commands to inspect and validate the catalog:

```sql
LOAD ducklake;
ATTACH 'ducklake:postgres:host=127.0.0.1 port=5432 dbname=rocklake'
  AS lake (DATA_PATH '/path/to/data');
```

```bash
rocklake inspect snapshot --catalog file:///path/to/catalog
rocklake export-catalog --catalog file:///path/to/catalog --out catalog.ndjson
rocklake verify catalog --catalog file:///path/to/catalog
rocklake diagnose --catalog file:///path/to/catalog --json
rocklake backup create --catalog file:///path/to/catalog --out catalog-backup
rocklake restore plan --backup catalog-backup --catalog file:///path/to/restored
```
