# Type-Aware Statistics

SlateDuck stores per-column statistics for each data file (min value, max value, null count, distinct count). These statistics enable partition pruning: DuckDB can skip entire data files if their column statistics prove the file contains no matching rows for a given predicate.

## The Challenge

DuckDB supports many column types (INTEGER, VARCHAR, TIMESTAMP, DECIMAL, UUID, etc.), and each type has different comparison semantics and serialization requirements. The statistics must be stored in a uniform format (protobuf bytes in the catalog) while preserving type-specific comparison semantics when DuckDB uses them for pruning.

## Implementation

Statistics are stored in the `ducklake_file_column_stats` table with these fields:

- `file_id` — Which data file this statistic belongs to
- `column_id` — Which column this statistic describes
- `min_value` — Minimum value in the file for this column (as bytes)
- `max_value` — Maximum value in the file for this column (as bytes)
- `null_count` — Number of NULL values
- `has_null` — Whether any NULLs exist (boolean, faster to check than null_count > 0)

The `min_value` and `max_value` fields are stored as opaque byte arrays. The encoding of these bytes depends on the column's DuckDB type, as implemented in `crates/slateduck-core/src/types.rs`:

- **Integers:** Big-endian fixed-width encoding (same as key encoding)
- **Strings/VARCHAR:** UTF-8 bytes
- **Timestamps:** Microseconds since epoch as i64 big-endian
- **Decimals:** Scaled integer representation
- **UUIDs:** 16 raw bytes

## Type Registry

The type system in `types.rs` maintains a registry mapping DuckDB type names to encoding/comparison functions. When DuckDB registers a data file with column statistics, SlateDuck validates that the statistic bytes are well-formed for the declared column type.

## How DuckDB Uses Statistics

During query planning, DuckDB asks the catalog for column statistics of each data file. It then compares the query predicates against the min/max values:

1. Query: `SELECT * FROM events WHERE timestamp > '2024-01-01'`
2. DuckDB asks SlateDuck for file column stats for the `timestamp` column
3. For each data file, DuckDB compares '2024-01-01' against the max_value
4. If max_value < '2024-01-01', the entire file is skipped (no matching rows possible)

This can dramatically reduce I/O for range queries on partitioned data.

## Limitations

- Statistics are only as good as the data files provide. If a Parquet file does not contain column statistics, SlateDuck stores NULL for min/max and DuckDB cannot prune.
- Complex types (STRUCT, LIST, MAP) do not have meaningful min/max statistics.
- Statistics are per-file, not per-row-group. More granular pruning requires reading Parquet file metadata directly (which DuckDB does separately).
