# Single Writer, Many Readers

SlateDuck uses a **single-writer** concurrency model for catalog mutations. At any given time, exactly one process is authorized to write to a catalog. Any number of processes can read concurrently, and they do so without coordination or locking. This model is simpler than multi-writer approaches, eliminates entire categories of concurrency bugs, and is appropriate for the DuckLake workload where catalog writes are infrequent and small.

## Why Single-Writer?

Catalogs are coordination points. They answer questions like "what is the next available snapshot ID?" and "what tables exist in schema X?" If multiple writers could modify the catalog concurrently, they would need to coordinate on ID allocation, handle write-write conflicts, and implement distributed locking or optimistic concurrency control. Each of these adds complexity and failure modes.

For DuckLake's workload, catalog writes are rare relative to reads. A typical analytics workflow might read the catalog hundreds of times per second (to resolve table schemas, list data files for queries, check column statistics) but write to it only a few times per minute (when data loading completes, when a schema change is applied). The single-writer model is not a bottleneck for this workload.

The benefits of single-writer are substantial:

**No write-write conflicts.** Because there is only one writer, there is never a case where two processes try to modify the same row simultaneously. No conflict resolution logic is needed.

**No distributed locking.** There is no need for a lock manager, lease system, or consensus protocol. The writer holds exclusive access by virtue of being the only writer.

**No split-brain.** There is no possibility of two processes both believing they are the writer and issuing conflicting mutations. Writer fencing (described below) ensures this even across process restarts.

**Simplified reasoning.** The catalog transitions through a linear sequence of snapshots. Each snapshot is the result of exactly one writer's actions. This makes debugging, auditing, and testing dramatically simpler.

## Writer Fencing

SlateDuck enforces single-writer semantics through **epoch-based fencing**. When a SlateDuck process starts and acquires the writer role, it increments the writer epoch stored in the catalog (a system key at `0xFF | "writer-epoch"`) and remembers the new epoch. Every subsequent write operation checks that the epoch in the catalog still matches the writer's epoch. If it does not match, the write fails with a `WriterFenced` error (SQLSTATE `57P04`).

This handles the case where a writer crashes and a new instance starts. The new instance increments the epoch, which invalidates any in-flight operations from the old writer (should it somehow recover and attempt to write). There is no window during which two writers can both succeed.

## Unlimited Readers

While writes are serialized through a single writer, reads are completely independent. Any number of processes can read the catalog concurrently by opening SlateDB in read-only mode and issuing GET/prefix-scan operations. Readers:

- Do not communicate with the writer
- Do not communicate with each other
- Do not hold any locks or leases
- Do not modify any state
- See a consistent snapshot (may lag behind the latest write by a few seconds)

This is possible because SlateDB's underlying storage is append-only (new SSTs are added, old ones are eventually removed by compaction but only after no reader references them). A reader that opened the catalog at time T will continue to see the state as of time T even as the writer advances.

## Reader Freshness

Readers may not see the absolute latest state because they discover new SSTs by reading the manifest, which is updated periodically. In practice, the lag is typically 0-5 seconds. For most analytics workloads, this is perfectly acceptable — you do not need sub-second freshness for a schema description or file list.

If you need guaranteed freshness (e.g., immediately after a write, verify the write took effect), you can re-open the catalog or explicitly refresh the reader's state.

## Multi-Writer via Dataset Partitioning

For workloads that genuinely need concurrent writers (e.g., multiple teams independently loading data into different tables), SlateDuck provides **dataset partitioning** through the `CatalogRegistry`. Instead of one big catalog for everything, you create multiple independent catalogs (one per dataset, team, or domain), each with its own single writer:

```
Global Registry (single catalog)
  ├── dataset: "team-alpha/events"  → s3://bucket/catalogs/alpha-events/
  ├── dataset: "team-alpha/users"   → s3://bucket/catalogs/alpha-users/
  ├── dataset: "team-beta/metrics"  → s3://bucket/catalogs/beta-metrics/
  └── dataset: "shared/reference"   → s3://bucket/catalogs/reference/
```

Each dataset has an independent writer, independent snapshot sequence, and independent garbage collection. Cross-dataset queries work by attaching multiple catalogs in DuckDB and joining across them.

This is an explicit partitioning model, not transparent sharding. You choose the partition boundaries based on your organizational and workload structure. The trade-off is that cross-dataset transactions are not atomic (each dataset advances independently).

## When Single-Writer Is Not Enough

If your workload requires atomic cross-dataset transactions, or if you genuinely need multiple concurrent writers to the same set of tables, SlateDuck may not be the right choice. Consider PostgreSQL-backed DuckLake instead, which provides full multi-writer transactional semantics through PostgreSQL's mature concurrency control.

However, in our experience, most analytics workloads are naturally partitioned by team, domain, or data source, and the single-writer-per-partition model works well.
