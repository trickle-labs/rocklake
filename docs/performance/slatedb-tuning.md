# SlateDB Tuning

> Historical reference only. The timing and cache thresholds below are not
> v0.63.2 measurements or operating limits.

RockLake inherits its storage engine from [SlateDB](https://slatedb.io), an
embedded LSM-tree key-value store that writes directly to object storage.
This page documents the parameters exposed through `rocklake tune` and the
`--cost-mode` flag, plus guidance on compaction tuning.

## Quick-start

Run the auto-tuner and apply the recommended settings:

```
rocklake tune --catalog s3://my-bucket/catalog --apply
```

Without `--apply` the command prints a diff of current vs. recommended
settings.

## Key parameters

### `l0_sst_count_threshold`

Controls how many Level-0 SSTs may accumulate before a compaction is
triggered.

| Value | Behaviour |
|-------|-----------|
| 2     | Compact aggressively; low read-amplification, higher PUT rate |
| 4     | Default; TPC-H SF10 benchmark operating point |
| 8     | Conservative; fewer S3 PUTs, higher p99 read-amplification |

Rule of thumb: measure object-store and compaction behavior before changing
these historical settings. `rocklake inspect cache-utilization` does not
currently expose SlateDB hit ratios.

### `max_write_batch_bytes`

Maximum number of bytes buffered in the memtable before flushing to L0.
Higher values reduce flush frequency (and therefore PUT count) at the cost
of higher peak memory usage and longer recovery time after a crash.

Recommended range: 16 MiB – 128 MiB.

### `block_size`

Block size for SST encoding. Larger blocks reduce the number of GET requests
per read but increase the bytes downloaded per cache miss.

- 4 KiB: best for point lookups with a large block cache
- 8 KiB: default; good balance for scan-heavy workloads
- 16 KiB+: best for bulk-scan workloads with tight memory budgets

### `bloom_filter_fp_rate`

False-positive rate for the per-SST Bloom filters. The default 0.01 (1%)
means roughly 1 in 100 key misses will perform a needless SST read.

Reducing this to 0.001 increases filter memory usage by ~40% but cuts wasted
GETs on key misses by 10×.

## Compaction tuning

Compaction runs in the background and merges L0 SSTs into larger L1 files.
The `compaction_aggressiveness` setting (1–10) controls how eagerly
compaction consumes I/O:

| Setting | CPU / S3 I/O usage | Best for |
|---------|-------------------|----------|
| 1–3     | Low               | Write-heavy ingest pipelines |
| 4–6     | Moderate (default)| Mixed workloads |
| 7–10    | High              | Read-heavy, latency-sensitive queries |

High aggressiveness reduces read-amplification at the expense of higher S3
PUT and GET counts during compaction.  Use `rocklake inspect api-costs` to
measure the trade-off in your environment.

## Block cache sizing guide

The block cache holds decompressed SST blocks in memory.  A well-sized cache
eliminates the majority of S3 GETs for hot catalog data.

Use `rocklake inspect cache-utilization` to see the catalog's estimated
working set. Cache occupancy, hit ratios, evictions, and capacity remain
unknown because the inspection command does not attach SlateDB's cache
metrics recorder:

```
rocklake inspect cache-utilization \
    --catalog s3://bucket/catalog/
```

The working-set calculation is a rough estimate based on catalog counts. This
report does not configure SlateDB or recommend a cache size; use a serving
configuration that explicitly owns the cache budget and collect real cache
metrics before tuning it.

## See also

- [Cost Analysis](cost-analysis.md) — how `l0_sst_count_threshold` affects
  your monthly S3 bill.
- [Tuning](tuning.md) — storage backend selection and general tuning guidance.
