# DuckLake 1.1 Compatibility Inventory

**Status:** specification and verification inventory. The current support boundary is stated in the [compatibility page](../compatibility.md); this inventory records its coverage and limits.

**Baseline reviewed:** RockLake `main` at `92b8e40` (`v0.65.0`), DuckLake 1.1 stable documentation and release notes as published on 2026-10-21, reviewed 2026-11-05. The upstream documentation source is `duckdb/ducklake-web` `main` at `465f852b1aedde79936a1cd722f1be2f9003cfc1` (HEAD from `git ls-remote` on the review date); the served pages identify themselves as 1.1 stable. The DuckLake 1.0 pages were also consulted for the comparison.

## Baseline contract at `92b8e40`

At the reviewed baseline, RockLake's [compatibility page](../compatibility.md) and compatibility workflow claim DuckLake 1.0. DuckDB 2.0 tests explicitly pin `DUCKLAKE_VERSION '1.0'`; the corpus is pinned to catalog version 7 and has a gate against version 8. The existing experimental import token `V1_1_DEV_1` refers to a prerelease source version, not stable DuckLake 1.1 support. The implementation and certification evidence below supersede that baseline for stable 1.1.

The upstream 1.1 table overview lists 29 metadata tables, including the new `ducklake_view_column_tag`. RockLake's v1.0 catalog schema fixture and PgWire registry cover the v1.0 surface. An implementation primitive or generic string field is not evidence that a DuckLake 1.1 behavior works end to end.

## Specification and behavior inventory

| 1.1 requirement or change | RockLake evidence at `92b8e40` | Finding and follow-on |
|---|---|---|
| **Version selection and catalog lifecycle.** New catalogs default to 1.1 in DuckDB 2.0. Existing 1.0 catalogs stay at 1.0 unless `AUTOMATIC_MIGRATION` is requested; downgrade is unsupported. | RockLake's docs and `.github/workflows/compatibility.yml` pin DuckDB 2.0 tests to 1.0. `migrate_from_ducklake.rs` reads `MAX(schema_version)` from `ducklake_snapshot` as the source catalog version and recognizes numeric version 8 as `V1_1_DEV_1`. A local DuckDB 2.0 probe reported format `version = 1.1` in `ducklake_metadata` while the new catalog's initial snapshot `schema_version` was `0`. | DuckDB 2.0's native catalog supports selecting/reporting 1.0 and defaults a fresh catalog to 1.1. RockLake has no stable 1.1 path, and its current numeric import gate does not identify the stable format from the metadata field observed upstream. Reconcile version detection before migration or compatibility claims. [#110](https://github.com/trickle-labs/rocklake/issues/110), [#111](https://github.com/trickle-labs/rocklake/issues/111). |
| **Nested schemas.** `ducklake_schema.parent_schema_id` represents parentage and schema paths; child schemas must resolve independently in metadata and storage. | `SchemaRow` and `schema_schema()` in `crates/rocklake-core/src/rows.rs` and `crates/rocklake-pgwire/src/schema_registry.rs` have no `parent_schema_id`. Existing schema operations are in `crates/rocklake-catalog/src/writer/mod.rs`. | The metadata shape and nested create/list/drop/reconnect behavior need implementation and live verification. [#110](https://github.com/trickle-labs/rocklake/issues/110). |
| **`NULL` type.** The new type may appear at top level or inside nested types and can later be promoted. | Type parsing and pruning are in `crates/rocklake-core/src/types.rs`; the enum includes `Variant`, but the inspected parser has no `NULL` type case. | Add type and inlined-value coverage through DuckDB 2.0. Existing 1.0 clients must continue to reject or safely handle unsupported version-specific metadata. [#110](https://github.com/trickle-labs/rocklake/issues/110). |
| **`VARIANT` inlining for PostgreSQL and SQLite catalogs.** The inlining representation is Parquet variant metadata bytes followed by value bytes (`BYTEA`/`BLOB`); nested values need the documented encoding too. | `DuckLakeType::Variant`, inlined-table metadata, and file variant statistics exist in the core/catalog. No stable 1.1 byte-encoding round trip was found in the examined tests or PgWire surface. | The enum and statistics support do not prove binary inlining. Add exact byte round trips for top-level and nested variants, plus reconnect. [#110](https://github.com/trickle-labs/rocklake/issues/110), [#112](https://github.com/trickle-labs/rocklake/issues/112). |
| **Epoch partition transforms.** `epoch_year`, `epoch_month`, `epoch_day`, and `epoch_hour` use units from the Unix epoch, unlike calendar `year`/`month`/`day`/`hour`. | `PartitionColumnRow` stores transform text; the catalog reader/writer and PgWire partition handlers are in `crates/rocklake-catalog/src/` and `crates/rocklake-pgwire/src/`. No epoch-transform implementation or fixture was found. | Persisting the transform name alone is insufficient; verify accepted syntax, resulting partition values, pruning, and snapshot reads against DuckDB 2.0. [#110](https://github.com/trickle-labs/rocklake/issues/110), [#112](https://github.com/trickle-labs/rocklake/issues/112). |
| **Multi-snapshot Puffin delete files.** A delete file can carry deletion vectors from multiple snapshots while preserving time travel. | `DeleteFileRow` carries format and snapshot-window fields in `crates/rocklake-core/src/rows.rs`; the v1.0 PgWire registry and file writer APIs are in `schema_registry.rs` and `crates/rocklake-catalog/src/writer/mod.rs`. | The stable 1.1 `row_group_count` is absent from the row and registry. No Puffin or multi-snapshot deletion-vector round trip was found. Verify write, read, update/delete, and old-snapshot behavior before claiming support. [#110](https://github.com/trickle-labs/rocklake/issues/110), [#112](https://github.com/trickle-labs/rocklake/issues/112). |
| **Exact string statistics.** File and table column statistics add `min_is_exact` and `max_is_exact`; conservative truncated string bounds remain usable for pruning but are not exact aggregates. | `FileColumnStatsRow`, `TableColumnStatsRow`, the corresponding v1.0 PgWire schemas, and `DuckLakeType` pruning are in `rows.rs`, `schema_registry.rs`, and `types.rs`. Neither statistics row nor schema has the exactness flags. | Add the fields through write/read/export/backup surfaces and test long strings where Parquet bounds are truncated. Confirm `min`/`max` results do not treat conservative bounds as exact. [#110](https://github.com/trickle-labs/rocklake/issues/110), [#112](https://github.com/trickle-labs/rocklake/issues/112). |
| **Row-group counts.** `ducklake_data_file` and `ducklake_delete_file` add nullable `row_group_count` so readers can plan scans without fetching each footer. | `DataFileRow`, `DeleteFileRow`, and their PgWire schemas omit `row_group_count`. | Preserve null for old files, round-trip counts for new files, and verify counts for both data and delete files after restart. [#110](https://github.com/trickle-labs/rocklake/issues/110), [#112](https://github.com/trickle-labs/rocklake/issues/112). |
| **View column tags.** New `ducklake_view_column_tag` rows use the output column name before aliases, are snapshot-versioned, and retire with the view. | The v1.0 schema registry, SQL classifier, catalog row types, and writer expose table and table-column tags; no view-column-tag table or handler was found. | Add the 29th table and lifecycle semantics, including comments on aliased view columns, snapshot reads, and export/restore. [#110](https://github.com/trickle-labs/rocklake/issues/110), [#112](https://github.com/trickle-labs/rocklake/issues/112). |
| **Inlined-table migration.** Internal columns are renamed to `_ducklake_row_id`, `_ducklake_begin_snapshot`, `_ducklake_end_snapshot`; migration also adds missing schema-version rows and repairs defaults and incorrectly typed column bounds. | `InlinedDataTablesRow` is in `rows.rs`; migration-from-DuckLake has a prerelease import gate, but no stable `AUTOMATIC_MIGRATION` implementation or repair test was found. | This is a data-preserving catalog migration, not just version acceptance. Cover explicit opt-in, no-op attachment, all repairs, interruptions/recovery, and the no-downgrade boundary. [#111](https://github.com/trickle-labs/rocklake/issues/111). |
| **Features available without migration.** DuckDB 2.0 adds create-time sort/partition declarations, table options, broader statistics pruning, compaction selection controls, and Parquet import types. | `duckdb_2_0_non_migration_features_work_with_rocklake` in `crates/rocklake-pgwire/tests/duckdb_binary_tests.rs` exercises these paths against a fresh 1.1 catalog, including table-option set/reset, `OR`/nested `IS NULL` filters, compaction time windows and file limits, and importing `TIME_NS`, `TIMESTAMPTZ_NS`, and `VARIANT` data from Parquet. | The local DuckDB 2.0 scenario now confirms these paths end to end. It does not establish the same feature matrix on a 1.0 catalog or Silo-backed storage, and the import case covers a representative file rather than every supported Parquet type. [#112](https://github.com/trickle-labs/rocklake/issues/112). |
| **Storage, snapshots, and existing 1.0 contract.** Preserve attach, schema/table DDL, inlined and Parquet data, insert/update/delete, views/macros/tags, file/statistics metadata, restart and time-travel reads, and multi-writer behavior already covered by the 1.0 compatibility contract. | `docs/compatibility.md`, `tests/fixtures/ducklake-v1.0-schema.toml`, and the v1.0 corpus define that baseline. `.github/workflows/compatibility.yml` runs the DuckDB 1.5.3–2.0.0 matrix pinned to 1.0. `duckdb_parquet_and_catalog_share_s3_store` in `duckdb_container_tests.rs` exercises Silo-backed catalog and Parquet paths under distinct prefixes. | This is useful 1.0 and S3-compatible evidence, but it does not certify 1.1 or real AWS S3. Add separate 1.1 local and Silo scenarios, fresh-client/restart reads, history, concurrent writers, and preserve the current 1.0 jobs. [#110](https://github.com/trickle-labs/rocklake/issues/110), [#112](https://github.com/trickle-labs/rocklake/issues/112). |
| **Import/export, backup/restore, client and binding surfaces.** New catalog metadata must survive supported interfaces; unsupported operations must fail clearly rather than drop fields. | Catalog import/export logic is in `crates/rocklake-catalog/src/export.rs` and `migrate_from_ducklake.rs`; backup/restore and public clients live in `crates/rocklake-catalog/src/backup.rs` and `bindings/`. These are versioned around the current 1.0 model. | No 1.1 metadata round-trip matrix or documented binding limitations were found. Exercise each supported interface with the new columns/table and state any deliberate limitations. [#112](https://github.com/trickle-labs/rocklake/issues/112). |
| **DuckDB 2.0 performance claims.** The release post reports faster attach, metadata enumeration, inlining, and bulk catalog operations for the DuckDB extension with DuckDB and PostgreSQL catalogs. | RockLake has separate SlateDB/KV storage and PgWire catalog paths; the inspected compatibility suite reports correctness, not equivalent comparative benchmarks. | These upstream measurements do not transfer to RockLake. No comparable baseline is established here; avoid a performance parity claim unless RockLake is benchmarked under matching workloads. [#112](https://github.com/trickle-labs/rocklake/issues/112). |

## Reference client probe (run)

The installed DuckDB CLI reported `v2.0.0 (Cyanoptera) d54cbbe3df`. Against fresh local DuckDB metadata catalogs, `lake.options()` returned `version = 1.1` by default and `version = 1.0` when attached with `DUCKLAKE_VERSION '1.0'`. The 1.1 catalog's `ducklake_metadata` row also stored `version = 1.1`; its initial `ducklake_snapshot.schema_version` was `0`.

`pragma_table_info` against a fresh DuckDB 2.0 catalog enumerated all 29 metadata tables, with ordered columns, SQL types, and nullability matching the 1.1 specification's full schema script. Comparing it with the v1.0 schema confirmed the catalog delta: one new table (`ducklake_view_column_tag`); `parent_schema_id` on `ducklake_schema`; `row_group_count` on data and delete files; and `min_is_exact`/`max_is_exact` on file and table column statistics. The inlined-table system-column rename is a physical data-table change and is not represented in this metadata-table inventory. This reference capture does not exercise RockLake PgWire or data operations. A SQLite-backed probe could not run because `sqlite_scanner` was unavailable locally and network access prevented downloading it.

## Final verification evidence

The 1.0 schema manifest test is retained. The v1.1 schema delta is covered by
the registry tests for nested-schema parentage, data/delete row-group counts,
exact statistics columns, and view-column tags, alongside the DuckLake 1.1
critical-path certification. This checks the complete 1.0 baseline plus the
specified stable 1.1 catalog additions without duplicating the full manifest.

DuckDB 2.0.0 end-to-end coverage exercises fresh 1.1 creation, `NULL`
promotion, VARIANT inlining, epoch transforms, Puffin deletion history,
statistics and row-group metadata, view-column tags, inlined system-column
names, create-time table layout/options, pruning and compaction, and
representative Parquet import. Tests read catalog rows and verify query
results, historical snapshots, reconnect/restart, concurrent commits,
unsupported-client rejection, and 1.0-to-1.1 migration with recovery and
downgrade checks.

The metadata export/restore test creates a 1.1 backup and imports its NDJSON
catalog into a new catalog, then checks version, nested-schema parentage,
row-group counts, exactness flags, and view-column tags. The backup contains
catalog metadata and file references; it does not copy Parquet bytes. Local
and separate-prefix Silo-backed catalog and Parquet scenarios verify fresh
client reads. The final Release Certification workflow is the source for the
exact passing candidate and its 1.0/1.1 job results.

## Retained limits

- DuckDB 2.0.0 is the only client certified against stable DuckLake 1.1. DuckDB
  1.5.3–1.5.6 remain covered against DuckLake 1.0; DuckDB 1.5.6 has an explicit
  1.1 rejection/no-mutation check.
- Silo and emulator scenarios do not certify real AWS S3, GCS, or Azure.
- The Parquet import scenario covers representative types, not every supported
  Parquet type. Equivalent new-feature coverage on 1.0 catalogs is not claimed.
- The C ABI and Python, Node.js, Go, and Java bindings expose selected
  high-level projections; they do not provide arbitrary 1.1 metadata access or
  catalog import/export and migration APIs.
- Catalog backup and restore preserve metadata and file references, not the
  referenced Parquet bytes. Comparative performance parity is not benchmarked.

The 1.1 implementation and certification changes are tracked in PRs [#130](https://github.com/trickle-labs/rocklake/pull/130) and [#131](https://github.com/trickle-labs/rocklake/pull/131). The [compatibility page](../compatibility.md) records the support claim and tested versions.

## Sources

- [DuckLake 1.1 specification source at `465f852`](https://github.com/duckdb/ducklake-web/tree/465f852b1aedde79936a1cd722f1be2f9003cfc1/docs/stable/specification): [introduction](https://ducklake.select/docs/stable/specification/introduction), [data types](https://ducklake.select/docs/stable/specification/data_types), [queries](https://ducklake.select/docs/stable/specification/queries), and [table index](https://ducklake.select/docs/stable/specification/tables/overview).
- [DuckLake 1.1 release notes](https://ducklake.select/2026/10/21/ducklake-11/) and [upgrade guide](https://ducklake.select/docs/stable/duckdb/migrations/upgrading_to_ducklake_1_1).
- [DuckLake 1.0 specification](https://ducklake.select/docs/1.0/specification/introduction).
- RockLake sources and workflows named in the inventory, reviewed at `92b8e40`.
