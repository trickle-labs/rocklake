-- DuckLake Tutorial Golden Reference (SQLite-backed)
-- DuckDB 1.5.2 | DuckLake extension
-- This file captures expected output from the standard DuckLake tutorial
-- against a SQLite-backed catalog for spec-conformance comparison.

-- Step 1: Attach DuckLake catalog
-- ATTACH 'ducklake:catalog.db' AS lake (DATA_PATH 'data/');
-- Result: Success

-- Step 2: Create schema
-- CREATE SCHEMA lake.main;
-- Result: Success (schema_id=1, snapshot_id=1)

-- Step 3: Create table
-- CREATE TABLE lake.main.test_table (id INTEGER, name VARCHAR);
-- Result: Success (table_id=1, columns=[id:INTEGER, name:VARCHAR], snapshot_id=2)

-- Step 4: Insert data
-- INSERT INTO lake.main.test_table VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Charlie');
-- Result: 3 rows inserted, data_file created, snapshot_id=3

-- Step 5: Query data
-- SELECT * FROM lake.main.test_table;
-- Expected output:
-- ┌───────┬─────────┐
-- │  id   │  name   │
-- │ int32 │ varchar │
-- ├───────┼─────────┤
-- │     1 │ Alice   │
-- │     2 │ Bob     │
-- │     3 │ Charlie │
-- └───────┴─────────┘

-- Step 6: Time travel
-- SELECT * FROM lake.main.test_table AT (SNAPSHOT 2);
-- Expected: empty result (table existed but had no data at snapshot 2)

-- Step 7: Add column
-- ALTER TABLE lake.main.test_table ADD COLUMN email VARCHAR;
-- Result: Success (column_id=3, snapshot_id=4)

-- Step 8: Drop table
-- DROP TABLE lake.main.test_table;
-- Result: Success (end_snapshot set on table and columns, snapshot_id=5)
