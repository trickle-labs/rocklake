# Garbage Collection

Garbage collection advances the catalog's visibility floor. It does not
physically delete catalog facts. The operation is split into a preview and an
explicit apply command.

## Preview the retention change

```bash
rocklake gc plan \
  --catalog s3://bucket/catalog/ \
  --retention-days 30
```

The plan reports the current and proposed retention floor plus any pinned or
leased snapshots that affect the change.

## Apply the retention change

```bash
rocklake gc apply \
  --catalog s3://bucket/catalog/ \
  --retention-days 30
```

Applying GC makes snapshots older than the new floor unavailable to readers.
The underlying objects remain until an explicit excision operation.

## Scheduling

Schedule `gc apply` with the host's process supervisor. For example, a daily
cron entry is:

```cron
0 3 * * * /usr/local/bin/rocklake gc apply --catalog s3://bucket/catalog/ --retention-days 30
```

Use [Excision](excision.md) only when physical deletion is required.
