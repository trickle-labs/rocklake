# Latency Model

Understanding SlateDuck's latency requires understanding the layers involved in a catalog operation. This page breaks down where time is spent and what can (and cannot) be optimized.

## The Layers

A catalog read operation passes through these layers:

```
DuckDB → Network → SlateDuck PG-wire → SQL Classifier → CatalogReader → SlateDB → Object Storage
```

Each layer adds latency:

| Layer | Typical Latency | Optimization |
|-------|----------------|-------------|
| Network (localhost) | < 1ms | Use native extension (Strategy C) |
| Network (same VPC) | 1-5ms | Co-locate in same AZ |
| PG-wire parse | < 0.1ms | None needed |
| SQL classification | < 0.01ms | None needed |
| CatalogReader logic | < 0.1ms | None needed |
| SlateDB read (cache hit) | < 1ms | Increase cache size |
| SlateDB read (cache miss → S3) | 20-100ms | Use S3 Express, increase cache |
| SlateDB read (cache miss → GCS) | 10-50ms | Increase cache |
| SlateDB read (local FS) | < 1ms | Use SSD |

## Dominant Factors

For most deployments, the dominant latency factor is **object storage round-trip time**. When SlateDB's in-memory cache does not contain the requested data, it must fetch an SST block from S3/GCS/Azure, which takes 20-100ms depending on the provider and network conditions.

The second most significant factor is **scan amplification** — how many key-value pairs must be read to answer a query. A table with 50 columns requires reading ~50 column rows. If each column has been modified 5 times and GC has not run, that is ~250 rows to scan.

## Hot Key Optimization

SlateDuck caches the "hot key" — the most frequently read system key that contains high-level catalog metadata. This avoids an object storage round-trip for the most common operation (checking if the catalog is accessible). The hot key is refreshed on writes and cached indefinitely between writes.

## Write Latency

Write latency is dominated by the SlateDB WAL append, which requires one PUT to object storage:

| Storage Backend | Write Latency |
|----------------|--------------|
| S3 Standard | 50-150ms |
| S3 Express One Zone | 3-10ms |
| GCS | 30-80ms |
| Azure Blob | 30-100ms |
| Local filesystem | < 1ms |

Writes are batched — a transaction that registers 100 files creates one write batch with 100+ key-value pairs, all committed in a single PUT. The per-operation cost is amortized across the batch.

## End-to-End Examples

**Simple table lookup (S3 Standard, warm cache):**
- Network: 1ms
- Classification + logic: 0.1ms
- SlateDB cache hit: 0.5ms
- **Total: ~2ms**

**Table creation (S3 Standard):**
- Network: 1ms
- Classification + logic: 0.1ms
- SlateDB write (WAL PUT): 80ms
- **Total: ~81ms**

**List 50 columns (S3 Standard, cold cache):**
- Network: 1ms
- Classification + logic: 0.1ms
- SlateDB scan (1 SST block fetch): 50ms
- **Total: ~51ms**

These numbers represent typical behavior. Actual latency varies with network conditions, object storage load, cache state, and catalog size.
