# IVM Backup and Restore

This guide covers backup and restore operations for IVM state stores.

## Overview

Each materialized view shard maintains its incremental state in a SlateDB
state store. Backups pin a consistent snapshot of this state, allowing
point-in-time recovery without full recomputation.

## Creating a Backup

```bash
slateduck-ivm backup \
  --matview revenue_by_dept \
  --shard 0 \
  --output s3://bucket/backups/
```

The backup creates a **manifest** that records:
- The frontier (sequence position) at backup time
- List of pinned SST files
- SlateDB checkpoint ID

## Restoring from Backup

```bash
slateduck-ivm restore \
  --matview revenue_by_dept \
  --shard 0 \
  --manifest s3://bucket/backups/manifest.json
```

On restore:
1. The worker loads the backup manifest
2. State store is reset to the pinned checkpoint
3. Worker resumes from the backed-up frontier
4. Only post-backup CDC events are re-processed

## Automatic Rebuild on State Loss

If a state store is lost (e.g., due to S3 bucket deletion), the worker
can automatically rebuild from scratch:

```sql
ALTER MATERIALIZED VIEW revenue_by_dept
  SET (auto_rebuild_on_loss = true);
```

Without this flag, the worker logs a warning and waits for operator
intervention.

## Missing State Store Handling

When a worker claims a shard lease but finds no state store:

1. **Default behavior**: Log error, emit metric, wait for operator
2. **With `auto_rebuild_on_loss`**: Start REFRESH FULL automatically

## Backup Retention

Backups reference pinned SSTs. SlateDB's compaction will not delete
pinned files. To release old backups:

```bash
slateduck-ivm backup prune \
  --matview revenue_by_dept \
  --keep-last 3
```

## Operational Considerations

- Backups are lightweight (manifest + SST pins, no data copy)
- Restore is fast: only replay post-backup events
- Frequent backups reduce recovery time at minimal cost
- Cross-region restore requires SST replication (out of scope for v0.15)
