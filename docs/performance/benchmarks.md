# Benchmarks

This page documents SlateDuck's benchmarking methodology, baseline results, and how to reproduce benchmarks in your environment.

## Methodology

Benchmarks measure catalog operation latency and throughput under controlled conditions. Each benchmark:

1. Creates a fresh catalog with known contents
2. Performs N iterations of the target operation
3. Records per-operation latency (p50, p95, p99)
4. Reports throughput (operations per second)

Benchmarks use the `criterion` framework for statistical rigor (warm-up iterations, confidence intervals, outlier detection).

## Baseline Results

Results from the standard benchmark suite on a c5.xlarge EC2 instance in us-east-1 with S3 Standard storage:

### Point Operations

| Operation | p50 | p95 | p99 | Throughput |
|-----------|-----|-----|-----|-----------|
| Read schema (cache hit) | 0.8ms | 1.2ms | 2.1ms | 1,200 ops/s |
| Read schema (cache miss) | 48ms | 72ms | 95ms | 20 ops/s |
| Read table metadata | 1.1ms | 1.8ms | 3.2ms | 900 ops/s |
| Read column (single) | 0.9ms | 1.5ms | 2.8ms | 1,100 ops/s |

### Scan Operations

| Operation | p50 | p95 | p99 | Throughput |
|-----------|-----|-----|-----|-----------|
| List schemas (10 schemas) | 2.1ms | 3.5ms | 5.2ms | 470 ops/s |
| List columns (50 columns) | 4.8ms | 7.2ms | 12ms | 200 ops/s |
| List data files (1000 files) | 18ms | 28ms | 42ms | 55 ops/s |

### Write Operations

| Operation | p50 | p95 | p99 | Throughput |
|-----------|-----|-----|-----|-----------|
| Create table (5 columns) | 82ms | 120ms | 180ms | 12 ops/s |
| Register 1 data file | 75ms | 110ms | 160ms | 13 ops/s |
| Register 100 data files (batch) | 95ms | 140ms | 210ms | 10 ops/s |

### Observations

Write operations are dominated by the S3 PUT latency (~70-100ms). The actual catalog logic adds negligible overhead (< 1ms). Batching 100 file registrations into one transaction only marginally increases latency compared to registering 1 file, demonstrating the efficiency of write batching.

Read operations with warm cache are very fast (< 2ms). Cold cache reads pay the full S3 GET penalty. In production, frequently accessed keys stay warm, so most operations hit the cache.

## Running Benchmarks

```bash
cd crates/slateduck-catalog
cargo bench --bench catalog_bench
```

The benchmarks require access to object storage (S3 or local filesystem). Configure via environment variables:

```bash
# Local filesystem (fastest, for development)
BENCHMARK_STORAGE=./bench-catalog cargo bench

# S3 (realistic production latency)
AWS_REGION=us-east-1 BENCHMARK_STORAGE=s3://bench-bucket/catalog/ cargo bench
```

## Interpreting Results

When evaluating these numbers for your use case, consider:

1. **Network distance matters.** If DuckDB and SlateDuck are in different regions, add cross-region latency to every operation.
2. **Cache warm-up.** First operations after startup will be slower (cold cache). Steady-state performance is better.
3. **Catalog size.** Larger catalogs (more tables, more files) have more SST blocks to scan. Performance degrades linearly with catalog size for scan operations.
4. **S3 Express One Zone.** Reduces all S3-dominated operations by 5-10x. If latency is your primary concern, this is the highest-impact optimization.
