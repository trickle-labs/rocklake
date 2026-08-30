# Excision

Excision physically removes catalog facts that are older than a chosen
snapshot. It is irreversible and should follow a verified logical backup.

## Preview

```bash
rocklake excise plan \
  --catalog s3://bucket/catalog/ \
  --before 1000
```

The plan reports eligible rows and whether the retention floor and snapshot
pins allow the operation.

## Apply

```bash
rocklake excise apply \
  --catalog s3://bucket/catalog/ \
  --before 1000
```

Apply only after confirming the plan and preserving any required export:

```bash
rocklake export-catalog --catalog s3://bucket/catalog/ --out pre-excision.ndjson
```

Excision is separate from `gc`: GC changes visibility, while excision writes
deletions for old catalog facts. Neither command deletes Parquet data files.
