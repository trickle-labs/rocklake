# Tested compatibility matrix

This page lists the component and backend versions covered by CI. The public
policy for format, upgrade, deprecation, support, and scale claims is in the
root [`COMPATIBILITY.md`](https://github.com/trickle-labs/rocklake/blob/main/COMPATIBILITY.md).

## Support Matrix

| Level | Interfaces | Details |
|---|---|---|
| **Supported** | `rocklake` binary, PostgreSQL wire protocol, DuckDB DuckLake | Primary supported path with full CI test certification. |
| **Preview** | Rust client (`rocklake-client`), read-only API, Apache DataFusion | Preview read APIs; maintained with documented migrations. |
| **Experimental** | Language bindings (Python, Node.js, Java) and unmaintained engine integrations (Spark, Trino) | Experimental bindings without maintained certification paths. |
| **Internal** | Wire corpus, repair internals, low-level catalog key manipulations | Implementation details; no public stability guarantees. |

## DuckLake v1.0 Compatibility Claim

**RockLake v0.66.0 is CI-certified for DuckLake 1.0 with DuckDB 1.5.3–1.5.6 and 2.0.0, and for stable DuckLake 1.1 with DuckDB 2.0.0.** DuckDB 2.0 creates new catalogs as 1.1 by default; explicitly pin `DUCKLAKE_VERSION '1.0'` on `ATTACH` to keep the 1.0 format.

This claim is backed by the durable compatibility corpus and automated CI evidence:

| Acceptance Criterion | Status |
|----------------------|--------|
| DuckDB can attach fresh and create/drop schemas and tables; pin 1.0 on DuckDB 2.0 for the certified path | ✅ |
| Inlined and file-backed tables both work | ✅ |
| INSERT, DELETE, UPDATE, ALTER, DROP work for all 28 spec tables | ✅ |
| Views, macros, tags, column tags, sort info, partition columns work | ✅ |
| Fresh reads, restart reads, time-travel reads, projection reads are correct | ✅ |
| `postgres_query` can inspect every metadata table without RowDescription failures | ✅ |
| All 28 DuckLake v1.0 tables have exact SQL schemas | ✅ |
| Table stats, column stats, data-file metadata survive incremental commits | ✅ |
| Conflict checks and snapshot changes work under multiple writers | ✅ |
| Exact column schema, count, names, and OIDs under describe match the spec | ✅ |
| Catalog Version 7 (`V1_0`) is returned for the 1.0 path; existing 1.0 catalogs stay at 1.0 on ordinary attach | ✅ |

**CI Evidence**: The `ducklake-corpus` job in `.github/workflows/compatibility.yml`
runs on every push/PR and verifies the full DuckLake compatibility corpus. A nightly job
additionally runs the fresh, restart, and concurrent-writers scenarios.

## RockLake v0.66.0 Version Domains

`rocklake status --output json` reports independent version domains instead of
using the DuckLake schema version as a proxy for every persisted contract:

| Domain | Current version | Read | Write |
|---|---:|---|---|
| DuckLake catalog | 7 | 7 | 7 |
| RockLake catalog storage | 1 | 1 | 1 |
| Catalog backup manifest | 2 | 2 | 2 |
| Managed registry | 1 | 1 | 1 |
| Administrative job ledger | 1 | 1 | 1 |
| Audit schema | 1 | 1 | 1 |
| Public JSON | 1 | 1 | 1 |
| Evidence result | 1 | 1 | 1 |

The status report's `format_version` identifies the per-catalog DuckLake
metadata version (`1.0` or `1.1`); the numeric catalog version domain is
reported separately.

The direct-upgrade source is v0.60.0. The RockLake storage migration registry
is fail-closed: v0.66.0 ships a verified same-format no-op, while unknown
targets and all downgrades are rejected before mutation. DuckLake's separate
1.0-to-1.1 migration requires explicit `AUTOMATIC_MIGRATION`.

## DuckLake v1.1 Certified Support

Release Certification covers stable DuckLake 1.1 with DuckDB 2.0.0. The real
client scenarios exercise fresh catalogs, nested schemas, `NULL` promotion,
VARIANT inlining, epoch partitions, exact string bounds, row-group counts,
Puffin deletion history, view-column tags, table layout options, migration from
1.0, snapshot history, reconnect/restart, concurrent writers, and rejection by
DuckDB 1.5.6. Local and Silo-backed catalog and Parquet paths are included.
DuckDB 1.5.3–1.5.6 retain the separate DuckLake 1.0 compatibility matrix.

The certified boundary is DuckDB 2.0.0 for stable 1.1 through RockLake's
PostgreSQL-wire catalog. Silo tests do not certify real AWS S3, GCS, or Azure;
comparative performance parity has not been benchmarked. Catalog backup and
restore preserve metadata and file references, but do not copy Parquet bytes.
The C ABI and Python, Node.js, Go, and Java bindings expose selected high-level
projections, not arbitrary 1.1 metadata tables, 1.1-only fields, or catalog
import/export and migration operations. See the
[upgrade guide](operations/ducklake-version-upgrade.md) for migration and
recovery steps.

---

## DuckDB Client Versions

| DuckDB Version | Status | Notes |
|---------------|--------|-------|
| 1.5.3–1.5.6 | ✅ Supported | Live DuckLake 1.0 compatibility matrix. |
| 2.0.0 | ✅ Supported with DuckLake 1.0 and 1.1 | Stable 1.1 is the default; explicitly pin `DUCKLAKE_VERSION '1.0'` to preserve 1.0. |
| < 1.5.2 | ❌ Not supported | Older versions do not support DuckLake 1.0 |

For a stable DuckLake 1.1 catalog, DuckDB 2.0.0 is the tested client. An
end-to-end test confirms DuckDB 1.5.6 rejects the 1.1 format with a clear
version error and leaves the catalog version and snapshot history unchanged.
Other client versions have not been tested against migrated 1.1 catalogs.

## DuckLake Protocol

| Feature | Status | Version | Notes |
|---------|--------|---------|-------|
| DuckLake v1.0 catalog format | ✅ Supported | v0.27+ | All 28 spec tables |
| `ducklake_tag` SQL facade | ✅ Supported | v0.27+ | `tag_name`/`tag_value` per spec |
| `ducklake_column_tag` SQL facade | ✅ Supported | v0.27+ | `tag_name`/`tag_value` per spec |
| `ducklake_sort_info` SQL facade | ✅ Supported | v0.27+ | `sort_order`/`column_id` columns included |
| `ducklake_schema_version` SQL facade | ✅ Supported | v0.27+ | Single global version row |
| DROP TABLE CASCADE for tags | ✅ Supported | v0.27+ | All tag/column-tag rows retired |
| DROP TABLE CASCADE for sort_info | ✅ Supported | v0.27+ | All sort_info rows retired |
| `migrate-from-ducklake` CLI | ✅ Supported | v0.27+ | NDJSON source |
| `export-catalog` CLI | ✅ Supported | v0.27+ | NDJSON metadata export; 1.1 fields are covered by backup/restore round-trip tests |
| DuckLake compatibility corpus CI | ✅ Supported | v0.27.10 | Pinned DuckDB 1.5.3 / DuckLake 1.0 in CI |
| DuckLake v1.0-to-1.1 migration | ✅ Supported | DuckDB 2.0.0 | Explicit `AUTOMATIC_MIGRATION`; recovery and downgrade rejection are tested |

## SQL Clients

| Client | Version Tested | Status | Notes |
|--------|---------------|--------|-------|
| psql | 16, 17, 18 | ✅ Supported | Standard PostgreSQL client (v0.36.0: smoke tests for handshake, query, transaction, auth failure, TLS) |
| pgcli | 4.x | ✅ Supported | v0.36.0: connection setup, catalog SELECT, transaction, auth failure, TLS-required |
| DBeaver | 24.x | ✅ Supported | JDBC PostgreSQL driver 42.7.3 (v0.36.0: startup query sequence, schema browser validated) |
| Metabase | 0.49+ | ✅ Supported | PostgreSQL connection (v0.36.0: connection registration, catalog query validated) |

## Apache Spark

| Spark Version | Status | Protocol | Notes |
|--------------|--------|----------|-------|
| 3.5.x | ⚠️ Experimental | pg-wire | Wire-corpus validated in CI; community maintained |
| 3.4.x | ⚠️ Untested | pg-wire | Expected to work; not tested |
| 3.3.x | ❌ Not supported | Not applicable | Extended query protocol required |

## Trino / Presto

| Version | Status | Protocol | Notes |
|---------|--------|----------|-------|
| Trino 432+ | ⚠️ Experimental | pg-wire | Wire-corpus validated in CI; community maintained |
| Trino 400-431 | ⚠️ Untested | pg-wire | Expected to work |
| Presto | ❌ Not tested | Not applicable | May work; contributions welcome |

## Apache DataFusion

| DataFusion Version | Status | Notes |
|-------------------|--------|-------|
| Git revision `8f77b417` (55.1.0 development) | ⚠️ Preview | Parquet scan validated; use the same Git revision in downstream Rust applications |
| crates.io releases | ⚠️ Untested | Git and crates.io builds have distinct Rust trait identities |

## Object Storage Backends

| Backend | Version | Status | Notes |
|---------|---------|--------|-------|
| AWS S3 | Not specified | ✅ Supported | Via `object_store` 0.12 |
| Google Cloud Storage | Not specified | ✅ Supported | Via `object_store` 0.12; v0.36.0: GcsEmulatorHarness + backend_compat suite |
| Azure Blob Storage | Not specified | ✅ Supported | Via `object_store` 0.12; v0.36.0: AzureEmulatorHarness + backend_compat suite |
| MinIO | RELEASE.2024+ | ✅ Supported | S3-compatible endpoint |
| Local filesystem | Not specified | ✅ Supported | Development / testing; v0.36.0: LocalFS backend_compat suite |

## SlateDB

| SlateDB Version | Status | Notes |
|----------------|--------|-------|
| 0.13.x | ✅ Supported | Pinned in workspace |
| 0.12.x | ❌ Not supported | API incompatible |

## TLS

| TLS Version | Status | Notes |
|-------------|--------|-------|
| TLS 1.3 | ✅ Supported | Default; v0.36.0: acceptance verified by protocol-version gating test |
| TLS 1.2 | ✅ Supported | Via rustls; v0.36.0: acceptance verified by protocol-version gating test |
| TLS 1.1 or older | ❌ Rejected | Security policy; v0.36.0: rejection structurally guaranteed by rustls (no TLS11 constant) |

## Rust

| Rust Version | Status | Notes |
|-------------|--------|-------|
| Stable latest | ✅ Supported | Recommended |
| 1.95.0 (MSRV) | ✅ Supported | Minimum supported Rust version |
| < 1.95 | ❌ Not supported | |

## Platform

| Platform | Architecture | Status | Notes |
|----------|-------------|--------|-------|
| Linux (Ubuntu 22.04+) | x86-64 | ✅ Supported | Primary CI target; release binary provided |
| Linux | aarch64 | ✅ Supported | Release binary provided via cross-compilation |
| macOS 13+ | arm64 (Apple Silicon) | ✅ Supported | CI runs on `macos-latest`; release binary provided |
| macOS | x86-64 (Intel) | ❌ Not supported | Intel macOS is not tested in CI and no release binary is provided. All Apple hardware shipped since late 2020 uses Apple Silicon. Intel macOS support is deferred pending community demand. |
| Windows | x86-64 | ✅ Supported | CI runs on `windows-latest` (v0.38.0+); `rocklake.exe` and FFI DLL release artifacts provided |

## Version Policy

RockLake follows [Semantic Versioning](https://semver.org/).

- **Major (1.x.x)**: Breaking changes to the pg-wire API, catalog format, or FFI.
- **Minor (0.x.0)**: New features, backward-compatible changes.
- **Patch (0.0.x)**: Bug fixes and security patches.

Until `v1.0.0`, minor version bumps may include breaking changes to internal or experimental APIs.
