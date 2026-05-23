# Access-Pattern and Key-Layout Analysis

Derived from the DuckDB wire corpus (`tests/fixtures/wire-corpus/duckdb-1.5.2.jsonl`)
and the DuckLake specification.

## Observed Access Patterns

### Read Patterns

| Table | Dominant Query Shape | Key Layout Implication |
|-------|---------------------|----------------------|
| `ducklake_snapshot` | `SELECT max(snapshot_id)` / `ORDER BY snapshot_id DESC LIMIT 1` | Reverse scan on tag `0x02` prefix returns latest |
| `ducklake_schema` | `WHERE begin_snapshot <= $1 AND (end_snapshot IS NULL OR $1 < end_snapshot)` | Scan tag `0x04` prefix, MVCC filter in app layer |
| `ducklake_table` | `WHERE schema_id = $1 AND begin_snapshot <= ...` | Prefix `0x05 | schema_id` scans all tables in schema |
| `ducklake_column` | `WHERE table_id = $1 AND begin_snapshot <= ...` | Prefix `0x06 | table_id` scans all columns in table |
| `ducklake_data_file` | `WHERE table_id = $1 AND begin_snapshot <= ...` | Prefix `0x0B | table_id` scans all files for table |
| `ducklake_file_column_stats` | `WHERE table_id = $1 AND column_id = $2 AND ...` | Prefix `0x13 | table_id | column_id` for pruning |
| `pg_catalog.pg_type` | `WHERE typname IN (...)` | Hardcoded response; no KV lookup |

### Write Patterns

| Operation | Tables Written | Transaction Scope |
|-----------|---------------|-------------------|
| `CREATE TABLE` | snapshot, snapshot_changes, table, columns | Single transaction |
| `INSERT` (data) | snapshot, snapshot_changes, data_file, file_column_stats | Single transaction |
| `DROP TABLE` | snapshot, snapshot_changes, UPDATE table.end_snapshot | Single transaction |
| `ALTER TABLE ADD COLUMN` | snapshot, snapshot_changes, column | Single transaction |

## Key Decisions from Corpus Analysis

### 1. ID Allocation

DuckDB supplies explicit IDs in `INSERT` statements. The client reads
`next_catalog_id` from `ducklake_metadata`, allocates locally, then passes
the ID in the INSERT. SlateDuck must support this pattern by exposing counter
reads and increments atomically within a transaction.

### 2. `data_path` Handling

In the captured corpus, `ducklake_metadata.data_path` is an absolute path
(e.g., `s3://bucket/data/warehouse-a/`). The `path_is_relative` column on
`ducklake_data_file` distinguishes relative vs absolute file paths.

### 3. Transaction Wrapping

DuckDB wraps catalog operations in explicit `BEGIN`/`COMMIT`. Every
schema-modifying operation (CREATE TABLE, INSERT, etc.) is inside a
transaction block.

### 4. Protocol Usage

DuckDB uses the simple query protocol for all catalog operations observed.
Extended query protocol (`Parse`/`Bind`/`Execute`) was not observed in the
basic tutorial corpus but should be supported for parameter binding.

### 5. `pg_catalog` Probes

DuckDB probes `pg_catalog.pg_type` during startup to map type OIDs.
No other `pg_catalog` tables were observed in the captured corpus.

## Key Layout Confirmation

The proposed key layout from the design document is confirmed by the access
patterns. No revisions needed:

- Schema lookups: `0x04 | schema_id` → scan prefix for MVCC filtering
- Table lookups by schema: `0x05 | schema_id | table_id | begin_snapshot`
- Column lookups by table: `0x06 | table_id | column_id | begin_snapshot`
- File lookups by table: `0x0B | table_id | data_file_id`
- Stats lookups: `0x13 | table_id | column_id | data_file_id`

All prefix-scan patterns align with the proposed key structure.
