# Backup & Restore

SlateDuck provides multiple mechanisms for backing up and restoring catalog state. Because the catalog lives in object storage, many traditional backup concerns (filesystem snapshots, WAL archiving) do not apply. Instead, SlateDuck provides logical exports (NDJSON), checkpoints (named restore points), and relies on object storage durability for physical backup.

## Object Storage as Backup

The simplest "backup" strategy is to rely on object storage's built-in durability (11 nines for S3 Standard). Your catalog data is already replicated across multiple availability zones by the cloud provider. In most scenarios, this is sufficient — you do not need an additional backup mechanism.

However, object storage durability protects against hardware failure, not against logical corruption or accidental deletion. For protection against these risks, use one of the mechanisms below.

## NDJSON Export

Export the catalog to a human-readable NDJSON (newline-delimited JSON) file:

```bash
slateduck export --storage s3://bucket/catalog/ --output catalog-backup.ndjson
```

The export includes all live rows at the current snapshot (or a specified snapshot), serialized as JSON objects with table name and field values. This format is useful for:

- Migrating catalogs between SlateDuck instances
- Auditing catalog contents with standard JSON tools (jq, DuckDB, pandas)
- Disaster recovery when the underlying SlateDB state is corrupt
- Archival for compliance or regulatory purposes

To restore from an NDJSON export:

```bash
slateduck import --storage s3://bucket/new-catalog/ --input catalog-backup.ndjson
```

This creates a fresh catalog populated with the exported rows. Snapshot IDs and counter values are reassigned during import (they are not preserved from the original).

## Checkpoints

Checkpoints are named restore points stored within the catalog itself:

```bash
slateduck checkpoint create --storage s3://bucket/catalog/ --label "before-migration"
```

A checkpoint records the current snapshot ID and timestamp. You can later restore to that point:

```bash
slateduck checkpoint restore --storage s3://bucket/catalog/ --label "before-migration"
```

Restoring a checkpoint resets the catalog's effective state to the checkpoint's snapshot. This is faster than NDJSON import (no data movement) but requires that the underlying rows still exist (i.e., excision has not removed them).

List available checkpoints:

```bash
slateduck checkpoint list --storage s3://bucket/catalog/
```

## Cross-Region Replication

For disaster recovery across regions, enable your cloud provider's cross-region replication feature on the bucket containing the catalog:

- **AWS:** S3 Cross-Region Replication (CRR)
- **GCS:** Multi-Region or Dual-Region buckets
- **Azure:** Geo-Redundant Storage (GRS) or Geo-Zone-Redundant Storage (GZRS)

SlateDuck does not need to know about cross-region replication — it is handled transparently at the storage layer. In a disaster recovery scenario, point a new SlateDuck instance at the replicated bucket in the secondary region.

## Versioned Buckets

Enabling object versioning on your bucket provides an additional safety net against accidental deletion or corruption. If SlateDB's compaction inadvertently loses data (extremely unlikely but possible with bugs), object versioning allows recovery of previous object versions.

This is particularly useful during the early adoption period when you may not yet fully trust the system's durability guarantees.
