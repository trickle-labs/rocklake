# DuckLake Version Upgrade Guide

This guide explains how to work with multiple DuckLake catalog versions in
RockLake and how to prepare for future version upgrades.

## Supported DuckLake Versions

| DuckLake format version | RockLake migration support |
|-------------------------|----------------------------|
| 1.0                     | Supported by default |
| 1.1-dev1                | Experimental — requires `--accept-version V1_1_DEV_1` |
| 1.1                     | Rejected until stable 1.1 metadata is supported |

RockLake currently supports DuckLake 1.0. It does not claim stable 1.1 support.

## Checking Your Source Catalog Version

Check the format version stored in `ducklake_metadata`:

```sh
sqlite3 /var/lib/ducklake/catalog.db \
  "SELECT value FROM ducklake_metadata WHERE key = 'version'"
```

The result `1.0` is accepted by default. The result `1.1-dev1` requires
`--accept-version V1_1_DEV_1`. Stable `1.1` is rejected until RockLake can
preserve its metadata. `ducklake_snapshot.schema_version` tracks schema changes
within a catalog; it is not the format version. Only use its maximum as a
fallback for legacy catalogs that have no metadata version row.

## Migrating from DuckLake v1.1-dev1

```sh
rocklake migrate-from-ducklake \
  --source sqlite:/var/lib/ducklake/catalog.db \
  --catalog s3://my-bucket/rocklake/ \
  --accept-version V1_1_DEV_1
```

!!! warning
    DuckLake v1.1 schema changes are not yet finalized. Migrating from a v1.1
    catalog may result in missing metadata for features introduced after v1.0.

## Version Gate and SQLSTATE

When a source format is not supported, RockLake returns:

```
ERROR 0A000: unsupported DuckLake catalog version <N>: ...
```

`SQLSTATE 0A000` is the PostgreSQL `feature_not_supported` code, which surfaces
correctly in clients that understand the PG wire protocol.

## Stable 1.1 Support

Stable DuckLake 1.1 catalogs remain unsupported by this importer. The
`--accept-version V1_1_DEV_1` flag applies only to the prerelease format and does
not bypass the supported v1.0 contract.

## See Also

- [Migration from DuckLake](migration-from-ducklake.md)
