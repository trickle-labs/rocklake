# CLI Reference

RockLake v0.51.3 uses one typed Clap parser. Unknown commands, flags, and
positional arguments fail before any catalog is opened. Use `--help` on the
binary or a command for the complete generated reference.

## Commands

```text
serve
doctor
config check|example
backup create|inspect
restore plan|apply
gc plan|apply
excise plan|apply
checkpoint create|list|restore|pin|unpin|pins
export
import
pg-migrate
rebuild
inspect snapshot|api-costs|cache-utilization
verify catalog|data-files
repair
warmup
migrate
corpus diff|validate
tune
migrate-from-ducklake
export-catalog
diagnose
sweep-orphans
completions
```

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
rocklake backup create --catalog ./lake --out ./lake-backup
rocklake backup inspect ./lake-backup --output json
rocklake restore plan --backup ./lake-backup --catalog ./restored
rocklake restore apply --backup ./lake-backup --catalog ./restored
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
