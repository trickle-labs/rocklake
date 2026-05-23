# Immutability

Immutability is the single most important design principle in SlateDuck. Once a catalog entry is written, it is never modified in place. Updates create new versions. Deletes mark existing versions as superseded. The original bytes remain in storage until explicitly removed by an operator-initiated excision process. This principle enables time travel, simplifies crash recovery, eliminates read-write conflicts, and makes horizontal read scale-out possible without coordination.

## What Immutability Means in Practice

When you create a table in SlateDuck, a `TableRow` is written to SlateDB with a `begin_snapshot` recording the snapshot at which it became visible. If you later rename that table, SlateDuck does not overwrite the original row. Instead, it sets an `end_snapshot` on the original row (marking it as superseded from that point forward) and writes a new row with the updated name and a new `begin_snapshot`. Both the old and new versions coexist in storage.

This pattern applies to all versioned entities: schemas, tables, columns, views, macros, and inlined data. The catalog is an append-only log of facts, each annotated with the time range during which it was true.

## Why Immutability?

The benefits of immutability compound across multiple dimensions of the system:

**Time travel is free.** Reading the catalog at any historical snapshot is simply a matter of filtering rows by their visibility bounds. There is no need to maintain separate historical copies, WAL replay, or point-in-time snapshots. The entire history is always present (until excised).

**Crash safety is automatic.** If the process crashes during a write, one of two things is true: either the write completed (it's in the WAL and will be visible after recovery) or it didn't (the bytes were never made durable). There is no possibility of a partial update corrupting existing data because existing data is never modified.

**Readers never block writers, and writers never block readers.** A reader operating at snapshot N sees a fixed, immutable set of rows regardless of what the writer is doing. The writer can create snapshot N+1 concurrently without affecting the reader's view. This is possible because writes only append new rows and set end-snapshots; they never modify the bytes that the reader is currently examining.

**Horizontal read scale-out requires no coordination.** Because the underlying SST files in SlateDB are immutable, any number of processes can read them concurrently via object storage GET requests. There is no lock manager, no read-write lock, no lease system. Readers are completely independent.

## The Cost of Immutability

Nothing comes for free. Immutability has two costs:

**Storage grows monotonically.** Every schema change, every table creation, every column addition creates new rows that are never automatically reclaimed. For most workloads this is negligible (catalog metadata is tiny compared to data), but catalogs with very high churn (thousands of schema changes per day) can accumulate significant historical data.

**Physical deletion requires explicit action.** If you have regulatory requirements to delete data (GDPR right to erasure applied to metadata, for example), you must run the two-phase garbage collection process: first advance the retention horizon with `gc`, then physically remove old rows with `excise`. This is deliberate — destructive operations should be explicit, not automatic.

## How SlateDuck Manages Growth

SlateDuck provides three mechanisms to manage catalog growth:

1. **Visibility GC** advances the `retain-from` horizon. Snapshots older than the horizon are no longer queryable via time travel, but the actual bytes remain. This is a logical deletion that can be reversed by resetting the horizon (as long as excision has not occurred).

2. **Excision** physically removes superseded rows whose `end_snapshot` is before the retention horizon. This is irreversible and permanently destroys historical data. It is gated behind safety checks: excision refuses to proceed if the retention horizon has not been advanced past the target snapshot.

3. **SlateDB compaction** reclaims storage at the LSM level by merging SST files and dropping tombstoned entries. This happens automatically in the background and is orthogonal to the catalog-level GC.

## Immutability and Consistency

The combination of immutability and single-writer semantics gives SlateDuck a very strong consistency model for its catalog. Within a single writer's session, every read observes a consistent snapshot. Across concurrent readers, every reader observes a consistent snapshot (though different readers may observe different snapshots if the writer has advanced the state between their reads). There is never a case where a reader sees a partial transaction or an inconsistent mix of old and new versions.

This consistency model is similar to PostgreSQL's MVCC (which also uses begin/end transaction IDs to control visibility) but simpler because there is only one writer and no need for vacuum or XID wraparound handling.
