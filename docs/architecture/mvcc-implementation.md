# MVCC Implementation

This page describes the storage-level implementation of multi-version concurrency control in SlateDuck. While the [Concepts: MVCC](../concepts/mvcc.md) page explains the theory, this page focuses on how MVCC is encoded in keys and values, how visibility filtering works at the code level, and how the different MVCC behaviors (versioned, append-only, mutable singleton) are implemented.

## MVCC Behaviors by Table

Not all catalog tables use the same MVCC strategy. SlateDuck defines three behaviors, assigned per table in the tag registry:

### Versioned Tables

Tables where entities can be modified over time. Each modification creates a new version with its own key (because `begin_snapshot` is part of the key). The previous version gets `end_snapshot` set to mark it as superseded.

**Tables:** ducklake_schema, ducklake_table, ducklake_column, ducklake_view, ducklake_macro

**Key structure:** `tag | ... | begin_snapshot` (begin_snapshot is the last key component)

**Visibility rule:** `begin_snapshot <= target AND (end_snapshot IS NULL OR target < end_snapshot)`

**Example:** A table renamed at snapshot 10 has two key-value pairs:
```
Key: 0x05|schema_id|table_id|5    Value: {name:"orders", end_snapshot:10}
Key: 0x05|schema_id|table_id|10   Value: {name:"customer_orders", end_snapshot:null}
```

### Append-Only Tables

Tables where entries are created once and never modified. There is no `end_snapshot` concept.

**Tables:** ducklake_snapshot, ducklake_snapshot_changes, ducklake_data_file, ducklake_delete_file, ducklake_macro_impl, ducklake_macro_parameters, ducklake_inlined_data_tables, ducklake_files_scheduled_for_deletion

**Key structure:** `tag | ... ` (no begin_snapshot in key; the value carries `snapshot_id` or `begin_snapshot`)

**Visibility rule:** `snapshot_id <= target` (or `begin_snapshot <= target`)

Data files, for example, are visible from the moment they are registered and remain visible forever (they are never superseded, only logically hidden by advancing `retain_from`).

### Mutable Singleton Tables

Tables where each logical entry has exactly one current version, updated in place (conceptually). SlateDuck implements this by overwriting the value for a fixed key.

**Tables:** ducklake_metadata (scope + key identifies the entry), ducklake_table_stats

**Key structure:** `tag | scope | scope_id | key` (no version component)

**Visibility rule:** Always visible (the current value is the only value)

## The Visibility Filter Function

The core MVCC function is `is_visible`:

```rust
pub fn is_visible(
    begin_snapshot: u64,
    end_snapshot: Option<u64>,
    dl_snapshot_id: u64,
) -> bool {
    begin_snapshot <= dl_snapshot_id
        && end_snapshot.map_or(true, |end| dl_snapshot_id < end)
}
```

This function is called for every row returned by a prefix scan in the reader. It is deliberately simple (two integer comparisons) because it sits in the hot path of every catalog read operation.

## Latest Visible Version Resolution

For versioned tables, a reader needs the "current" version of an entity at a given snapshot. The function `latest_visible_version` handles this:

```rust
pub fn latest_visible_version<T>(
    versions: impl Iterator<Item = (u64, Option<u64>, T)>,
    dl_snapshot_id: u64,
) -> Option<T> {
    versions
        .filter(|(begin, end, _)| is_visible(*begin, *end, dl_snapshot_id))
        .max_by_key(|(begin, _, _)| *begin)
        .map(|(_, _, row)| row)
}
```

It filters to visible versions, then selects the one with the largest `begin_snapshot`. This handles the case where an entity has been modified multiple times: the reader sees the most recent version that is visible at their snapshot.

## GC Eligibility

A superseded row becomes eligible for physical deletion (excision) when no valid reader can ever see it:

```rust
pub fn is_insert_gc_eligible(
    end_snapshot: Option<u64>,
    oldest_retained: u64,
) -> bool {
    end_snapshot.map_or(false, |end| end <= oldest_retained)
}
```

A row with `end_snapshot <= retain_from` is GC-eligible because:
- All readers must read at `snapshot >= retain_from`
- The row is invisible at `snapshot >= end_snapshot` (which is <= retain_from)
- Therefore, no valid reader can see it

## Inlined Data MVCC

Inlined data (small rows stored directly in the catalog) uses a slightly different MVCC model:

**Inlined inserts** have both `begin_snapshot` and optional `end_snapshot`, using the standard visibility rule. The `end_snapshot` is set when the inlined row is logically deleted (marked for deletion in a subsequent snapshot).

**Inlined deletes** have only `begin_snapshot`. They record that a specific row in a data file has been deleted as of a certain snapshot. Visibility is: `begin_snapshot <= target`.

## Secondary Index and MVCC

The secondary index (tag `0xFC`) provides snapshot-scoped file lookups. Its key includes `snapshot_id`, allowing efficient queries like "what files were registered at snapshot 42 for table T?" without scanning all files for the table. This is an optimization for change tracking, not a replacement for the standard data file scan.

## MVCC and Compaction

SlateDB's compaction process merges SST files, but it does not understand MVCC semantics. It treats all key-value pairs as opaque. This means superseded MVCC versions (rows with `end_snapshot` set) remain in storage until explicitly removed by SlateDuck's excision process.

SlateDB's own tombstones (for deleted keys) are handled by SlateDB compaction. But SlateDuck never deletes keys during normal operation — it only sets `end_snapshot` in the value. Physical key deletion only happens during excision.

## Performance Considerations

The MVCC filter adds per-row overhead to every scan. For tables with many historical versions (e.g., a column that has been altered 100 times), the scan must examine all 100 versions to find the visible one. In practice, this is rarely a problem because:

1. Most entities have 1-3 versions
2. The filter is two integer comparisons (very fast)
3. SlateDB's block cache means the bytes are usually in memory
4. Garbage collection (advancing retain_from + excision) removes old versions

For pathological cases, the secondary index provides an alternative access path that avoids scanning all historical versions.
