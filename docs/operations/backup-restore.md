# Backup & Restore

RockLake stores all catalog state in object storage, which provides 99.999999999% (11 nines) durability by default. Your data is already replicated across multiple availability zones by the cloud provider, surviving hardware failures, rack outages, and natural disasters. In many scenarios, you do not need a separate backup mechanism at all — object storage IS the backup.

However, durability protects against physical loss, not logical corruption. If a bug corrupts your catalog, if an operator accidentally runs excision with wrong parameters, or if you need to undo a schema migration that went wrong, you need recovery mechanisms that work at the logical level. This page covers all backup and restoration strategies: from zero-cost approaches (leveraging storage durability) to full NDJSON exports and named checkpoints.

## Backup Strategy Decision Guide

| Risk | Protection | Strategy |
|------|-----------|----------|
| Hardware failure | Object storage durability | No action needed (built-in) |
| Accidental deletion (bucket) | Object versioning | Enable bucket versioning |
| Region outage | Cross-region replication | CRR / multi-region bucket |
| Logical corruption (bad write) | Checkpoints | Create before risky operations |
| Complete catalog loss | NDJSON export | Periodic full export |
| Schema migration rollback | Time travel / checkpoint | Checkpoint before migration |
| Compliance archive | NDJSON export | Monthly or quarterly exports |
| Migration to new instance | NDJSON export | One-time export and import |

## Object Storage as Backup (Zero-Cost)

The simplest backup strategy costs nothing and requires no configuration: rely on object storage's built-in durability.

When RockLake writes data to S3/GCS/Azure:

- S3 Standard: 99.999999999% durability (data replicated across 3+ AZs)
- GCS Standard: Same durability class
- Azure Blob (LRS): 99.999999999% durability within a single region
- Azure Blob (GRS): Same durability + cross-region copy

This means the probability of losing data due to storage infrastructure failure is astronomically low. For many teams, this is sufficient — especially combined with the time travel feature (which lets you access any previous catalog state within the retention window).

### When Object Storage Alone Is Not Enough

You need additional backup when:

1. **Logical corruption is possible** — a bug writes incorrect data to the catalog
2. **Retention window expires** — GC advances past the state you need
3. **Human error** — someone accidentally runs excision or deletes the bucket
4. **Compliance requires** — auditors want archived copies outside the live system
5. **Cross-system migration** — moving catalog state to a different RockLake instance

## NDJSON Export (Full Logical Backup)

NDJSON (Newline-Delimited JSON) export creates a complete, portable snapshot of the catalog in a human-readable format. The subcommand is `export-catalog`:

```bash
rocklake export-catalog --catalog s3://bucket/catalog/ --out catalog-backup.ndjson
```

### What Gets Exported

`export-catalog` exports all live rows at the active snapshot across **32 catalog tables** — the 28 tables defined in the DuckLake specification plus 4 RockLake extensions:

**DuckLake spec tables (28)**

| Table | Description |
|-------|-------------|
| `ducklake_snapshot` | Snapshot history |
| `ducklake_snapshot_changes` | Per-snapshot change log |
| `ducklake_schema` | Schema (namespace) definitions |
| `ducklake_table` | Table definitions |
| `ducklake_column` | Column definitions |
| `ducklake_data_file` | Registered data files (Parquet paths, sizes) |
| `ducklake_delete_file` | Delete files for row-level deletes |
| `ducklake_table_stats` | Aggregate table statistics |
| `ducklake_table_column_stats` | Per-column statistics |
| `ducklake_file_column_stats` | Per-file, per-column statistics |
| `ducklake_file_variant_stats` | Variant (nested) column stats |
| `ducklake_metadata` | Free-form catalog metadata key/value |
| `ducklake_view` | View definitions |
| `ducklake_macro` | Macro (UDF) headers |
| `ducklake_macro_impl` | Macro implementation bodies |
| `ducklake_macro_parameters` | Macro parameter definitions |
| `ducklake_tag` | Object-level tags |
| `ducklake_column_tag` | Column-level tags |
| `ducklake_partition_info` | Partition spec headers |
| `ducklake_partition_column` | Partition column bindings |
| `ducklake_file_partition_value` | Per-file partition values |
| `ducklake_sort_info` | Sort order headers |
| `ducklake_sort_expression` | Sort expression bindings |
| `ducklake_files_scheduled_for_deletion` | GC deletion queue |
| `ducklake_inlined_data_tables` | Inlined (small) table data |
| `ducklake_schema_version` | Schema evolution history |
| `ducklake_encrypted_secret` | Secret references (secrets **redacted**) |
| `ducklake_encryption_key` | Encryption key metadata (keys **redacted**) |

**RockLake extension tables (4)**

| Table | Description |
|-------|-------------|
| `ducklake_column_mapping` | Column ID remapping for schema evolution |
| `ducklake_name_mapping` | Source name→column ID resolution |
| `rocklake_snapshot_lease` | Writer fencing / lease records |
| `rocklake_extension_schema` | Extension-defined catalog tables |

> **Security note**: The `encrypted_secret` field in `ducklake_encrypted_secret` and the
> `encryption_key` field in `ducklake_encryption_key` are replaced with the literal string
> `<redacted>` in the export. Never store or transmit key material in plain text.

### Export Format

Each line is a self-contained JSON object with a `table` and `row` field:

```json
{"table":"ducklake_schema","row":{"schema_id":1,"schema_name":"analytics","begin_snapshot":1}}
{"table":"ducklake_table","row":{"table_id":1,"schema_id":1,"table_name":"events","begin_snapshot":2}}
{"table":"ducklake_column","row":{"column_id":1,"table_id":1,"column_name":"id","column_type":"BIGINT","begin_snapshot":2}}
```

### Export at a Specific Snapshot

Export the catalog as it appeared at a past snapshot:

```bash
# Export at snapshot 500
rocklake export-catalog --catalog s3://bucket/catalog/ --out backup-snap500.ndjson --at-snapshot 500
```

The following options are **planned** for a future release and are not yet implemented:

| Option | Status | Description |
|--------|--------|-------------|
| `--at-time <timestamp>` | Planned | Export at a wall-clock time |
| `--schema <name>` | Planned | Filter export to one schema |
| `--table <name>` | Planned | Filter export to one table |
| `--merge` | Planned | Merge export into existing catalog |
| `--dry-run` | Planned | Validate without writing |

### Export Size and Performance

Typical export sizes:

| Catalog Complexity | Export Size | Export Time |
|-------------------|-------------|-------------|
| Small (10 tables) | 50–100 KB | <1 second |
| Medium (100 tables) | 500 KB – 2 MB | 1–5 seconds |
| Large (1000 tables) | 5–20 MB | 5–30 seconds |
| Very large (10k tables) | 50–200 MB | 1–5 minutes |

### Automated Export Schedule

```bash
# Daily backup to a dated file (local storage example)
BACKUP_PATH="/backups/rocklake/$(date +%Y-%m-%d).ndjson"
rocklake export-catalog --catalog s3://bucket/catalog/ --out "$BACKUP_PATH"
```

Kubernetes CronJob:

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: rocklake-backup
spec:
  schedule: "0 2 * * *"  # Daily at 2 AM
  jobTemplate:
    spec:
      template:
        spec:
          containers:
            - name: backup
              image: ghcr.io/trickle-labs/rocklake:0.32.0
              command:
                - "sh"
                - "-c"
                - |
                  rocklake export-catalog \
                    --catalog s3://bucket/catalog/ \
                    --out /mnt/backup/rocklake-$(date +%Y-%m-%d).ndjson
          restartPolicy: OnFailure
```

## Restoring from NDJSON

Import an NDJSON backup into a fresh catalog using the `import-catalog` subcommand:

```bash
# Restore to a new catalog location
rocklake import-catalog --catalog s3://bucket/new-catalog/ --in catalog-backup.ndjson
```

### What Happens During Import

1. A fresh SlateDB instance is initialized at the storage path
2. Each NDJSON line is parsed and written as a binary key-value pair using the canonical key layout
3. Counter values (snapshot IDs, table IDs) are derived from the imported data; the first new snapshot starts after the highest imported snapshot ID
4. The import completes as a series of batched writes — it is **not** a single atomic commit for large catalogs
5. After import, run a normal catalog open to verify integrity

### Important: Snapshot IDs Are Not Preserved

Imported catalogs continue snapshot numbering from where the export left off. If external systems reference specific snapshot IDs from the original catalog, those references will be invalid after import. Table and column IDs **are** preserved — they are embedded in the data rows themselves.

## Checkpoints (Named Restore Points)

Checkpoints are lightweight named markers stored within the catalog itself. They record a snapshot ID and timestamp, allowing you to return to that exact catalog state later.

### Creating Checkpoints

```bash
# Before a risky migration
rocklake checkpoint create --catalog s3://bucket/catalog/ --label "before-v2-migration"

# Before bulk data registration
rocklake checkpoint create --catalog s3://bucket/catalog/ --label "pre-load-20241215"

# Manual checkpoint with description
rocklake checkpoint create --catalog s3://bucket/catalog/ \
    --label "release-3.2" \
    --description "Catalog state at release 3.2 deployment"
```

### Listing Checkpoints

```bash
rocklake checkpoint list --catalog s3://bucket/catalog/

# Output:
# Label                  Snapshot  Created               Description
# before-v2-migration    450       2024-12-10T14:30:00Z  
# pre-load-20241215      620       2024-12-15T09:00:00Z
# release-3.2            700       2024-12-18T16:00:00Z  Catalog state at release 3.2 deployment
```

### Restoring from Checkpoint

```bash
rocklake checkpoint restore --catalog s3://bucket/catalog/ --label "before-v2-migration"
```

Restoration works by setting the catalog's visible state back to the checkpoint's snapshot. Critically:

- **This is fast** — no data is moved, only the active snapshot pointer changes
- **Newer data still exists** — it becomes invisible (like time travel) but is not deleted
- **Requires rows to exist** — if excision has removed the checkpointed data, restoration fails
- **Creates a new snapshot** — the restoration itself is a committed operation

### Checkpoint Retention

Checkpoints interact with GC:

- GC will NOT advance past a checkpointed snapshot automatically
- Checkpoints act like implicit pins (they protect the snapshot they reference)
- To allow GC past a checkpoint, delete it first:

```bash
rocklake checkpoint delete --catalog s3://bucket/catalog/ --label "before-v2-migration"
```

## Cross-Region Replication

For disaster recovery across regions, use your cloud provider's built-in replication:

### AWS S3 Cross-Region Replication

```bash
aws s3api put-bucket-replication --bucket my-catalog-bucket --replication-configuration '{
    "Role": "arn:aws:iam::123456789012:role/s3-replication",
    "Rules": [{"Status": "Enabled", "Destination": {"Bucket": "arn:aws:s3:::my-catalog-dr"}}]
}'
```

### GCS Multi-Region Bucket

```bash
# Create as multi-region from the start
gsutil mb -l US gs://my-catalog-bucket/
```

### Azure GRS

```bash
az storage account create --name mycatalog --sku Standard_RAGRS
```

RockLake does not need to know about cross-region replication — it happens transparently at the storage layer.

## Bucket Versioning (Accidental Deletion Protection)

Enable object versioning for protection against accidental deletions:

```bash
# AWS
aws s3api put-bucket-versioning --bucket my-catalog-bucket \
    --versioning-configuration Status=Enabled

# GCS
gsutil versioning set on gs://my-catalog-bucket/

# Azure (soft delete with 30-day retention)
az storage blob service-properties delete-policy update \
    --account-name mycatalog --enable true --days-retained 30
```

With versioning enabled, if a SlateDB compaction bug or manual error deletes an object, the previous version remains recoverable.

## Disaster Recovery Procedures

### Scenario: Catalog Corrupted by Bad Write

1. Identify when corruption occurred (check logs, monitoring)
2. Restore from checkpoint (if one exists before the corruption):
   ```bash
   rocklake checkpoint restore --catalog s3://bucket/catalog/ --label "last-good"
   ```
3. Or restore from NDJSON backup:
   ```bash
   rocklake import --catalog s3://bucket/catalog/ --input last-backup.ndjson --overwrite
   ```

### Scenario: Bucket Accidentally Deleted

1. If versioning was enabled: recover deleted objects from versions
2. If CRR was enabled: point to the replica bucket in the secondary region
3. If NDJSON backups exist: import into a new bucket

### Scenario: Region Outage

1. Verify replication status of the secondary bucket
2. Deploy RockLake in the secondary region pointing to the replica
3. Remove `--read-only` to promote to writer (see [Multi-Region](../deployment/multi-region.md))

### Scenario: Need to Undo Schema Migration

1. If within retention window, use time travel:
   ```sql
   -- Check what the table looked like before migration
   SELECT * FROM lake.information_schema.columns AT SNAPSHOT 650;
   ```
2. Restore from pre-migration checkpoint:
   ```bash
   rocklake checkpoint restore --catalog s3://bucket/catalog/ --label "pre-migration"
   ```

## Testing Your Backup Strategy

Backups that have never been tested are not backups. Periodically verify:

```bash
# 1. Export current catalog
rocklake export --catalog s3://bucket/catalog/ --output test-backup.ndjson

# 2. Import into a test location
rocklake import --catalog s3://bucket/test-restore/ --input test-backup.ndjson

# 3. Start a read-only instance against the restored catalog
rocklake serve --catalog s3://bucket/test-restore/ --bind 127.0.0.1:5433 --read-only

# 4. Verify data is accessible
psql -h localhost -p 5433 -c "SELECT count(*) FROM ducklake_tables"

# 5. Clean up test location
rocklake destroy --catalog s3://bucket/test-restore/ --confirm
```

## Backup Scheduling Recommendations

| Workload Pattern | Export Frequency | Checkpoint Strategy | Retention |
|-----------------|-----------------|---------------------|-----------|
| Development / testing | Never (rely on storage durability) | Manual before experiments | N/A |
| Low-change production | Weekly | Before deployments | 30 days |
| Active production (daily changes) | Daily | Before deployments + migrations | 90 days |
| High-churn (hourly changes) | Every 6 hours | Automatic before any DDL | 30 days |
| Compliance-regulated | Daily + quarterly archive | Before every schema change | 7 years |

### Automating with Cron

```bash
# /etc/cron.d/rocklake-backup
0 2 * * * rocklake /usr/local/bin/rocklake export --catalog s3://prod/catalog/ --output s3://backups/rocklake/$(date +\%Y-\%m-\%d).ndjson 2>&1 | logger -t rocklake-backup
```

### Backup Monitoring

Alert if backups are not being created:

```yaml
- alert: RockLakeBackupStale
  expr: time() - rocklake_last_backup_timestamp_seconds > 172800  # 48 hours
  labels:
    severity: warning
  annotations:
    summary: "No RockLake backup in 48 hours"
```

## Restoring in Place vs. to New Location

You have two restoration approaches:

### Restore in Place (Overwrite)

Replaces the current catalog with the backup content:

```bash
rocklake import --catalog s3://bucket/catalog/ --input backup.ndjson --overwrite
```

**Danger:** This destroys any changes made after the backup was taken. All DuckDB clients must reconnect. Use only when the current catalog is known-corrupt.

### Restore to New Location (Side-by-Side)

Creates a new catalog without affecting the current one:

```bash
rocklake import --catalog s3://bucket/catalog-restored/ --input backup.ndjson
```

This allows you to:
- Compare the restored state with the current state
- Selectively apply corrections rather than wholesale rollback
- Test that the restoration is valid before switching over
- Keep the current catalog running while verifying the backup

## Further Reading

- **[Excision](excision.md)** — Physical deletion that makes restoration impossible
- **[Garbage Collection](garbage-collection.md)** — Retention policies affecting backup windows
- **[Export](export.md)** — Detailed export options and filtering
- **[Deployment: Multi-Region](../deployment/multi-region.md)** — Cross-region DR setup
