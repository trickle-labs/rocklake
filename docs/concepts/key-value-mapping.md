# Key-Value Mapping

SlateDuck stores relational catalog concepts (schemas, tables, columns, data files) in a key-value store where keys are opaque byte sequences with careful structure. Understanding this mapping helps you reason about scan performance, predict which operations are fast, and understand the output of debugging tools.

## The Design Challenge

A relational catalog has natural hierarchies: schemas contain tables, tables contain columns, tables reference data files. To store this hierarchy in a flat key-value namespace, SlateDuck encodes the relationships directly into the key bytes using a tag-prefixed, big-endian encoding scheme that preserves lexicographic ordering.

## Key Structure

Every key in SlateDuck's catalog follows this pattern:

```
[tag: 1 byte] [composite key fields: variable length]
```

The first byte is the **tag**, which identifies which catalog table the entry belongs to. For example, tag `0x04` is `ducklake_schema`, tag `0x05` is `ducklake_table`, tag `0x06` is `ducklake_column`, and tag `0x0B` is `ducklake_data_file`.

After the tag, the remaining bytes encode the composite key fields for that table, with multi-byte integers stored in **big-endian** format. Big-endian encoding is critical because it ensures that lexicographic byte ordering matches numeric ordering. When you scan keys with a prefix, you get results in ascending ID order.

## Examples

A table row for table ID 42 in schema ID 1, created at snapshot 7:

```
key: 0x05 | 0x0000000000000001 | 0x000000000000002A | 0x0000000000000007
      tag      schema_id (1)        table_id (42)       begin_snapshot (7)
```

A column row for column ID 3 in table ID 42, created at snapshot 7:

```
key: 0x06 | 0x000000000000002A | 0x0000000000000003 | 0x0000000000000007
      tag      table_id (42)        column_id (3)       begin_snapshot (7)
```

A data file row for file ID 100 in table ID 42:

```
key: 0x0B | 0x000000000000002A | 0x0000000000000064
      tag      table_id (42)        data_file_id (100)
```

## Why This Encoding?

The encoding is designed to make the most common access patterns efficient:

**Listing all tables in a schema** is a prefix scan on `0x05 | schema_id`. SlateDB seeks to the first key with that prefix and scans forward until the prefix changes. This is O(tables_in_schema), not O(total_tables).

**Listing all columns for a table** is a prefix scan on `0x06 | table_id`. Again, O(columns_in_table).

**Listing all data files for a table** is a prefix scan on `0x0B | table_id`. This is the most common read operation in the catalog (DuckDB needs the file list to execute any query) and it is optimally efficient.

**Point lookups** for a specific entity (given all key fields) are single GET operations in SlateDB. SlateDB's LSM-tree with bloom filters makes point lookups very fast.

## The Tag Registry

SlateDuck allocates tags from a fixed registry of 28 DuckLake catalog tables plus internal system tables:

| Tag Range | Purpose |
|-----------|---------|
| `0x01` - `0x1C` | DuckLake catalog tables (metadata, snapshots, schemas, tables, columns, views, macros, data files, delete files, statistics, partitions, sort info, tags, etc.) |
| `0xFC` | Secondary index (performance optimization for snapshot-scoped file lookups) |
| `0xFD` | Inlined data (small row inserts/deletes stored directly in the catalog) |
| `0xFE` | Counters (auto-incrementing ID generators for snapshots, catalog IDs, file IDs) |
| `0xFF` | System keys (writer epoch, retention settings, format version, audit log, checkpoints) |

The tag is the first byte of every key, which means a prefix scan for `0x05` (all tables) will never accidentally include entries from `0x06` (columns) or `0x0B` (data files). The tag provides perfect namespace isolation at the byte level.

## MVCC and Key Uniqueness

For versioned tables (schemas, tables, columns, views, macros), the `begin_snapshot` is part of the key. This means multiple versions of the same logical entity have different keys:

```
0x05 | schema_id=1 | table_id=42 | begin_snapshot=7   (original version)
0x05 | schema_id=1 | table_id=42 | begin_snapshot=15  (renamed version)
```

When reading at a specific snapshot, the MVCC filter examines both rows and returns only the one whose visibility bounds include the target snapshot. The original version has `end_snapshot=15` (set when the rename occurred) and the new version has `end_snapshot=NULL` (still active). A reader at snapshot 10 sees the original; a reader at snapshot 20 sees the renamed version.

## Value Encoding

Values are wrapped in a lightweight envelope format:

```
[encoding_version: 1 byte] [magic: "SDKV" 4 bytes] [protobuf payload: variable]
```

The magic bytes and version allow detection of corruption and future format evolution. The payload is a protobuf-encoded message whose schema depends on the tag (e.g., tag `0x05` values decode as `TableRow`, tag `0x06` as `ColumnRow`).

## Implications for Performance

The key-value mapping directly determines performance characteristics:

- **Operations that follow natural key prefixes are fast:** listing tables in a schema, columns in a table, files for a table. These are all prefix scans that touch only relevant keys.
- **Operations that span multiple tags are multiple scans:** describing a table (need TableRow from `0x05` and ColumnRows from `0x06`) requires two prefix scans.
- **Cross-cutting queries are expensive:** finding all tables across all schemas that have a column named "email" would require scanning all columns (`0x06` prefix) and filtering in memory. SlateDuck does not need this operation, but it illustrates the trade-off.
