# Snapshots

Snapshots are the fundamental unit of catalog versioning in SlateDuck. Every mutation to the catalog creates a new snapshot with a monotonically increasing ID. Snapshots enable time travel, provide audit history, define the scope of transactions, and control garbage collection boundaries. Understanding snapshots is essential for operating SlateDuck effectively.

## What a Snapshot Represents

A snapshot is a point-in-time view of the entire catalog. It captures the state of all schemas, tables, columns, views, macros, data files, and metadata at the exact moment the snapshot was created. Conceptually, snapshot N contains all catalog entries whose `begin_snapshot <= N AND (end_snapshot IS NULL OR N < end_snapshot)`.

Each snapshot is recorded as a row in the `ducklake_snapshot` table (tag `0x02`) with:

- **snapshot_id:** A unique monotonically increasing identifier
- **schema_version:** The catalog schema version at this snapshot (for format evolution)
- **snapshot_time:** UTC timestamp when the snapshot was created
- **author:** Optional identifier of who/what created the snapshot
- **message:** Optional human-readable description of what changed

Snapshots are append-only: once created, a snapshot row is never modified or deleted (until excision physically removes it).

## How Snapshots Are Created

DuckDB's `ducklake` extension creates a snapshot at the end of every logical transaction that modifies the catalog. The sequence is:

1. DuckDB sends `BEGIN` to start a transaction
2. DuckDB sends one or more catalog-modifying statements (CREATE TABLE, INSERT INTO ducklake_data_file, etc.)
3. DuckDB sends `INSERT INTO ducklake_snapshot` with the snapshot metadata
4. DuckDB sends `COMMIT`

SlateDuck allocates the next snapshot ID from its counter system, writes the snapshot row, and applies all buffered operations atomically. If any step fails, the entire transaction is rolled back (no partial snapshot is visible).

## Snapshot IDs and Ordering

Snapshot IDs are 64-bit unsigned integers that increase monotonically. The first snapshot in a new catalog has ID 1. Each subsequent snapshot increments by exactly 1. This sequential ordering is guaranteed by the single-writer model: there is no concurrent allocation race because only one process can allocate snapshot IDs.

The monotonic ordering means you can reason about causality: if snapshot A has a lower ID than snapshot B, then A happened before B. This is true even across process restarts because the counter is persisted in SlateDB.

## Time Travel

Because all historical snapshots are preserved (until excised), you can read the catalog at any historical point by specifying a snapshot ID. DuckDB supports this through the `SNAPSHOT` parameter in the ATTACH statement:

```sql
-- Read at the current latest snapshot
ATTACH '' AS current (TYPE ducklake, PG 'host=... port=5432');

-- Read at a specific historical snapshot
ATTACH '' AS historical (TYPE ducklake, PG 'host=... port=5432', SNAPSHOT '42');
```

When reading at snapshot 42, SlateDuck returns only the catalog entries visible at that snapshot. Tables created after snapshot 42 are invisible. Columns added after snapshot 42 are invisible. Data files registered after snapshot 42 are invisible. You see the catalog exactly as it was when snapshot 42 was created.

## Snapshot Changes

Each snapshot can optionally record what changed in the `ducklake_snapshot_changes` table (tag `0x03`). This provides a per-snapshot audit trail:

- **change_type:** What kind of change (create_schema, drop_table, register_data_file, etc.)
- **change_info:** Human-readable description of the change
- **schema_id / table_id:** Which entities were affected

This is useful for understanding catalog history without reconstructing it from the raw versioned rows.

## Snapshots and Garbage Collection

The `retain_from` system key defines the oldest snapshot that readers are allowed to access. When you advance `retain_from` to snapshot N (via `slateduck gc --retain-days 30`), it means:

- Readers cannot specify a snapshot older than N for time travel
- Catalog entries superseded before N are candidates for physical deletion (excision)
- Entries still active at N or later are protected regardless of when they were created

This two-phase approach (logical GC via `retain_from`, then optional physical deletion via excision) gives you full control over the trade-off between storage cost and historical access.

## Pinned Snapshots

Sometimes you need to prevent garbage collection from advancing past a specific snapshot. For example, a long-running ETL job might need to read at a fixed snapshot throughout its execution, even if garbage collection would normally advance past that point. SlateDuck supports **pinned snapshots** for this use case:

```bash
slateduck pin-snapshot --storage s3://bucket/catalog/ --snapshot-id 42
```

A pinned snapshot prevents `gc_apply` from advancing `retain_from` past it. The pin must be explicitly removed when no longer needed.

## Snapshot Metadata in Practice

Snapshots accumulate quickly in an active catalog. A data loading pipeline that runs every 5 minutes and registers new files will create 12 snapshots per hour, 288 per day, over 100,000 per year. Each snapshot row is small (approximately 100-200 bytes), so 100,000 snapshots occupy less than 20 MB of catalog storage. This is negligible.

However, the versioned rows (old column definitions, superseded table names, etc.) that accumulate across those 100,000 snapshots might be more significant. This is why garbage collection exists: to reclaim storage from versions that are no longer needed for time travel.
