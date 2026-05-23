# MVCC Filter

SlateDuck implements Multi-Version Concurrency Control at the key-value level. Each versioned entity (schema, table, column, view, macro) has a `begin_snapshot` and optionally an `end_snapshot`. The MVCC filter determines which rows are visible to a given reader based on the snapshot they are reading at.

## Visibility Rules

A row is visible at snapshot S if and only if:

```
begin_snapshot <= S AND (end_snapshot IS NULL OR end_snapshot > S)
```

In words: the row was created at or before snapshot S, and either it has not been superseded (end_snapshot is NULL) or it was superseded after snapshot S.

## Implementation

The MVCC filter is implemented in `crates/slateduck-core/src/mvcc.rs`. It operates on decoded row structs after they have been read from SlateDB and deserialized from protobuf:

```rust
pub fn is_visible(row: &impl Versioned, at_snapshot: u64) -> bool {
    row.begin_snapshot() <= at_snapshot
        && row.end_snapshot().map_or(true, |end| end > at_snapshot)
}
```

The filter is applied during prefix scans. When a reader scans for all columns of a table, it reads all column rows (all versions) and filters to only those visible at the target snapshot. For current-snapshot reads, this means selecting rows where `end_snapshot` is NULL.

## Performance Implications

The MVCC filter itself is trivially fast — two integer comparisons per row. The performance concern is scan amplification: reading N rows to find the K visible ones (where K << N if the entity has many historical versions).

For a freshly-created catalog (or one where GC + excision have been run recently), N ≈ K — almost every row is visible. For a catalog with extensive history, N may be significantly larger than K.

The filter does not require sorted order (it works on any set of rows), so it does not constrain the key layout. However, the key layout is designed so that all versions of the same entity are adjacent (same prefix, different `begin_snapshot` suffix), which means they are likely in the same SST block and can be fetched with one object storage read.

## Interaction with GC

Garbage collection advances the `retain_from` snapshot. After GC, readers cannot request snapshots before `retain_from`. This means the MVCC filter will never see requests for very old snapshots, which limits the range of relevant rows and allows excision to safely remove rows that are invisible to all valid snapshots.

## Non-Versioned Tables

Some catalog tables are not versioned (data files, delete files, file column stats, inlined inserts). These use a different visibility model: they are visible at a specific snapshot (they belong to the snapshot that created them) and are filtered by snapshot range rather than begin/end semantics.
