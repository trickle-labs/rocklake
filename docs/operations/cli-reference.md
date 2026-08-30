# CLI Reference

RockLake v0.49.0 uses one typed Clap parser. Unknown commands, flags, and
positional arguments fail before any catalog is opened. Use `--help` on the
binary or a command for the complete generated reference.

## Commands

```text
serve
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

```bash
rocklake serve \
  --catalog <file://...,s3://...,gs://...,az://...> \
  [--bind <host:port>] \
  [--mode writer|reader] [--read-only] \
  [--max-sessions <n>] \
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
