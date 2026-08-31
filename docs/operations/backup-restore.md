# Backup and Restore

The v0.50.0 supported backup boundary is catalog export/import. The planned
`backup create`, `backup inspect`, `restore plan`, and `restore apply` commands
are not available in this release.

RockLake keeps catalog history in object storage. A logical backup is a
complete NDJSON export at an explicit snapshot; object-store versioning or a
separate bucket can provide an additional recovery boundary.

## Create a backup

```bash
rocklake inspect snapshot --catalog s3://prod/catalog/
rocklake export-catalog \
  --catalog s3://prod/catalog/ \
  --out catalog-backup.ndjson
```

To preserve a historical state, pass `--at-snapshot` with the exact snapshot
ID reported by `inspect`.

## Restore

```bash
rocklake import \
  --catalog s3://restored/catalog/ \
  --input catalog-backup.ndjson
rocklake verify catalog --catalog s3://restored/catalog/
rocklake inspect snapshot --catalog s3://restored/catalog/
```

Restore into a separate catalog prefix first. Keep the source catalog
available until verification and any application cutover are complete.

## Operational guidance

- Export before a migration or physical excision.
- Keep the snapshot ID with the backup filename and retention record.
- Use `rocklake verify catalog` after import.
- Do not treat an export as a Parquet data backup; the export contains catalog
  metadata and file references, not the data files themselves.
