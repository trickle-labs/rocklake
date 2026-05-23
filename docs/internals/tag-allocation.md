# Tag Allocation

Every key in SlateDuck begins with a 1-byte tag that identifies the table (entity type) the key belongs to. This page documents how tags are allocated, what each tag means, and how the allocation scheme was designed.

## Tag Space

The tag is a single byte (0x00 - 0xFF), giving 256 possible table types. Currently, approximately 30 tags are allocated, leaving substantial room for future expansion.

## Allocation Scheme

Tags are allocated in three ranges:

### DuckLake Protocol Tables (0x01 - 0x1F)

These tags correspond to the table types defined by DuckDB's DuckLake extension. They store the catalog metadata that DuckDB expects to find:

| Tag | Table | Description |
|-----|-------|-------------|
| 0x01 | ducklake_catalog | Root catalog entry |
| 0x02 | ducklake_snapshot | Snapshot metadata (ID, timestamp, author) |
| 0x03 | ducklake_table_snapshot | Table-level snapshot associations |
| 0x04 | ducklake_schema | Schema definitions |
| 0x05 | ducklake_table | Table definitions |
| 0x06 | ducklake_column | Column definitions |
| 0x07 | ducklake_data_file | Registered data file metadata |
| 0x08 | ducklake_delete_file | Registered delete file metadata |
| 0x09 | ducklake_file_column_stats | Per-column statistics for data files |
| 0x0A | ducklake_table_stats | Table-level statistics |
| 0x0B | ducklake_view | View definitions |
| 0x0C | ducklake_macro | Macro definitions |
| ... | ... | Additional DuckLake tables |

### Internal Tables (0x80 - 0xFD)

These tags are for SlateDuck's internal use — data structures not part of the DuckLake protocol:

| Tag | Table | Description |
|-----|-------|-------------|
| 0x80 | secondary_index | Hot-path secondary indexes |
| 0x81 | audit_entry | Audit log entries |
| 0xFD | inlined_insert | Inlined small data files |

### System Space (0xFE - 0xFF)

Reserved for counters and system keys:

| Tag | Table | Description |
|-----|-------|-------------|
| 0xFE | counter | ID allocation counters |
| 0xFF | system | System keys (writer epoch, format version, etc.) |

## Design Rationale

**Why a single byte?** Because it is the most compact prefix that provides adequate namespace separation. A 2-byte tag would waste a byte on every key for no practical benefit (256 table types is far more than needed).

**Why separate ranges?** DuckLake tables (0x01-0x1F) are distinct from internal tables (0x80-0xFD) so that a hex dump of keys immediately reveals whether a key is "protocol" or "internal." The gap between ranges (0x20-0x7F) is reserved for future DuckLake protocol extensions.

**Why are system keys at 0xFF?** Because they sort last in lexicographic order. This means a forward scan of the entire keyspace naturally visits data tables first and system keys last, which matches typical access patterns (you almost never need to scan system keys together with data tables).
