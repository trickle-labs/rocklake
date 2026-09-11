# Backup and Restore

The v0.62.0 supported backup boundary is a versioned, snapshot-consistent
catalog artifact. `catalog backup create` writes the artifact; `catalog backup inspect`
validates its checksum and row count; `catalog restore plan` previews the target; and
`catalog restore apply` imports it into an empty catalog. Legacy flat invocations
(`rocklake backup` and `rocklake restore`) remain supported as backward-compatible aliases.

RockLake keeps catalog history in object storage. A logical backup is a
complete NDJSON export at an explicit snapshot; object-store versioning or a
separate bucket can provide an additional recovery boundary.

Catalog backup manifests are format `2` in v0.62.0. `rocklake status
--output json` reports this separately from the catalog storage format, so a
restore can be checked before any target is opened for writes. A backup from
v0.59.0 is in the supported direct-upgrade window.

## Create and inspect a backup

```bash
rocklake catalog backup create \
  --catalog s3://prod/catalog/ \
  --out catalog-backup/
rocklake catalog backup inspect catalog-backup/ --output json
```

To preserve a historical state, pass `--snapshot-id` with the exact snapshot
ID reported by `rocklake status` or `rocklake debug inspect snapshot`.

## Restore

```bash
rocklake catalog restore plan \
  --backup catalog-backup/ \
  --catalog s3://restored/catalog/ \
  --output json
rocklake catalog restore apply \
  --backup catalog-backup/ \
  --catalog s3://restored/catalog/
rocklake status --catalog s3://restored/catalog/
```

Restore into a separate catalog prefix first. For an existing target, pass
`--overwrite` only after the backup has been independently reviewed.

## Operational guidance

- Create a backup before a migration or physical excision.
- Keep the snapshot ID with the backup filename and retention record.
- Use `rocklake backup inspect` before every restore.
- Do not treat an export as a Parquet data backup; the export contains catalog
  metadata and file references, not the data files themselves.
