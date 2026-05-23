# Multi-Region Deployment

Multi-region deployment allows SlateDuck to serve catalog queries from multiple geographic regions with low latency. Because SlateDuck uses object storage as its durable layer, multi-region deployment leverages your cloud provider's cross-region replication features.

## Architecture

```
Region A (Primary)                Region B (Read Replica)
┌──────────────────┐              ┌──────────────────┐
│  SlateDuck       │              │  SlateDuck       │
│  (Writer)        │              │  (Read-Only)     │
│  └─→ S3 (A)     │──CRR──────→  │  └─→ S3 (B)     │
└──────────────────┘              └──────────────────┘
```

The primary region hosts the write-capable SlateDuck instance. Cross-Region Replication (CRR) copies all object storage data to the secondary region. A read-only SlateDuck instance in the secondary region serves local queries against the replicated data.

## Setup

### 1. Enable Cross-Region Replication

Configure your cloud provider's replication:

**AWS S3:**
```bash
aws s3api put-bucket-replication \
  --bucket source-bucket \
  --replication-configuration file://replication.json
```

**GCS:** Use Multi-Region or Dual-Region bucket when creating the bucket.

**Azure:** Enable GRS (Geo-Redundant Storage) on the storage account.

### 2. Deploy Read Replica

In the secondary region, deploy a SlateDuck instance in read-only mode:

```bash
slateduck --storage s3://replicated-bucket/catalog/ --bind 0.0.0.0:5432 --read-only
```

The `--read-only` flag prevents this instance from attempting writes and from competing for the writer epoch.

### 3. Client Routing

Route clients to their nearest SlateDuck instance. Use DNS-based routing (Route53 latency routing, Cloud DNS geolocation) or application-level routing.

## Replication Lag

Cross-region replication is asynchronous. There is a delay between a write in the primary region and the data appearing in the secondary region:

- **AWS S3 CRR:** Typically 15 minutes (SLA: within 15 minutes for 99.99% of objects)
- **GCS Multi-Region:** Near-instant (objects are replicated before the PUT returns)
- **Azure GRS:** Typically seconds to minutes

During this lag window, the read replica serves slightly stale catalog data. For most analytics workloads, this is acceptable — a catalog that is a few minutes old still points to valid data files.

## Disaster Recovery

If the primary region becomes unavailable:

1. Verify replication is up to date (check the latest snapshot ID in the replica)
2. Promote the read replica to writer mode by removing the `--read-only` flag
3. Update DNS to point clients to the new primary
4. When the original region recovers, either make it the new replica or failback

Note: If there were in-flight writes at the time of the outage that had not yet replicated, they will be lost. This is the fundamental trade-off of asynchronous replication.
