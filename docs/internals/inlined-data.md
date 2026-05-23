# Inlined Data

For very small data files (below a configurable threshold), SlateDuck can store the file contents directly in the catalog rather than as a separate object in storage. This eliminates the overhead of a separate object storage GET for data that is smaller than the cost of the GET itself.

## Motivation

A Parquet file with a single row might be 1-2 KB. Reading this file from S3 requires:
- 1 S3 GET request (~$0.0000004 + latency)
- Transfer of 1-2 KB of data

If the catalog already knows about this file (it has a metadata entry), and the file is very small, it is more efficient to store the data inline in the catalog entry. The reader gets the data as part of the normal catalog scan, without an additional round-trip.

## How It Works

When DuckDB registers a very small data file through the INSERT operation, SlateDuck checks the file size. If it is below the inline threshold (default: 4 KB), the file contents are stored in an `inlined_insert` entry (tag 0xFD) alongside the normal `ducklake_data_file` metadata entry.

The key format for inlined data:

```
0xFD | table_id (u64) | file_id (u64) | begin_snapshot (u64)
```

The value contains the raw file bytes (typically Parquet format) wrapped in the standard SDKV envelope.

## Reader Behavior

When DuckDB requests data files for a table, the reader returns both:
1. Regular data file metadata (pointing to external Parquet files in object storage)
2. Inlined data entries (containing the actual data bytes)

DuckDB handles inlined data transparently — it reads the Parquet bytes from the catalog response rather than fetching from object storage.

## Tradeoffs

**Advantages:**
- Eliminates extra object storage round-trip for tiny files
- Reduces total object count in storage (fewer tiny objects)
- Improves query latency for tables with many small files

**Disadvantages:**
- Increases catalog size (data bytes stored in the catalog)
- Catalog scans become heavier (reading data bytes during metadata scans)
- Complicates garbage collection (inlined data must be cleaned up with the metadata)

## Configuration

The inline threshold can be configured:

```bash
SLATEDUCK_INLINE_THRESHOLD_BYTES=4096 slateduck --storage s3://bucket/catalog/
```

Set to 0 to disable inlining entirely. For most workloads, the default (4 KB) is appropriate — it captures single-row inserts and very small batch files without bloating the catalog.
