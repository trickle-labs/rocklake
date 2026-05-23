# Supported SQL Reference

This page provides an exhaustive list of SQL statements that SlateDuck's bounded SQL dispatcher recognizes and handles. These are the exact patterns emitted by DuckDB's `ducklake` extension.

## Schema Operations

```sql
-- Create a schema
INSERT INTO ducklake_schemas (schema_name) VALUES ('analytics')

-- Drop a schema
UPDATE ducklake_schemas SET end_snapshot = ? WHERE schema_id = ?

-- Rename a schema
INSERT INTO ducklake_schemas (schema_id, schema_name, begin_snapshot) VALUES (?, 'new_name', ?)
UPDATE ducklake_schemas SET end_snapshot = ? WHERE schema_id = ? AND begin_snapshot = ?

-- List schemas
SELECT schema_id, schema_name FROM ducklake_schemas WHERE begin_snapshot <= ? AND (end_snapshot IS NULL OR end_snapshot > ?)
```

## Table Operations

```sql
-- Create a table
INSERT INTO ducklake_tables (table_name, schema_id) VALUES ('events', 1)

-- Drop a table
UPDATE ducklake_tables SET end_snapshot = ? WHERE table_id = ?

-- Rename a table
INSERT INTO ducklake_tables (table_id, schema_id, table_name, begin_snapshot) VALUES (?, ?, 'new_name', ?)
UPDATE ducklake_tables SET end_snapshot = ? WHERE table_id = ? AND begin_snapshot = ?

-- Move table to schema
INSERT INTO ducklake_tables (table_id, schema_id, table_name, begin_snapshot) VALUES (?, new_schema_id, ?, ?)
UPDATE ducklake_tables SET end_snapshot = ? WHERE table_id = ? AND begin_snapshot = ?

-- List tables in schema
SELECT table_id, table_name FROM ducklake_tables WHERE schema_id = ? AND begin_snapshot <= ? AND (end_snapshot IS NULL OR end_snapshot > ?)
```

## Column Operations

```sql
-- Add column
INSERT INTO ducklake_columns (table_id, column_name, data_type, column_index, is_nullable) VALUES (?, ?, ?, ?, ?)

-- Drop column
UPDATE ducklake_columns SET end_snapshot = ? WHERE column_id = ?

-- Rename column
INSERT INTO ducklake_columns (column_id, table_id, column_name, ..., begin_snapshot) VALUES (?, ?, 'new_name', ..., ?)
UPDATE ducklake_columns SET end_snapshot = ? WHERE column_id = ? AND begin_snapshot = ?

-- List columns
SELECT column_id, column_name, data_type, column_index, is_nullable FROM ducklake_columns WHERE table_id = ? AND begin_snapshot <= ? AND (end_snapshot IS NULL OR end_snapshot > ?)
```

## Data File Operations

```sql
-- Register data file
INSERT INTO ducklake_data_files (table_id, file_path, file_size_bytes, row_count, snapshot_id) VALUES (?, ?, ?, ?, ?)

-- Register column statistics
INSERT INTO ducklake_file_column_stats (file_id, column_id, min_value, max_value, null_count) VALUES (?, ?, ?, ?, ?)

-- List data files for table
SELECT file_id, file_path, file_size_bytes, row_count FROM ducklake_data_files WHERE table_id = ? AND snapshot_id <= ?

-- Get column statistics
SELECT min_value, max_value, null_count FROM ducklake_file_column_stats WHERE file_id = ? AND column_id = ?
```

## Transaction Operations

```sql
-- Begin transaction (allocate snapshot)
BEGIN TRANSACTION

-- Commit (finalize snapshot)
COMMIT

-- Rollback
ROLLBACK
```

## Session Operations

```sql
-- These are accepted for protocol compatibility but have limited effect
SET search_path TO ...
SET client_encoding TO ...
SELECT version()
```

## Notes

- The exact SQL format may vary slightly between DuckDB versions (column ordering, quoting, spacing)
- SlateDuck's classifier handles these variations through pattern matching rather than exact string comparison
- Statements not matching any known pattern are rejected with SQLSTATE 42601 (Syntax Error)
