# Catalog Tables Reference

This page lists all table types stored in the SlateDuck catalog, their fields, and their purposes. Each table type corresponds to a tag byte in the key encoding and a protobuf message type for its values.

## DuckLake Protocol Tables

These tables implement the DuckLake catalog protocol. Their schema matches what DuckDB's `ducklake` extension expects.

### ducklake_catalog (tag 0x01)

The root catalog entry. One row per catalog.

| Field | Type | Description |
|-------|------|-------------|
| catalog_id | u64 | Unique catalog identifier |
| catalog_name | string | Human-readable catalog name |
| catalog_version | u64 | Schema version of this catalog |

### ducklake_snapshot (tag 0x02)

Records each catalog snapshot (atomic commit point).

| Field | Type | Description |
|-------|------|-------------|
| snapshot_id | u64 | Unique snapshot identifier |
| timestamp | i64 | Unix timestamp (microseconds) when snapshot was created |
| author | string | Who created the snapshot (process name or user) |
| message | string | Optional human-readable commit message |

### ducklake_schema (tag 0x04)

Schema definitions. Versioned (has begin_snapshot, end_snapshot).

| Field | Type | Description |
|-------|------|-------------|
| schema_id | u64 | Unique schema identifier |
| schema_name | string | Schema name (e.g., "public", "analytics") |
| begin_snapshot | u64 | Snapshot that created this version |
| end_snapshot | Option<u64> | Snapshot that superseded this version (None if current) |

### ducklake_table (tag 0x05)

Table definitions. Versioned.

| Field | Type | Description |
|-------|------|-------------|
| table_id | u64 | Unique table identifier |
| schema_id | u64 | Schema this table belongs to |
| table_name | string | Table name |
| begin_snapshot | u64 | Snapshot that created this version |
| end_snapshot | Option<u64> | Snapshot that superseded this version |

### ducklake_column (tag 0x06)

Column definitions. Versioned.

| Field | Type | Description |
|-------|------|-------------|
| column_id | u64 | Unique column identifier |
| table_id | u64 | Table this column belongs to |
| column_name | string | Column name |
| data_type | string | DuckDB type name (e.g., "BIGINT", "VARCHAR") |
| column_index | u32 | Position in the table (0-based) |
| is_nullable | bool | Whether the column allows NULLs |
| default_value | Option<string> | Default expression (if any) |
| begin_snapshot | u64 | Snapshot that created this version |
| end_snapshot | Option<u64> | Snapshot that superseded this version |

### ducklake_data_file (tag 0x07)

Registered data file metadata. Not versioned (belongs to a specific snapshot).

| Field | Type | Description |
|-------|------|-------------|
| file_id | u64 | Unique file identifier |
| table_id | u64 | Table this file belongs to |
| snapshot_id | u64 | Snapshot that registered this file |
| file_path | string | Object storage path to the data file |
| file_size_bytes | u64 | Size of the data file in bytes |
| row_count | u64 | Number of rows in the file |
| file_format | string | File format (typically "parquet") |

### ducklake_delete_file (tag 0x08)

Registered delete file metadata (for row-level deletes).

| Field | Type | Description |
|-------|------|-------------|
| file_id | u64 | Unique file identifier |
| table_id | u64 | Table this file belongs to |
| snapshot_id | u64 | Snapshot that registered this file |
| file_path | string | Object storage path to the delete file |
| data_file_id | u64 | The data file whose rows are being deleted |

### ducklake_file_column_stats (tag 0x09)

Per-column statistics for data files.

| Field | Type | Description |
|-------|------|-------------|
| file_id | u64 | Data file these stats describe |
| column_id | u64 | Column these stats describe |
| min_value | Option<bytes> | Minimum value (type-dependent encoding) |
| max_value | Option<bytes> | Maximum value (type-dependent encoding) |
| null_count | u64 | Number of NULL values |
| has_null | bool | Whether any NULLs exist |

### ducklake_view (tag 0x0B)

View definitions. Versioned.

| Field | Type | Description |
|-------|------|-------------|
| view_id | u64 | Unique view identifier |
| schema_id | u64 | Schema this view belongs to |
| view_name | string | View name |
| sql | string | View definition SQL |
| begin_snapshot | u64 | Snapshot that created this version |
| end_snapshot | Option<u64> | Snapshot that superseded this version |

### ducklake_macro (tag 0x0C)

Macro definitions. Versioned.

| Field | Type | Description |
|-------|------|-------------|
| macro_id | u64 | Unique macro identifier |
| schema_id | u64 | Schema this macro belongs to |
| macro_name | string | Macro name |
| macro_definition | string | Macro SQL body |
| begin_snapshot | u64 | Snapshot that created this version |
| end_snapshot | Option<u64> | Snapshot that superseded this version |

## System Tables

### counter (tag 0xFE)

ID allocation counters.

| Key Suffix | Description |
|------------|-------------|
| `next_snapshot_id` | Next snapshot ID to allocate |
| `next_catalog_id` | Next catalog/schema/table ID |
| `next_file_id` | Next file ID to allocate |

### system (tag 0xFF)

System configuration and state.

| Key Suffix | Description |
|------------|-------------|
| `catalog-format-version` | Format version (currently 1) |
| `writer-epoch` | Current writer epoch |
| `retain-from` | GC retention horizon |
| `hot-key` | Cached frequently-read metadata |
