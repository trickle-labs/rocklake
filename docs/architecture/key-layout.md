# Key Layout

Every piece of catalog metadata in SlateDuck is stored as a key-value pair in SlateDB's LSM-tree. The key encoding is carefully designed to support efficient prefix scans, maintain lexicographic ordering, and encode hierarchical relationships without secondary indexes. This page documents the complete key layout.

## Encoding Principles

All keys follow these rules:

1. **Tag-first:** The first byte identifies which catalog table the entry belongs to (0x01-0x1C for DuckLake tables, 0xFC-0xFF for internal tables)
2. **Big-endian integers:** Multi-byte integers are encoded in big-endian (network byte order) to ensure lexicographic byte ordering matches numeric ordering
3. **Fixed-width fields:** Most key components are fixed-width u64 values (8 bytes each), making key parsing trivial
4. **Variable-length fields:** Where present (metadata keys, system keys), are length-prefixed with u16

## Complete Key Schema

### DuckLake Catalog Tables (0x01 - 0x1C)

| Tag | Table Name | Key Fields | Total Key Size |
|-----|-----------|------------|----------------|
| `0x01` | ducklake_metadata | `scope_enum(u8) \| scope_id(u64) \| key_len(u16) \| key_bytes` | 12 + key_len |
| `0x02` | ducklake_snapshot | `snapshot_id(u64)` | 9 |
| `0x03` | ducklake_snapshot_changes | `snapshot_id(u64)` | 9 |
| `0x04` | ducklake_schema | `schema_id(u64) \| begin_snapshot(u64)` | 17 |
| `0x05` | ducklake_table | `schema_id(u64) \| table_id(u64) \| begin_snapshot(u64)` | 25 |
| `0x06` | ducklake_column | `table_id(u64) \| column_id(u64) \| begin_snapshot(u64)` | 25 |
| `0x07` | ducklake_view | `schema_id(u64) \| view_id(u64) \| begin_snapshot(u64)` | 25 |
| `0x08` | ducklake_macro | `schema_id(u64) \| macro_id(u64) \| begin_snapshot(u64)` | 25 |
| `0x09` | ducklake_macro_impl | `macro_id(u64) \| impl_id(u64)` | 17 |
| `0x0A` | ducklake_macro_parameters | `macro_id(u64) \| impl_id(u64) \| column_id(u64)` | 25 |
| `0x0B` | ducklake_data_file | `table_id(u64) \| data_file_id(u64)` | 17 |
| `0x0C` | ducklake_delete_file | `data_file_id(u64) \| delete_file_id(u64)` | 17 |
| `0x0D` | ducklake_files_scheduled_for_deletion | `schedule_start(u64) \| data_file_id(u64)` | 17 |
| `0x0E` | ducklake_inlined_data_tables | `table_id(u64) \| schema_version(u64)` | 17 |
| `0x0F` | ducklake_column_mapping | `table_id(u64) \| column_id(u64)` | 17 |
| `0x10` | ducklake_name_mapping | `table_id(u64) \| column_id(u64)` | 17 |
| `0x11` | ducklake_table_stats | `table_id(u64)` | 9 |
| `0x12` | ducklake_file_column_stats | `table_id(u64) \| column_id(u64) \| data_file_id(u64)` | 25 |
| `0x13` | ducklake_file_variant_stats | `table_id(u64) \| column_id(u64) \| data_file_id(u64)` | 25 |
| `0x14` - `0x1C` | partition_info, partition_columns, file_partition_values, sort_info, sort_expressions, tags, column_tags, schema_versions | (various) | (various) |

### Internal Tables (0xFC - 0xFF)

| Tag | Purpose | Key Fields |
|-----|---------|-----------|
| `0xFC` | Secondary index | `snapshot_id(u64) \| table_id(u64) \| data_file_id(u64)` |
| `0xFD` | Inlined data | `subtype(u8) \| table_id(u64) \| (schema_version or data_file_id)(u64) \| row_id(u64)` |
| `0xFE` | Counters | `counter_id(u8)` |
| `0xFF` | System keys | `suffix_bytes (variable, e.g. "writer-epoch", "retain-from")` |

## Prefix Scan Patterns

The key layout is optimized for these common access patterns:

**List all tables in a schema:** Scan prefix `0x05 | schema_id`. Returns all table versions (including historical) for that schema, in order of table_id then begin_snapshot.

**List all columns for a table:** Scan prefix `0x06 | table_id`. Returns all column versions for that table, in order of column_id then begin_snapshot.

**List all data files for a table:** Scan prefix `0x0B | table_id`. Returns all data files for that table, in order of data_file_id.

**List all file column stats for a table:** Scan prefix `0x12 | table_id`. Returns stats for all columns across all files.

**List all inlined inserts for a table:** Scan prefix `0xFD | 0x01 | table_id`. Returns all inlined insert rows for the table.

## Lexicographic Ordering

Big-endian encoding ensures that the natural ordering of keys matches what you would expect logically:

- Tables in schema 1 come before tables in schema 2 (because `0x0000000000000001` < `0x0000000000000002` lexicographically)
- Within a schema, tables are ordered by table_id
- Within a table, versions are ordered by begin_snapshot (oldest first)

This ordering is important for SlateDB's SST structure: keys within an SST are sorted, and SSTs are organized by key range. Prefix scans exploit this sorting to efficiently skip irrelevant key ranges.

## Counter Keys

Counter keys are minimal: `0xFE | counter_id`. The counter_id values are:

| Counter ID | Meaning | Initial Value |
|-----------|---------|---------------|
| `0x01` | next_snapshot_id | 1 |
| `0x02` | next_catalog_id | 1 |
| `0x03` | next_file_id | 1 |
| `0x04+` | next_column_id per table | 1 |

Counters are updated atomically in the same write batch as the rows they reference. This ensures that an allocated ID is always paired with its corresponding row (no "phantom" IDs from a crash between allocation and use).

## System Keys

System keys use tag `0xFF` followed by a human-readable suffix:

| Key | Purpose |
|-----|---------|
| `0xFF \| "writer-epoch"` | Current writer epoch (u64, for fencing) |
| `0xFF \| "retain-from"` | GC retention horizon (snapshot_id, 0 = infinite) |
| `0xFF \| "catalog-format-version"` | Format version (u32, currently 1) |
| `0xFF \| "hot-key"` | Packed current state for cold-start optimization |
| `0xFF \| "audit" \| snapshot_id` | Per-snapshot audit log entry (JSON) |
| `0xFF \| "checkpoint:" \| timestamp` | Checkpoint metadata |
| `0xFF \| "pin:" \| snapshot_id` | Pinned snapshot marker |

System keys are sparse (typically fewer than 20 entries in a catalog) and accessed by exact key lookup, not prefix scan.
