# Multi-Version Concurrency Control (MVCC)

SlateDuck uses multi-version concurrency control to allow concurrent readers at different points in time without any coordination or locking. Every versioned catalog entry carries visibility bounds (a `begin_snapshot` and an optional `end_snapshot`) that determine which readers can see it. This mechanism powers time travel, enables lock-free read scale-out, and provides the foundation for garbage collection.

## How Visibility Works

Every versioned row in the catalog has two fields:

- **`begin_snapshot`**: The snapshot ID at which this version became visible. Set when the row is first written.
- **`end_snapshot`**: The snapshot ID at which this version was superseded. NULL if the version is still current.

A row is **visible** at snapshot `S` if and only if:

```
begin_snapshot <= S AND (end_snapshot IS NULL OR S < end_snapshot)
```

This means a row is visible from the moment it was created until (but not including) the moment it was superseded. A reader at any snapshot sees exactly one version of each logical entity, or no version if the entity did not exist at that snapshot.

## Example: Table Rename

Consider a table "orders" created at snapshot 5 and renamed to "customer_orders" at snapshot 12:

| Key | begin_snapshot | end_snapshot | table_name |
|-----|---------------|--------------|------------|
| `0x05\|1\|42\|5` | 5 | 12 | orders |
| `0x05\|1\|42\|12` | 12 | NULL | customer_orders |

- Reader at snapshot 8 sees "orders" (5 <= 8 AND 8 < 12)
- Reader at snapshot 12 sees "customer_orders" (12 <= 12 AND end is NULL)
- Reader at snapshot 4 sees nothing (5 > 4, so the first row is not visible; second row has begin=12 > 4)

## Snapshot Isolation Semantics

Each read operation in SlateDuck is bound to a specific snapshot. When DuckDB connects and begins a session, SlateDuck determines the current snapshot (typically the latest) and all subsequent reads within that session see a consistent view at that snapshot. This provides **snapshot isolation**: the reader sees a frozen-in-time view of the catalog even as the writer is creating new snapshots concurrently.

The writer never modifies bytes that readers are currently examining. It only appends new rows (with new begin_snapshots) and sets end_snapshots on existing rows. Because SlateDB's underlying SST files are immutable, the bytes backing a reader's view are never touched by the writer. This is why readers never need locks and never conflict with the writer.

## The Latest Visible Version

For entities with multiple historical versions, SlateDuck needs to find the "current" one for a given snapshot. The function `latest_visible_version` scans all versions of an entity and returns the one with the largest `begin_snapshot` that is still visible at the target snapshot. This handles cases where an entity has been modified multiple times.

## Append-Only Tables

Not all catalog tables use full MVCC. Some are **append-only**: once written, they never get an `end_snapshot` set. Data files (`0x0B`) and delete files (`0x0C`) are examples. A data file, once registered, is visible forever (or at least until the retention horizon makes it logically invisible). Data files are not "superseded" by newer files; they accumulate.

For append-only tables, the visibility rule simplifies to:

```
begin_snapshot <= S
```

No end_snapshot check is needed because there is none.

## Mutable Singleton Tables

A third category is **mutable singleton**: tables like `ducklake_metadata` (`0x01`) where each logical entry (identified by scope + key) has exactly one current version. Updates replace the value in-place rather than creating a new versioned row. These are used for metadata like the catalog description or table comments where history is not important.

## Garbage Collection and MVCC

The `retain_from` system key controls the garbage collection horizon. When `retain_from` is set to snapshot N, it means: "readers should not attempt to read at snapshots before N." This does not immediately delete anything — it only constrains the set of valid read snapshots.

A superseded row (one with `end_snapshot` set) becomes GC-eligible when its `end_snapshot <= retain_from`. At that point, no valid reader can ever see it (because readers are not allowed to read before `retain_from`, and the row was superseded at `end_snapshot`). The row can be physically removed by excision without affecting any possible future read.

## Relationship to DuckLake Snapshots

DuckLake snapshots map one-to-one with SlateDuck's internal snapshot concept. When DuckDB creates a DuckLake snapshot (via `INSERT INTO ducklake_snapshot`), SlateDuck increments its snapshot counter and records the snapshot metadata. Subsequent catalog entries created in the same logical transaction use this snapshot ID as their `begin_snapshot`. Time travel in DuckDB (specifying a snapshot version when attaching a catalog) directly translates to SlateDuck reading at that snapshot ID.

## Performance Implications

MVCC scans are more expensive than non-MVCC scans because every row must be tested for visibility. However, the overhead is bounded: the visibility check is a simple integer comparison (two comparisons at most), and the number of versions per entity is typically small (most entities have 1-3 versions). The pathological case is an entity modified thousands of times, but this is rare in catalog metadata.

For the common case of listing current data files for a table, the MVCC overhead is minimal because data files are append-only (no end_snapshot to check) and the prefix scan already limits the result set to the correct table.
