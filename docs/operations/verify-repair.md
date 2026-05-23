# Verify & Repair

SlateDuck includes tools for verifying catalog integrity and performing conservative repairs when issues are detected. These tools are designed for situations where you suspect corruption (unexpected errors, inconsistent query results) and need to diagnose and potentially fix the problem.

## Verify

The verify command performs a comprehensive integrity check of the catalog:

```bash
slateduck verify --storage s3://bucket/catalog/
```

It checks:

- **Format version:** Is the stored format version recognized by this SlateDuck binary?
- **Counter consistency:** Are all counter values higher than any existing ID of that type?
- **MVCC invariants:** For all versioned rows, is `end_snapshot > begin_snapshot` (when end is set)?
- **Referential integrity:** Do column rows reference existing tables? Do data file rows reference existing tables?
- **Value decoding:** Can all values be decoded from their protobuf representation?
- **Duplicate detection:** Are there duplicate keys that should not exist?

The output categorizes findings as errors (corruption that needs repair) or warnings (unusual but not necessarily broken):

```
Verify Results
==============
Tables checked:    23
Rows checked:      2,145
Errors:            1
Warnings:          2

Errors:
  [E001] Counter next_file_id (1800) is <= existing file ID 1892

Warnings:
  [W001] Table "archive.old_events" has 847 historical versions (consider GC)
  [W002] Orphaned inlined insert at key 0xFD010000...
```

## Repair

The repair command fixes issues identified by verify, using a conservative approach that refuses to act on anything it cannot safely resolve:

```bash
# Preview repairs (dry-run)
slateduck repair --storage s3://bucket/catalog/ --dry-run

# Apply repairs
slateduck repair --storage s3://bucket/catalog/
```

### What Repair Can Fix

- **Stale counters:** If a counter value is lower than existing IDs, repair advances it to the correct value
- **Orphaned inlined rows:** Inlined data rows that reference non-existent tables are removed
- **Dangling statistics:** File column stats referencing non-existent files are removed

### What Repair Cannot Fix

- **Protobuf decode failures for retained rows:** If a value cannot be decoded and the row is within the retention window, repair refuses to modify it (it might be needed by readers). The recommended action is to restore from a backup.
- **Magic byte mismatches:** Indicates deep corruption. Repair logs the key and recommends checkpoint restore or NDJSON reimport.
- **Missing system keys:** If critical system keys (format version, writer epoch) are missing, the catalog may need to be reinitialized.

### Safety Principle

Repair follows the principle: "First, do no harm." It will never delete data that might be needed by a valid reader. It will never modify rows within the retention window unless the modification is provably safe (like advancing a counter). When in doubt, repair reports the issue and recommends manual intervention.
