# RockLake compatibility

This file defines RockLake's public compatibility policy before v1.0. The
[tested compatibility matrix](docs/compatibility.md) records component and
backend versions that CI covers.

## Supported product path

| Level | Interfaces |
|---|---|
| Supported | `rocklake` binary, PostgreSQL wire protocol, and DuckDB DuckLake. |
| Preview | Rust client, read-only API, and DataFusion integration. |
| Experimental | Language bindings and engine integrations without a maintained certification path. |
| Internal | Corpus, repair internals, and implementation-level exports. |

## Catalog and upstream versions

- RockLake targets DuckLake 1.0, Catalog Version 7 (`V1_0`).
- Each release records the DuckDB and DuckLake versions tested in the matrix.
- A new upstream release becomes supported only after its compatibility tests
  pass and the matrix names the release.
- A release records which catalog format versions it can read and write.
- An incompatible catalog or migration is rejected before a write can corrupt
  the target.

| Catalog format | Current read support | Current write support |
|---|---|---|
| DuckLake 1.0, Catalog Version 7 (`V1_0`) | v0.62.0 | v0.62.0 |

RockLake v0.62.0 reports independent version domains in `status --output json`:
DuckLake catalog `7`, catalog storage `1`, registry `1`, catalog backup `2`,
job ledger `1`, audit `1`, public JSON `1`, and evidence `1`. v0.60.0 is the
minimum direct-upgrade source. Unknown required formats and unsafe downgrades
are rejected before a write; a format-changing migration is not shipped in
v0.62.0.

## Metrics

The PG-wire query-duration histogram uses cumulative buckets. The `+Inf`
bucket equals `rocklake_pgwire_queries_total`. Query timing starts before SQL
classification and ends after response completion. Query logs and child
catalog spans carry a unique `query_id` and the connection's stable
`connection_id`.

The session metrics `rocklake_active_sessions` and `rocklake_idle_sessions`
remain deprecated aliases for `rocklake_connections_open` and
`rocklake_connections_idle` through v0.53.x. They are removed from the next
major metrics contract only after a documented migration.

## Upgrade and restore contract

Every thematic release tests this sequence:

1. A previous supported release creates a catalog.
2. The current release opens, reads, writes, and reopens the catalog.
3. A backup from the previous release restores under the current release.
4. An incompatible downgrade fails clearly before it can corrupt state.

The v0.60.0 migration command is read-only by default. It uses the typed
migration registry, records an administrative job when applied, verifies the
catalog before activation, and rejects unregistered targets. Since v0.60.0
does not change the catalog storage format, the supported apply path is a
verified no-op. v0.62.0 freezes the documented public surface and adds the
redacted support-bundle workflow without changing persisted catalog formats.

Backups and exports state whether they restore into a current release and which
catalog formats they contain. A catalog export contains metadata and file
references, not the referenced data files.

## CLI and configuration deprecations

For a public command, flag, or configuration setting:

1. Remove it from generated examples.
2. Continue accepting it during the deprecation window.
3. Emit one startup or config-check warning.
4. Document its replacement or state that it has no independent effect.
5. Remove it only after at least two minor releases.

The same window applies to renamed metrics. Keep the old metric as a
deprecated alias or publish a dashboard migration before removing it.

## Rust API policy

The Rust client and read-only API remain Preview. They may change between minor
releases when the migration is documented. The binary, PostgreSQL wire
protocol, and DuckDB DuckLake path follow the contracts in this file and the
tested matrix.

## Release support

Every release receives correctness and security fixes during its declared
support window. Security and correctness dependency updates can land in any
release. SlateDB, object-store, serialization, and other material dependency
changes rerun recovery and real-cloud baselines before the next scale claim.

## Object-store evidence

Functional support, recovery certification, and scale certification are separate
claims.

| Backend | Functional support | Recovery certified | Scale certified |
|---|---:|---:|---:|
| Local filesystem | Yes | Yes | Pending v0.52.0 |
| MinIO or S3-compatible | Yes | Yes | Pending v0.52.0 |
| AWS S3 | Yes | Yes | Pending v0.52.1 |
| Google Cloud Storage | Yes | Yes | Not yet |
| Azure Blob Storage | Yes | Yes | Not yet |
