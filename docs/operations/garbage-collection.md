# Garbage Collection

SlateDuck's garbage collection is a two-phase process that manages catalog growth by first making old snapshots logically inaccessible (advancing the retention horizon) and then optionally removing the physical bytes of superseded rows (excision). This separation gives operators full control: you can preview what will be affected before committing to irreversible deletion.

## Why GC Is Needed

Because SlateDuck uses an append-only data model, every schema change creates new rows without removing old ones. Over time, a catalog accumulates superseded versions of schemas, tables, columns, and other entities. While each individual row is small (50-200 bytes), a catalog with thousands of schema changes per month will grow continuously.

Additionally, old snapshots remain queryable via time travel indefinitely by default. If you do not need to query snapshots older than 30 days, keeping them around wastes storage and increases scan times (more versions to filter through).

## Phase 1: Advance Retention Horizon

The first phase advances the `retain_from` system key, which defines the oldest snapshot that readers are allowed to access:

```bash
slateduck gc --storage s3://bucket/catalog/ --retain-days 30
```

This command:
1. Calculates the snapshot ID corresponding to 30 days ago
2. Checks for pinned snapshots that would prevent advancement
3. Updates `retain_from` to the calculated snapshot ID
4. Reports the number of snapshots that are now logically inaccessible

After this command, time travel queries specifying snapshots older than 30 days will fail. However, no physical data has been deleted yet. If you change your mind, you can reset `retain_from` to a lower value and restore access.

## Phase 2: Excision (Optional)

The second phase physically removes superseded rows. See [Excision](excision.md) for full details:

```bash
slateduck excise --storage s3://bucket/catalog/ --before-snapshot 1000
```

This permanently deletes key-value pairs that are no longer visible to any valid reader. It is irreversible.

## GC Planning

Before running GC, you can preview what will be affected:

```bash
slateduck gc --storage s3://bucket/catalog/ --retain-days 30 --dry-run
```

The dry-run output shows:
- Current `retain_from` value
- Proposed new `retain_from` value
- Number of snapshots affected
- Any pinned snapshots that would block advancement

## Pinned Snapshots

If a long-running process needs to read at a specific snapshot, it can pin that snapshot to prevent GC from advancing past it:

```bash
slateduck pin-snapshot --storage s3://bucket/catalog/ --snapshot-id 500
```

GC will refuse to advance `retain_from` past any pinned snapshot. You must unpin the snapshot before GC can proceed past it:

```bash
slateduck unpin-snapshot --storage s3://bucket/catalog/ --snapshot-id 500
```

## Recommended Schedule

For most production workloads, run GC daily with a retention period appropriate for your audit and debugging needs:

- **30 days:** Sufficient for most analytics workloads. Allows debugging recent issues with time travel.
- **7 days:** Appropriate for high-churn catalogs where storage costs are a concern.
- **90 days:** For compliance-sensitive environments where longer audit trails are required.
- **0 days (no retention):** Only for compliance deletion scenarios. Destroys all time travel capability.

Excision is less frequent — weekly or monthly — because it is more expensive (scans all keys) and less urgent (the storage savings are typically small relative to data file storage).
