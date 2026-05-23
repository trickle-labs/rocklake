# Excision

Excision is the physical deletion of catalog entries that are no longer visible to any valid reader. It is the second phase of garbage collection (after advancing the retention horizon) and is the only operation in SlateDuck that permanently destroys data. Because of its destructive nature, excision includes multiple safety checks and produces an audit trail.

## When to Use Excision

Excision is appropriate when:

- Storage costs for historical catalog data are significant (rare, but possible for very large catalogs)
- Compliance requirements mandate physical deletion of superseded metadata (GDPR, data retention policies)
- You want to reduce scan amplification from many superseded versions

Excision is NOT needed for:

- Hiding old snapshots from time travel (that is what `gc --retain-days` does)
- Routine maintenance of a normal-sized catalog (the storage cost of keeping old versions is negligible)
- Performance optimization (SlateDB's compaction handles storage efficiency at the LSM level)

## How It Works

Excision scans all keys in the catalog and identifies rows that meet ALL of these criteria:

1. The row has an `end_snapshot` set (it has been superseded)
2. The `end_snapshot` is before the specified `--before-snapshot` threshold
3. The `retain_from` system key is >= the specified threshold (safety check)

Rows meeting all criteria are physically deleted from SlateDB via tombstone writes.

## Safety Checks

Excision refuses to proceed if:

- `retain_from` has not been advanced past the excision target (you must run GC first)
- Any pinned snapshot would be affected by the deletion
- The specified `--before-snapshot` is in the future relative to `retain_from`

These checks prevent the accidental deletion of data that readers might still need.

## Running Excision

```bash
# Preview what would be deleted (dry-run)
slateduck excise --storage s3://bucket/catalog/ --before-snapshot 1000 --dry-run

# Execute the deletion
slateduck excise --storage s3://bucket/catalog/ --before-snapshot 1000 --operator "admin@company.com"
```

The `--operator` flag records who authorized the excision in the audit log.

## Audit Trail

Every excision creates an audit entry recording:
- Timestamp of the excision
- The `before_snapshot` threshold used
- Number of keys deleted
- Operator identity (if provided)

This audit entry is stored in the catalog itself (under `0xFF | "audit"`) and survives the excision (audit entries are never themselves excised).

## Recovery

Excision is irreversible. Once rows are deleted, they cannot be recovered from SlateDuck. Your recovery options are:

1. **NDJSON backup:** If you exported the catalog before excision, you can reimport the full state
2. **Object storage versioning:** If bucket versioning was enabled, you can recover the deleted objects at the SlateDB level (advanced procedure, requires SlateDB expertise)
3. **Cross-region replica:** If the excision has not yet replicated, you may be able to recover from the replica

For these reasons, always take an NDJSON export before running excision on production catalogs.
