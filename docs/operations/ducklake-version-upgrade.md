# DuckLake Version Upgrade Guide

This guide covers existing DuckLake catalogs used through RockLake. Stable
DuckLake 1.1 with DuckDB 2.0.0 has passed release certification on the current
development tree. See the
[compatibility inventory](../integration/ducklake-1.1-compatibility-inventory.md)
for the tested scope and retained limits.

## Current Boundary

| DuckLake format | RockLake behavior |
|-----------------|-------------------|
| 1.0 | Supported. Ordinary attachment preserves 1.0. |
| Stable 1.1 | Release-certified with DuckDB 2.0.0 on the current development tree. New catalogs default to 1.1; existing RockLake 1.0 catalogs migrate only with explicit `AUTOMATIC_MIGRATION`. |
| 1.1-dev1 | Separate experimental importer only; requires `--accept-version V1_1_DEV_1`. This flag does not enable stable 1.1 migration. |

This in-place DuckDB migration is separate from `migrate-from-ducklake`, which
still rejects stable 1.1 source catalogs.

The tested migration uses DuckDB v2.0.0 with a RockLake PostgreSQL-wire
catalog. An explicit request for DuckLake 1.0 after migration is rejected and
leaves the catalog at 1.1. DuckDB 1.5.6 rejects a stable 1.1 catalog with a
version/catalog error and leaves its version and snapshot history unchanged.
DuckDB v2.0.0 is the tested client for migrated catalogs; other client versions
are not certified here. DuckLake does not support downgrading a migrated
catalog.

## Checking Your Source Catalog Version

Check the format version stored in `ducklake_metadata`:

```sh
sqlite3 /var/lib/ducklake/catalog.db \
  "SELECT value FROM ducklake_metadata WHERE key = 'version'"
```

The result `1.0` is accepted by default. `ducklake_snapshot.schema_version`
tracks schema changes within a catalog; it is not the format version. Only use
its maximum as a fallback for legacy catalogs that have no metadata version
row.

## Before Migrating a RockLake Catalog

1. Stop writes and cleanup jobs. Confirm every client that will access the
   catalog supports DuckLake 1.1; the tested client is DuckDB v2.0.0.
2. Back up the RockLake catalog with
   [`rocklake catalog backup create`](backup-restore.md) and separately back
   up or version the `DATA_PATH` files. A catalog backup contains metadata and
   file references, not Parquet data.
3. Keep the catalog and its referenced files at the same recovery point. Test
   the backup and restore into a separate catalog before using it as the
   production recovery path.
4. Use a RockLake account with catalog write access. Read-only attachment
   cannot migrate.

## Migrate with DuckDB

An ordinary attach leaves an existing 1.0 catalog at 1.0. Clear any global
version pin that forces 1.0, then request migration explicitly:

```sql
LOAD ducklake;
RESET ducklake_default_version;
ATTACH 'ducklake:postgres:host=localhost port=5432 dbname=rocklake' AS lake
  (AUTOMATIC_MIGRATION TRUE, DATA_PATH '/path/to/lake-data');
SELECT value FROM lake.options() WHERE option_name = 'version';
```

Verify the result is `1.1` before allowing other clients to write. Subsequent
attachments can omit `AUTOMATIC_MIGRATION`. Do not specify
`DUCKLAKE_VERSION '1.0'` or set `ducklake_default_version` to `1.0` for a
migrated catalog.

## Failure and Recovery

The migration is committed as one RockLake catalog snapshot. The recovery test
injects a failure at the snapshot pre-commit boundary after the migration
update has been staged, closes and reopens RockLake, verifies that the catalog
remains at 1.0 with its existing inlined rows, then retries the migration. It
does not simulate killing DuckDB or RockLake mid-process.

After an attach error, reconnect and query the catalog version before retrying.
If it is `1.0`, fix the reported cause and retry the explicit migration. If the
version is `1.1`, treat migration as complete and use 1.1-capable clients. If
the version or catalog state cannot be verified, stop writes and restore the
pre-migration catalog and matching data files from the backup before retrying.
Never try to set the version back to `1.0`.

## Migrating from DuckLake v1.1-dev1

```sh
rocklake migrate-from-ducklake \
  --source sqlite:/var/lib/ducklake/catalog.db \
  --catalog s3://my-bucket/rocklake/ \
  --accept-version V1_1_DEV_1
```

!!! warning
    The `1.1-dev1` prerelease format is distinct from stable DuckLake 1.1.
    Import it only when its source metadata is explicitly labeled `1.1-dev1`;
    this importer flag does not enable stable 1.1 migration.

## Version Gate and SQLSTATE

When a source format is not supported, RockLake returns:

```
ERROR 0A000: unsupported DuckLake catalog version <N>: ...
```

`SQLSTATE 0A000` is the PostgreSQL `feature_not_supported` code, which surfaces
correctly in clients that understand the PG wire protocol.

## See Also

- [Migration from DuckLake](migration-from-ducklake.md)
- [DuckLake 1.1 upgrade guide](https://ducklake.select/docs/stable/duckdb/migrations/upgrading_to_ducklake_1_1)
- [DuckLake backup and recovery guide](https://ducklake.select/docs/stable/duckdb/guides/backups_and_recovery)
