# Performance Tuning

This page covers configuration options and operational practices that can improve SlateDuck's performance for specific workloads.

## Storage Backend Selection

The single highest-impact performance decision is your choice of object storage tier:

| Backend | Read Latency | Write Latency | Cost | Use Case |
|---------|-------------|--------------|------|----------|
| Local SSD | < 1ms | < 1ms | Hardware | Development, testing |
| S3 Express One Zone | 3-10ms | 3-10ms | 10x S3 Standard | Latency-sensitive production |
| S3 Standard | 20-100ms | 50-150ms | Baseline | Cost-optimized production |
| GCS Standard | 10-50ms | 30-80ms | Similar to S3 | GCP deployments |
| MinIO (local) | 1-5ms | 1-5ms | Self-hosted | Air-gapped environments |

If your workload is latency-sensitive (interactive queries, sub-second response times), S3 Express One Zone or a local MinIO instance will provide dramatically better performance than S3 Standard.

## Cache Tuning

SlateDB's block cache keeps frequently-accessed SST blocks in memory. A larger cache means more operations are served from memory without object storage round-trips:

```bash
# Increase cache size (default varies by SlateDB version)
SLATEDUCK_CACHE_SIZE_MB=256 slateduck --storage s3://bucket/catalog/
```

For catalogs with < 10,000 total rows, a 64MB cache typically holds the entire catalog in memory after warm-up. For larger catalogs, size the cache to fit the "working set" (the tables and schemas accessed frequently).

## Garbage Collection Impact

Superseded rows that have not been garbage collected increase scan amplification. If a prefix scan returns 100 rows but only 10 are visible at the current snapshot, 90% of the I/O is wasted.

Run GC regularly to keep the ratio of visible rows to total rows high:

```bash
# Check the ratio
slateduck inspect --storage s3://bucket/catalog/
# Look at total rows vs. live entity counts
```

If the ratio of total rows to live entities is > 3:1, GC + excision will noticeably improve scan performance.

## Write Batching

SlateDuck automatically batches all writes within a transaction into a single write batch. To maximize write throughput, group related operations into transactions:

```sql
-- Inefficient: 100 separate transactions (100 S3 PUTs)
-- Each INSERT is auto-committed
INSERT INTO ducklake_data_file ...;
INSERT INTO ducklake_data_file ...;
-- (repeated 100 times)

-- Efficient: 1 transaction (1 S3 PUT)
BEGIN;
INSERT INTO ducklake_data_file ...;
INSERT INTO ducklake_data_file ...;
-- (repeated 100 times)
COMMIT;
```

DuckDB's `ducklake` extension already does this naturally — it batches all file registrations from an INSERT statement into one transaction.

## Network Optimization

- **Co-locate DuckDB and SlateDuck** in the same availability zone to minimize network latency
- **Use VPC endpoints** for S3 access to avoid public internet routing
- **Use the native extension** (Strategy C) to eliminate network overhead entirely for co-located deployments

## Hot Key Caching

The hot key cache is enabled by default and provides a fast path for the most common read pattern. It can be disabled for debugging purposes but should always be enabled in production:

```bash
SLATEDUCK_HOT_KEY_CACHE=true slateduck --storage s3://bucket/catalog/
```
